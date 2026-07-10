//! The recorder app: three-column studio layout (clips | takes | stage)
//! with a state-aware stage. The mode is mirrored onto `<body class>` so
//! the stylesheet drives what each mode shows — same contract as the CSS.

use crate::types::{Episode, Review, TakeInfo};
use crate::{api, audio};
use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use wasm_bindgen::JsCast;

thread_local! {
    /// The just-recorded take (JS Blob is !Send, so it can't live in a signal).
    static PENDING_BLOB: RefCell<Option<web_sys::Blob>> = const { RefCell::new(None) };
    /// Countdown / record-timer interval ids so a new cycle clears the old.
    static TICKERS: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Idle,
    Countdown,
    Recording,
    Review,
    Uploading,
}

impl Mode {
    fn class(self) -> &'static str {
        match self {
            Mode::Idle => "idle",
            Mode::Countdown => "countdown",
            Mode::Recording => "recording",
            Mode::Review => "review",
            Mode::Uploading => "uploading",
        }
    }
}

/// Every signal the components share. Copy — hand it around freely.
#[derive(Clone, Copy)]
pub struct Store {
    pub mode: RwSignal<Mode>,
    pub episode: RwSignal<Option<Episode>>,
    pub current: RwSignal<usize>,
    pub takes: RwSignal<Vec<TakeInfo>>,
    pub takes_reload: RwSignal<u32>,
    pub status: RwSignal<String>,
    /// Coach result for the just-recorded take (None while analyzing).
    pub review: RwSignal<Option<Review>>,
    pub review_error: RwSignal<Option<String>>,
    /// Archive number the review pass stored the pending take under.
    pub pending_take: RwSignal<Option<u32>>,
    /// Expanded takes-rail row, keyed "<clip-id>/<file>".
    pub expanded: RwSignal<Option<String>>,
    /// Audition url currently playing in the rail.
    pub playing: RwSignal<Option<String>>,
    pub countdown: RwSignal<u32>,
    pub elapsed: RwSignal<f64>,
    pub meter: RwSignal<(f64, f64)>,
    pub devices: RwSignal<Vec<(String, String)>>,
    pub active_device: RwSignal<String>,
}

impl Store {
    fn new() -> Self {
        Self {
            mode: RwSignal::new(Mode::Idle),
            episode: RwSignal::new(None),
            current: RwSignal::new(0),
            takes: RwSignal::new(Vec::new()),
            takes_reload: RwSignal::new(0),
            status: RwSignal::new("pick a clip, hit space, read after the countdown".into()),
            review: RwSignal::new(None),
            review_error: RwSignal::new(None),
            pending_take: RwSignal::new(None),
            expanded: RwSignal::new(None),
            playing: RwSignal::new(None),
            countdown: RwSignal::new(3),
            elapsed: RwSignal::new(0.0),
            meter: RwSignal::new((0.0, 0.0)),
            devices: RwSignal::new(Vec::new()),
            active_device: RwSignal::new(String::new()),
        }
    }

    pub fn clip(&self, i: usize) -> Option<crate::types::Clip> {
        self.episode
            .with(|e| e.as_ref().and_then(|e| e.clips.get(i).cloned()))
    }

    pub fn current_clip(&self) -> Option<crate::types::Clip> {
        self.clip(self.current.get())
    }

    pub fn current_clip_untracked(&self) -> Option<crate::types::Clip> {
        self.episode.with_untracked(|e| {
            e.as_ref()
                .and_then(|e| e.clips.get(self.current.get_untracked()).cloned())
        })
    }

    /// Switch clips: reset the transient review state, keep archive state.
    pub fn select(&self, i: usize) {
        self.current.set(i);
        self.mode.set(Mode::Idle);
        self.review.set(None);
        self.review_error.set(None);
        self.pending_take.set(None);
        PENDING_BLOB.with(|b| *b.borrow_mut() = None);
        let c = self.clip(i);
        self.elapsed.set(0.0);
        if let Some(c) = c {
            let _ = c;
        }
    }
}

pub fn fmt2(v: f64) -> String {
    format!("{v:.2}")
}

