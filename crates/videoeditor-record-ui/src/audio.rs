//! Browser audio interop: mic capture, L/R analysers, MediaRecorder, and
//! the timer/raf plumbing. JS handles are !Send, so they live in
//! thread-locals (Leptos CSR is single-threaded).

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AnalyserNode, AudioContext, Blob, BlobEvent, BlobPropertyBag, MediaRecorder,
    MediaRecorderOptions, MediaStream, MediaStreamConstraints, MediaStreamTrack,
};

/// A live MediaRecorder and the event closures keeping its handlers alive —
/// replaced wholesale each recording, which drops the previous set.
type RecorderSlot = (
    MediaRecorder,
    Closure<dyn FnMut(BlobEvent)>,
    Closure<dyn FnMut(web_sys::Event)>,
);
type IntervalSlot = (i32, Closure<dyn FnMut()>);
type RafClosure = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

thread_local! {
    static STREAM: RefCell<Option<MediaStream>> = const { RefCell::new(None) };
    static AUDIO_CTX: RefCell<Option<AudioContext>> = const { RefCell::new(None) };
    static ANALYSERS: RefCell<Option<(AnalyserNode, AnalyserNode)>> = const { RefCell::new(None) };
    static RECORDER: RefCell<Option<RecorderSlot>> = const { RefCell::new(None) };
    /// Countdown/record-timer interval closures, keyed by interval id.
    static INTERVALS: RefCell<Vec<IntervalSlot>> = const { RefCell::new(Vec::new()) };
    static METERS_RUNNING: RefCell<bool> = const { RefCell::new(false) };
}

fn err(v: JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}

fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

#[derive(Clone, Debug, PartialEq)]
pub struct MicDevice {
    pub id: String,
    pub label: String,
}