fn clear_tickers() {
    TICKERS.with(|t| {
        for id in t.borrow_mut().drain(..) {
            audio::clear_interval(id);
        }
    });
}

fn begin_countdown(store: Store) {
    clear_tickers();
    store.mode.set(Mode::Countdown);
    store.review.set(None);
    store.review_error.set(None);
    store.pending_take.set(None);
    store.playing.set(None);
    PENDING_BLOB.with(|b| *b.borrow_mut() = None);
    store.countdown.set(3);
    let id = audio::set_interval(700, move || {
        let n = store.countdown.get_untracked();
        if n <= 1 {
            clear_tickers();
            start_recording(store);
        } else {
            store.countdown.set(n - 1);
        }
    });
    TICKERS.with(|t| t.borrow_mut().push(id));
}

fn start_recording(store: Store) {
    let started = audio::start_recording(move |blob| on_take_recorded(store, blob));
    if let Err(e) = started {
        store.mode.set(Mode::Idle);
        store.status.set(format!("recording failed: {e}"));
        return;
    }
    store.mode.set(Mode::Recording);
    store.status.set("recording… space to stop".into());
    store.elapsed.set(0.0);
    let t0 = audio::now_ms();
    let id = audio::set_interval(50, move || {
        store.elapsed.set((audio::now_ms() - t0) / 1000.0);
    });
    TICKERS.with(|t| t.borrow_mut().push(id));
}

fn stop_recording(store: Store) {
    clear_tickers();
    audio::stop_recording();
    let _ = store;
}

fn toggle_record(store: Store) {
    match store.mode.get_untracked() {
        Mode::Recording => stop_recording(store),
        Mode::Idle | Mode::Review => begin_countdown(store),
        _ => {}
    }
}

/// MediaRecorder finished: stash the blob, enter review, ship it to the
/// coach (which also archives it server-side — every take is kept).
fn on_take_recorded(store: Store, blob: web_sys::Blob) {
    PENDING_BLOB.with(|b| *b.borrow_mut() = Some(blob.clone()));
    store.mode.set(Mode::Review);
    store.status.set("listen back — keep it or go again".into());
    if let Some(player) = player_element() {
        if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
            player.set_src(&url);
        }
    }
    let Some(c) = store.current_clip_untracked() else {
        return;
    };
    spawn_local(async move {
        match api::review(&c.id, &blob).await {
            Ok(r) => {
                store.takes_reload.update(|n| *n += 1);
                if store.mode.get_untracked() != Mode::Review {
                    return; // user already moved on
                }
                store.pending_take.set(Some(r.take));
                store.review.set(Some(r));
            }
            Err(e) => store.review_error.set(Some(e)),
        }
    });
}

fn keep_take(store: Store) {
    if store.mode.get_untracked() != Mode::Review {
        return;
    }
    let Some(blob) = PENDING_BLOB.with(|b| b.borrow().clone()) else {
        return;
    };
    let Some(c) = store.current_clip_untracked() else {
        return;
    };
    store.mode.set(Mode::Uploading);
    store.status.set("saving…".into());
    let pending = store.pending_take.get_untracked();
    spawn_local(async move {
        // review already archived this take → approve it in place; otherwise
        // (review still in flight or failed) upload the blob directly
        let result = match pending {
            Some(n) => api::approve(&c.id, &format!("take_{n:03}.mp3")).await,
            None => api::upload_take(&c.id, &blob).await,
        };
        match result {
            Ok(info) => {
                apply_take_response(store, &c.id, &info);
                store.status.set(format!(
                    "saved {}s{}{}",
                    fmt2(info.duration),
                    if info.fits {
                        " ✓ fits".to_string()
                    } else {
                        format!(
                            " ⚠ over the {}s window — retake or stretch the scene",
                            fmt2(info.window)
                        )
                    },
                    if info.warnings.is_empty() {
                        String::new()
                    } else {
                        format!(
                            " · {} timeline warning(s), re-run tts fit-check",
                            info.warnings.len()
                        )
                    },
                ));
                store.select(store.current.get_untracked());
                store.takes_reload.update(|n| *n += 1);
            }
            Err(e) => {
                store.status.set(format!("save failed: {e}"));
                store.mode.set(Mode::Review);
            }
        }
    });
}

fn retake(store: Store) {
    if store.mode.get_untracked() == Mode::Review {
        begin_countdown(store);
    }
}

/// Reflect an approve/keep response into the episode signal so the clip
/// list + header show the new current-take duration.
fn apply_take_response(store: Store, id: &str, info: &crate::types::TakeResponse) {
    let id = id.to_string();
    let duration = info.duration;
    store.episode.update(|e| {
        if let Some(e) = e {
            if let Some(c) = e.clips.iter_mut().find(|c| c.id == id) {
                c.take_duration = Some(duration);
            }
        }
    });
}

fn player_element() -> Option<web_sys::HtmlAudioElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id("player")?
        .dyn_into()
        .ok()
}

fn audition_element() -> Option<web_sys::HtmlAudioElement> {
    web_sys::window()?
        .document()?
        .get_element_by_id("audition")?
        .dyn_into()
        .ok()
}

#[component]
pub fn App() -> impl IntoView {
    let store = Store::new();

    // mode → <body class> — CSS owns the mode presentation
    Effect::new(move |_| {
        let m = store.mode.get();
        if let Some(body) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.body())
        {
            body.set_class_name(m.class());
        }
    });

    // boot: episode, mic, meters
    Effect::new(move |_| {
        spawn_local(async move {
            match api::episode().await {
                Ok(e) => {
                    store.episode.set(Some(e));
                    store.select(0);
                }
                Err(e) => store.status.set(format!("episode load failed: {e}")),
            }
            match audio::init_mic(None).await {
                Ok((devices, active)) => {
                    store
                        .devices
                        .set(devices.into_iter().map(|d| (d.id, d.label)).collect());
                    store.active_device.set(active);
                    audio::start_meters(move |l, r| store.meter.set((l, r)));
                }
                Err(e) => store.status.set(format!("mic init failed: {e}")),
            }
        });
    });

    // takes rail follows the selected clip (and explicit reloads)
    Effect::new(move |_| {
        let Some(c) = store.current_clip() else {
            return;
        };
        let _ = store.takes_reload.get();
        spawn_local(async move {
            match api::takes(&c.id).await {
                Ok(t) => {
                    // drop stale responses if the user switched clips mid-fetch
                    if store.current_clip_untracked().map(|cc| cc.id) == Some(c.id) {
                        store.takes.set(t);
                    }
                }
                Err(e) => store.status.set(format!("takes: {e}")),
            }
        });
    });

    // audition player follows the `playing` signal
    Effect::new(move |_| {
        let url = store.playing.get();
        let Some(el) = audition_element() else {
            return;
        };
        match url {
            Some(u) => {
                el.set_src(&u);
                let _ = el.play();
            }
            None => el.pause().unwrap_or(()),
        }
    });

    window_event_listener(ev::keydown, move |e: web_sys::KeyboardEvent| {
        let idle = store.mode.get_untracked() == Mode::Idle;
        match (e.code().as_str(), e.key().as_str()) {
            ("Space", _) => {
                e.prevent_default();
                toggle_record(store);
            }
            (_, "Enter") => keep_take(store),
            (_, "r") => retake(store),
            (_, "ArrowDown" | "ArrowRight") if idle => {
                let max = store
                    .episode
                    .with_untracked(|e| e.as_ref().map(|e| e.clips.len()).unwrap_or(1));
                store.select((store.current.get_untracked() + 1).min(max.saturating_sub(1)));
            }
            (_, "ArrowUp" | "ArrowLeft") if idle => {
                store.select(store.current.get_untracked().saturating_sub(1));
            }
            _ => {}
        }
    });

    view! {
        <ClipsRail store=store />
        <TakesRail store=store />
        <main>
            <StageHeader store=store />
            <Stage store=store />
            <FooterBar store=store />
        </main>
        <div id="countdown">{move || store.countdown.get()}</div>
    }
}