/// (Re)open the mic and wire the stereo analyser pair. Returns the input
/// device list and the id of the active device.
pub async fn init_mic(device_id: Option<String>) -> Result<(Vec<MicDevice>, String), String> {
    // stop the previous stream's tracks before opening a new one
    STREAM.with(|s| {
        if let Some(old) = s.borrow_mut().take() {
            for t in old.get_tracks().iter() {
                if let Ok(track) = t.dyn_into::<MediaStreamTrack>() {
                    track.stop();
                }
            }
        }
    });

    // { audio: { channelCount: 2, echoCancellation: false, ... } } —
    // raw stereo voice, no call-style processing
    let audio = js_sys::Object::new();
    let set = |k: &str, v: JsValue| {
        js_sys::Reflect::set(&audio, &k.into(), &v).expect("object set");
    };
    set("channelCount", 2.into());
    set("echoCancellation", false.into());
    set("noiseSuppression", false.into());
    set("autoGainControl", false.into());
    if let Some(id) = &device_id {
        let exact = js_sys::Object::new();
        js_sys::Reflect::set(&exact, &"exact".into(), &id.as_str().into()).expect("object set");
        set("deviceId", exact.into());
    }
    let constraints = MediaStreamConstraints::new();
    constraints.set_audio(&audio.into());

    let devices_api = window().navigator().media_devices().map_err(err)?;
    let stream: MediaStream = JsFuture::from(
        devices_api
            .get_user_media_with_constraints(&constraints)
            .map_err(err)?,
    )
    .await
    .map_err(err)?
    .dyn_into()
    .map_err(|_| "getUserMedia returned a non-stream")?;

    let ctx = AUDIO_CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_none() {
            *c = AudioContext::new().ok();
        }
        c.clone()
    });
    let ctx = ctx.ok_or("no AudioContext")?;
    let splitter = ctx
        .create_channel_splitter_with_number_of_outputs(2)
        .map_err(err)?;
    let left = ctx.create_analyser().map_err(err)?;
    let right = ctx.create_analyser().map_err(err)?;
    left.set_fft_size(1024);
    right.set_fft_size(1024);
    let source = ctx.create_media_stream_source(&stream).map_err(err)?;
    source.connect_with_audio_node(&splitter).map_err(err)?;
    splitter
        .connect_with_audio_node_and_output(&left, 0)
        .map_err(err)?;
    splitter
        .connect_with_audio_node_and_output(&right, 1)
        .map_err(err)?;

    let active = stream
        .get_audio_tracks()
        .get(0)
        .dyn_into::<MediaStreamTrack>()
        .ok()
        .map(|t| {
            js_sys::Reflect::get(&t.get_settings(), &"deviceId".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    STREAM.with(|s| *s.borrow_mut() = Some(stream));
    ANALYSERS.with(|a| *a.borrow_mut() = Some((left, right)));

    // enumerate AFTER permission is granted so labels are populated
    let list = JsFuture::from(devices_api.enumerate_devices().map_err(err)?)
        .await
        .map_err(err)?;
    let mut devices = Vec::new();
    for d in js_sys::Array::from(&list).iter() {
        if let Ok(info) = d.dyn_into::<web_sys::MediaDeviceInfo>() {
            if info.kind() == web_sys::MediaDeviceKind::Audioinput {
                let label = info.label();
                devices.push(MicDevice {
                    id: info.device_id(),
                    label: if label.is_empty() {
                        "microphone".into()
                    } else {
                        label
                    },
                });
            }
        }
    }
    Ok((devices, active))
}

/// Start the requestAnimationFrame meter loop (idempotent). Calls
/// `on_levels(peak_left, peak_right)` with 0..=1 peaks every frame.
pub fn start_meters(on_levels: impl Fn(f64, f64) + 'static) {
    let already = METERS_RUNNING.with(|m| std::mem::replace(&mut *m.borrow_mut(), true));
    if already {
        return;
    }
    let mut buf = [0.0f32; 1024];
    let cb: RafClosure = Rc::new(RefCell::new(None));
    let cb2 = cb.clone();
    *cb.borrow_mut() = Some(Closure::new(move || {
        let peaks = ANALYSERS.with(|a| {
            a.borrow().as_ref().map(|(l, r)| {
                let mut peak = |an: &AnalyserNode| {
                    an.get_float_time_domain_data(&mut buf);
                    buf.iter().fold(0.0f32, |m, v| m.max(v.abs())) as f64
                };
                (peak(l), peak(r))
            })
        });
        if let Some((l, r)) = peaks {
            on_levels(l, r);
        }
        raf(cb2.borrow().as_ref().expect("meter closure"));
    }));
    raf(cb.borrow().as_ref().expect("meter closure"));
}

fn raf(c: &Closure<dyn FnMut()>) {
    window()
        .request_animation_frame(c.as_ref().unchecked_ref())
        .expect("requestAnimationFrame");
}

/// Start recording the current mic stream; `on_stop` receives the full
/// take as a single Blob when recording ends.
pub fn start_recording(on_stop: impl Fn(Blob) + 'static) -> Result<(), String> {
    let stream = STREAM
        .with(|s| s.borrow().clone())
        .ok_or("mic not initialized")?;
    let mime = if MediaRecorder::is_type_supported("audio/webm;codecs=opus") {
        "audio/webm;codecs=opus"
    } else {
        "audio/mp4"
    };
    let opts = MediaRecorderOptions::new();
    opts.set_mime_type(mime);
    let rec = MediaRecorder::new_with_media_stream_and_media_recorder_options(&stream, &opts)
        .map_err(err)?;

    let chunks: Rc<RefCell<Vec<JsValue>>> = Rc::new(RefCell::new(Vec::new()));
    let chunks2 = chunks.clone();
    let on_data = Closure::new(move |e: BlobEvent| {
        if let Some(b) = e.data() {
            chunks2.borrow_mut().push(b.into());
        }
    });
    rec.set_ondataavailable(Some(on_data.as_ref().unchecked_ref()));

    let mime_owned = mime.to_string();
    let on_stop_cl = Closure::new(move |_: web_sys::Event| {
        let parts = js_sys::Array::new();
        for c in chunks.borrow().iter() {
            parts.push(c);
        }
        let bag = BlobPropertyBag::new();
        bag.set_type(&mime_owned);
        if let Ok(blob) = Blob::new_with_blob_sequence_and_options(&parts, &bag) {
            on_stop(blob);
        }
    });
    rec.set_onstop(Some(on_stop_cl.as_ref().unchecked_ref()));

    rec.start().map_err(err)?;
    RECORDER.with(|r| *r.borrow_mut() = Some((rec, on_data, on_stop_cl)));
    Ok(())
}

pub fn stop_recording() {
    RECORDER.with(|r| {
        if let Some((rec, ..)) = r.borrow().as_ref() {
            let _ = rec.stop();
        }
    });
}

/// setInterval that owns its closure (cleared + dropped via [`clear_interval`]).
pub fn set_interval(ms: i32, f: impl FnMut() + 'static) -> i32 {
    let cb: Closure<dyn FnMut()> = Closure::new(f);
    let id = window()
        .set_interval_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), ms)
        .expect("setInterval");
    INTERVALS.with(|i| i.borrow_mut().push((id, cb)));
    id
}

pub fn clear_interval(id: i32) {
    window().clear_interval_with_handle(id);
    INTERVALS.with(|i| i.borrow_mut().retain(|(iid, _)| *iid != id));
}

pub fn now_ms() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or_default()
}