#[component]
fn ClipsRail(store: Store) -> impl IntoView {
    view! {
        <aside class="clips">
            <header>
                <h1>"videoeditor · recorder"</h1>
                <div class="ep">{move || store.episode.with(|e| e.as_ref().map(|e| e.title.clone()).unwrap_or_default())}</div>
            </header>
            <div id="clipList">
                {move || {
                    store
                        .episode
                        .get()
                        .map(|e| {
                            e.clips
                                .iter()
                                .enumerate()
                                .map(|(i, c)| {
                                    let fits = c
                                        .take_duration
                                        .map(|d| d / c.tempo <= c.window + 0.005)
                                        .unwrap_or(false);
                                    let dot = match c.take_duration {
                                        None => "dot",
                                        Some(_) if fits => "dot has-take",
                                        Some(_) => "dot too-long",
                                    };
                                    let name = format!("{} / {}", c.scene, c.clip);
                                    let len = c.take_duration.map(|d| format!("{}s", fmt2(d))).unwrap_or("—".into());
                                    view! {
                                        <div
                                            class="clip-row"
                                            class:active=move || store.current.get() == i
                                            on:click=move |_| {
                                                if matches!(store.mode.get_untracked(), Mode::Idle | Mode::Review) {
                                                    store.select(i);
                                                }
                                            }
                                        >
                                            <div class=dot></div>
                                            <div class="name">{name}</div>
                                            <div class="len">{len}</div>
                                        </div>
                                    }
                                })
                                .collect_view()
                        })
                }}
            </div>
        </aside>
    }
}

/// One-line comparison stats from a take's saved coach report.
fn take_meta(r: &Review) -> String {
    let mut m = Vec::new();
    if let Some(acc) = r.accuracy_pct {
        m.push(format!("{acc:.0}% script"));
    }
    if let Some(wps) = r.wps {
        m.push(format!("{wps:.1} w/s"));
    }
    m.push(format!("peak {:.0}dB", r.max_db));
    if r.clipped {
        m.push("🔴 clipped".into());
    }
    if !r.fits {
        m.push("⚠ long".into());
    }
    if !r.pauses.is_empty() {
        m.push(format!(
            "💀 {} pause{}",
            r.pauses.len(),
            if r.pauses.len() > 1 { "s" } else { "" }
        ));
    }
    m.join(" · ")
}

#[component]
fn TakesRail(store: Store) -> impl IntoView {
    view! {
        <aside class="rail">
            <header>
                <h1>"takes · newest first"</h1>
                <div class="ep">"▶ audition · approve → becomes the clip"</div>
            </header>
            <div id="takesList">
                {move || {
                    let Some(c) = store.current_clip() else {
                        return Vec::new().into_iter().collect_view();
                    };
                    store
                        .takes
                        .get()
                        .into_iter()
                        .map(|t| {
                            let key = format!("{}/{}", c.id, t.file);
                            let url = format!("/audio/takes/{}/{}", c.id, t.file);
                            let clip_id = c.id.clone();
                            view! { <TakeRow store=store take=t key=key url=url clip_id=clip_id /> }
                        })
                        .collect_view()
                }}
            </div>
            <div
                class="rail-empty"
                style:display=move || if store.takes.with(|t| t.is_empty()) { "block" } else { "none" }
            >
                "no takes yet — every recording lands here, kept or not"
            </div>
            <audio id="audition" style="display:none" on:ended=move |_| store.playing.set(None)></audio>
        </aside>
    }
}

#[component]
fn TakeRow(
    store: Store,
    take: TakeInfo,
    key: String,
    url: String,
    clip_id: String,
) -> impl IntoView {
    let file = take.file.clone();
    let name = file.trim_end_matches(".mp3").to_string();
    let meta = take.review.as_ref().map(take_meta).unwrap_or_default();
    let coaching = take
        .review
        .as_ref()
        .map(|r| r.coaching.clone())
        .unwrap_or_default();
    let transcript = take.review.as_ref().and_then(|r| r.transcript.clone());
    let has_review = take.review.is_some();
    let approved = take.approved;
    let key_open = key.clone();
    let key_click = key.clone();
    let url_play = url.clone();
    let file_approve = file.clone();

    view! {
        <div
            class="take-row"
            class:approved=approved
            class:open=move || store.expanded.get().as_deref() == Some(key_open.as_str())
            on:click=move |_| {
                // the whole row is the disclosure control — one open at a time
                let now_open = store.expanded.get_untracked().as_deref() == Some(key_click.as_str());
                store.expanded.set(if now_open { None } else { Some(key_click.clone()) });
            }
        >
            <div class="top">
                <span class="chev">"›"</span>
                <span class="file">{name}</span>
                <span class="dur">{format!("{}s", fmt2(take.duration))}</span>
                <button
                    class="play"
                    on:click=move |e| {
                        e.stop_propagation();
                        let playing_this = store.playing.get_untracked().as_deref() == Some(url_play.as_str());
                        store.playing.set(if playing_this { None } else { Some(url_play.clone()) });
                    }
                >
                    {move || if store.playing.get().as_deref() == Some(url.as_str()) { "⏸" } else { "▶" }}
                </button>
                {if approved {
                    view! { <span class="badge">"✓"</span> }.into_any()
                } else {
                    view! {
                        <button
                            class="approve"
                            on:click=move |e| {
                                e.stop_propagation();
                                let id = clip_id.clone();
                                let file = file_approve.clone();
                                spawn_local(async move {
                                    match api::approve(&id, &file).await {
                                        Ok(info) => {
                                            apply_take_response(store, &id, &info);
                                            store.status.set(format!(
                                                "approved {file} → {id}.mp3{}{}",
                                                if info.fits { " ✓ fits".to_string() } else { format!(" ⚠ over the {}s window", fmt2(info.window)) },
                                                if info.warnings.is_empty() { String::new() } else { format!(" · {} timeline warning(s)", info.warnings.len()) },
                                            ));
                                            store.takes_reload.update(|n| *n += 1);
                                        }
                                        Err(e) => store.status.set(format!("approve failed: {e}")),
                                    }
                                });
                            }
                        >
                            "approve"
                        </button>
                    }
                    .into_any()
                }}
            </div>
            {(!meta.is_empty()).then(|| view! { <div class="meta">{meta}</div> })}
            <div class="take-detail">
                <div class="detail-inner">
                    {if has_review {
                        view! {
                            <ul class="coaching">
                                {coaching.into_iter().map(|n| view! { <li>{n}</li> }).collect_view()}
                            </ul>
                            {transcript.map(|t| view! { <div class="transcript">{format!("heard: “{t}”")}</div> })}
                        }
                        .into_any()
                    } else {
                        view! { <div class="no-review">"no analysis saved for this take"</div> }.into_any()
                    }}
                </div>
            </div>
        </div>
    }
}

#[component]
fn StageHeader(store: Store) -> impl IntoView {
    view! {
        <header>
            <div class="which" id="which">
                {move || store.current_clip().map(|c| format!("{} / {}", c.scene, c.clip)).unwrap_or("—".into())}
            </div>
            <div class="window" id="windowInfo">
                {move || {
                    store
                        .current_clip()
                        .map(|c| {
                            format!(
                                "window {}s at tempo {}{}",
                                fmt2(c.window),
                                c.tempo,
                                match c.take_duration {
                                    Some(d) => format!(" · current take {}s", fmt2(d)),
                                    None => " · no take yet".into(),
                                },
                            )
                        })
                        .unwrap_or_default()
                }}
            </div>
            <select
                id="micSelect"
                title="input device"
                on:change=move |e| {
                    let id = event_target_value(&e);
                    spawn_local(async move {
                        match audio::init_mic(Some(id)).await {
                            Ok((devices, active)) => {
                                store.devices.set(devices.into_iter().map(|d| (d.id, d.label)).collect());
                                store.active_device.set(active);
                            }
                            Err(e) => store.status.set(format!("mic switch failed: {e}")),
                        }
                    });
                }
            >
                {move || {
                    let active = store.active_device.get();
                    store
                        .devices
                        .get()
                        .into_iter()
                        .map(|(id, label)| {
                            let selected = id == active;
                            view! { <option value=id selected=selected>{label}</option> }
                        })
                        .collect_view()
                }}
            </select>
        </header>
    }
}

#[component]
fn Stage(store: Store) -> impl IntoView {
    view! {
        <div id="stage">
            <div class="inner">
                <div id="promptText">
                    {move || store.current_clip().map(|c| c.text).unwrap_or("loading…".into())}
                </div>
                <ReviewCard store=store />
            </div>
        </div>
    }
}

#[component]
fn ReviewCard(store: Store) -> impl IntoView {
    view! {
        <div id="reviewCard">
            <div class="hdr" id="coachHdr">
                {move || match (store.review.get(), store.review_error.get()) {
                    (Some(r), _) => {
                        let acc = r.accuracy_pct.map(|a| format!(" · script {a:.0}%")).unwrap_or_default();
                        let pace = r.wps.map(|w| format!(" · {w:.1} w/s")).unwrap_or_default();
                        view! {
                            "COACH · take " {format!("{:03}", r.take)} " · "
                            <b>{format!("{}s", fmt2(r.duration))}</b>
                            {format!(" / {}s window{acc}{pace} · peak {:.1} dB", fmt2(r.window), r.max_db)}
                        }
                        .into_any()
                    }
                    (None, Some(e)) => view! { "COACH · unavailable (" {e} ")" }.into_any(),
                    (None, None) => view! { "COACH · analyzing take…" }.into_any(),
                }}
            </div>
            <ul id="coachNotes">
                {move || {
                    store
                        .review
                        .get()
                        .map(|r| r.coaching.into_iter().map(|n| view! { <li>{n}</li> }).collect_view())
                }}
            </ul>
            <div id="transcript">
                {move || {
                    store
                        .review
                        .get()
                        .and_then(|r| r.transcript)
                        .map(|t| format!("heard: “{t}”"))
                }}
            </div>
            <div class="review-actions">
                <audio id="player" controls></audio>
                <button id="keepBtn" on:click=move |_| keep_take(store)>"keep (enter)"</button>
                <button id="retakeBtn" on:click=move |_| retake(store)>"retake (r)"</button>
            </div>
        </div>
    }
}

#[component]
fn FooterBar(store: Store) -> impl IntoView {
    let limit = move || {
        store
            .current_clip()
            .map(|c| c.window * c.tempo)
            .unwrap_or(0.0)
    };
    view! {
        <footer>
            <div id="meter">
                <div class="chan">
                    <span class="lbl">"L"</span>
                    <div class="track">
                        <div
                            id="meterL"
                            class={move || if store.meter.get().0 > 0.85 { "fill hot" } else { "fill" }}
                            style:width=move || format!("{}%", (store.meter.get().0 * 130.0).min(100.0))
                        ></div>
                    </div>
                </div>
                <div class="chan">
                    <span class="lbl">"R"</span>
                    <div class="track">
                        <div
                            id="meterR"
                            class={move || if store.meter.get().1 > 0.85 { "fill hot" } else { "fill" }}
                            style:width=move || format!("{}%", (store.meter.get().1 * 130.0).min(100.0))
                        ></div>
                    </div>
                </div>
            </div>
            <div class="controls">
                <button id="recBtn" title="record / stop (space)" on:click=move |_| toggle_record(store)>"⏺"</button>
                <div id="timer" class={move || if store.elapsed.get() > limit() && limit() > 0.0 { "over" } else { "" }}>
                    {move || fmt2(store.elapsed.get())}
                    <span class="lim">{move || format!(" / {}", fmt2(limit()))}</span>
                </div>
                <div id="status">{move || store.status.get()}</div>
            </div>
            <div class="hints">
                <kbd>"space"</kbd> " record / stop · " <kbd>"enter"</kbd> " keep · " <kbd>"r"</kbd> " retake · "
                <kbd>"↑"</kbd><kbd>"↓"</kbd> " switch clip · raw stereo mic (no browser processing) — watch the meters, stay out of the red · every take is archived in audio/takes/, kept or not"
            </div>
        </footer>
    }
}
