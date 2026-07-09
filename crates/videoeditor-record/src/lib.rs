//! Local web recorder: `videoeditor record <episode>` serves a one-page
//! teleprompter + mic-capture UI on localhost, and every kept take lands
//! directly in the episode (`audio/clips/<scene>__<clip>.mp3` + manifest),
//! ready for the normal render/assemble pipeline.
//!
//! Capture happens in the browser (getUserMedia/MediaRecorder) because
//! cross-platform native audio capture is a swamp — device pickers,
//! permission prompts, and live metering are already solved there.
//! localhost counts as a secure context, so the mic works without TLS.
//! The browser uploads webm/opus (Chrome) or mp4/aac (Safari); ffmpeg
//! transcodes to the same mp3 44.1 kHz mono the TTS path produces.
//!
//! Every kept take is archived under `audio/takes/<id>/` before the
//! current clip is replaced, so no take is ever lost to a retake.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::Path;
use videoeditor_timeline::{ClipInfo, Episode};

const INDEX_HTML: &str = include_str!("index.html");

#[derive(Serialize)]
struct ClipView {
    id: String,
    scene: String,
    clip: String,
    text: String,
    at: f64,
    tempo: f64,
    scene_duration: f64,
    /// Seconds of playback the scene has room for (scene duration − clip at).
    window: f64,
    /// Measured duration of the current take, if one exists.
    take_duration: Option<f64>,
}

#[derive(Serialize)]
struct EpisodeView {
    title: String,
    clips: Vec<ClipView>,
}

#[derive(Serialize)]
struct TakeResponse {
    duration: f64,
    window: f64,
    fits: bool,
    warnings: Vec<String>,
}

/// Serve the recorder for this episode until Ctrl+C.
pub fn run(ep: &Episode, port: u16, open_browser: bool) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let server =
        tiny_http::Server::http(&addr).map_err(|e| anyhow::anyhow!("binding {addr}: {e}"))?;
    let url = format!("http://{addr}");
    println!("record: teleprompter at {url}  (Ctrl+C to stop)");
    println!(
        "record: kept takes replace audio/clips/<id>.mp3; previous audio is archived in audio/takes/"
    );
    if open_browser {
        let _ = open_url(&url);
    }

    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();
        let resp = route(ep, &method, &url, &mut request);
        let _ = match resp {
            Ok(r) => request.respond(r),
            Err(e) => request.respond(
                tiny_http::Response::from_string(format!("error: {e:#}")).with_status_code(500),
            ),
        };
    }
    Ok(())
}

fn route(
    ep: &Episode,
    method: &tiny_http::Method,
    url: &str,
    request: &mut tiny_http::Request,
) -> Result<tiny_http::Response<std::io::Cursor<Vec<u8>>>> {
    use tiny_http::Method::{Get, Post};
    match (method, url) {
        (Get, "/") => Ok(html(INDEX_HTML)),
        (Get, "/api/episode") => Ok(json(&episode_view(ep)?)),
        (Get, path) if path.starts_with("/audio/") => {
            let id = sanitize_id(&path["/audio/".len()..])?;
            let file = ep.root.join(format!("audio/clips/{id}.mp3"));
            let bytes = fs::read(&file).with_context(|| format!("no take for {id}"))?;
            Ok(tiny_http::Response::from_data(bytes)
                .with_header(header("content-type", "audio/mpeg")))
        }
        (Post, path) if path.starts_with("/api/review/") => {
            let id = sanitize_id(&path["/api/review/".len()..])?;
            let mut body = Vec::new();
            request.as_reader().read_to_end(&mut body)?;
            let mime = content_type(request);
            Ok(json(&review_take(ep, &id, &body, &mime)?))
        }
        (Post, path) if path.starts_with("/api/take/") => {
            let id = sanitize_id(&path["/api/take/".len()..])?;
            let mut body = Vec::new();
            request.as_reader().read_to_end(&mut body)?;
            let mime = content_type(request);
            let resp = save_take(ep, &id, &body, &mime)?;
            println!(
                "record: {id} take kept ({:.2}s / window {:.2}s){}",
                resp.duration,
                resp.window,
                if resp.fits { "" } else { "  ⚠ too long" }
            );
            Ok(json(&resp))
        }
        _ => Ok(tiny_http::Response::from_string("not found").with_status_code(404)),
    }
}

fn episode_view(ep: &Episode) -> Result<EpisodeView> {
    let mut clips = Vec::new();
    for scene in &ep.scenes {
        for clip in &scene.clips {
            let id = format!("{}__{}", scene.name, clip.name);
            let file = ep.root.join(format!("audio/clips/{id}.mp3"));
            let take_duration = file
                .exists()
                .then(|| videoeditor_media::ffprobe_duration(&file))
                .transpose()?;
            let at = clip.at.unwrap_or(0.0);
            clips.push(ClipView {
                id,
                scene: scene.name.clone(),
                clip: clip.name.clone(),
                text: clip.text.clone(),
                at,
                tempo: clip.tempo,
                scene_duration: scene.duration,
                window: scene.duration - at,
                take_duration,
            });
        }
    }
    Ok(EpisodeView {
        title: ep.meta.title.clone(),
        clips,
    })
}

/// Transcode an uploaded take to the pipeline's mp3 format, archive what it
/// replaces, refresh the manifest, and fit-check the result.
fn save_take(ep: &Episode, id: &str, body: &[u8], mime: &str) -> Result<TakeResponse> {
    let (scene, clip) = ep
        .scenes
        .iter()
        .flat_map(|s| s.clips.iter().map(move |c| (s, c)))
        .find(|(s, c)| format!("{}__{}", s.name, c.name) == id)
        .with_context(|| format!("unknown clip id {id}"))?;

    let clips_dir = ep.root.join("audio/clips");
    let takes_dir = ep.root.join("audio/takes").join(id);
    fs::create_dir_all(&clips_dir)?;
    fs::create_dir_all(&takes_dir)?;

    // raw upload → temp file (extension helps ffmpeg pick a demuxer)
    let ext = if mime.contains("mp4") { "mp4" } else { "webm" };
    let raw = takes_dir.join(format!("upload.{ext}"));
    fs::write(&raw, body)?;

    // transcode to the exact format the TTS path produces
    let take = takes_dir.join(format!("take_{:03}.mp3", next_take_number(&takes_dir)));
    videoeditor_media::ffmpeg(&[
        "-y",
        "-i",
        raw.to_str().context("path")?,
        "-ac",
        "1",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        take.to_str().context("path")?,
    ])?;
    fs::remove_file(&raw).ok();

    // archive whatever the kept take replaces, then promote the new one
    let current = clips_dir.join(format!("{id}.mp3"));
    if current.exists() {
        let n = next_take_number(&takes_dir);
        fs::rename(&current, takes_dir.join(format!("replaced_{n:03}.mp3")))?;
    }
    fs::copy(&take, &current)?;

    let manifest = rebuild_manifest(ep)?;
    fs::write(
        ep.root.join("audio/clips.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    let duration = videoeditor_media::ffprobe_duration(&current)?;
    let window = scene.duration - clip.at.unwrap_or(0.0);
    Ok(TakeResponse {
        duration,
        window,
        fits: duration / clip.tempo <= window,
        warnings: ep.fit_check(&manifest),
    })
}

#[derive(Serialize)]
struct Pause {
    at: f64,
    len: f64,
}

/// The coach's report on one (not-yet-kept) take.
#[derive(Serialize)]
struct Review {
    duration: f64,
    window: f64,
    fits: bool,
    mean_db: f64,
    max_db: f64,
    clipped: bool,
    /// None = no ELEVENLABS_API_KEY; level/timing coaching still runs.
    transcript: Option<String>,
    accuracy_pct: Option<f64>,
    missing: Vec<String>,
    added: Vec<String>,
    wps: Option<f64>,
    pauses: Vec<Pause>,
    events: Vec<String>,
    coaching: Vec<String>,
}

/// Analyze a pending take WITHOUT keeping it: local level metrics via
/// ffmpeg always; script-accuracy / pacing / dead-air / background-noise
/// coaching via ElevenLabs Scribe when a key is present.
fn review_take(ep: &Episode, id: &str, body: &[u8], mime: &str) -> Result<Review> {
    let (scene, clip) = ep
        .scenes
        .iter()
        .flat_map(|s| s.clips.iter().map(move |c| (s, c)))
        .find(|(s, c)| format!("{}__{}", s.name, c.name) == id)
        .with_context(|| format!("unknown clip id {id}"))?;

    let dir = ep.root.join("audio/takes").join(id);
    fs::create_dir_all(&dir)?;
    let ext = if mime.contains("mp4") { "mp4" } else { "webm" };
    let raw = dir.join(format!("review.{ext}"));
    let mp3 = dir.join("review.mp3");
    fs::write(&raw, body)?;
    videoeditor_media::ffmpeg(&[
        "-y",
        "-i",
        raw.to_str().context("path")?,
        "-ac",
        "1",
        "-ar",
        "44100",
        "-b:a",
        "128k",
        mp3.to_str().context("path")?,
    ])?;

    let duration = videoeditor_media::ffprobe_duration(&mp3)?;
    let window = scene.duration - clip.at.unwrap_or(0.0);
    let fits = duration / clip.tempo <= window;
    let (mean_db, max_db) = audio_levels(&mp3)?;
    let clipped = max_db > -0.2;

    // Scribe is optional — no key, no transcript coaching.
    let stt = if videoeditor_voice::api_key().is_ok() {
        Some(videoeditor_voice::stt(&mp3).context("ElevenLabs STT")?)
    } else {
        None
    };
    fs::remove_file(&raw).ok();
    fs::remove_file(&mp3).ok();

    let mut review = Review {
        duration,
        window,
        fits,
        mean_db,
        max_db,
        clipped,
        transcript: None,
        accuracy_pct: None,
        missing: vec![],
        added: vec![],
        wps: None,
        pauses: vec![],
        events: vec![],
        coaching: vec![],
    };

    if let Some(t) = &stt {
        review.transcript = t["text"].as_str().map(|s| s.to_string());
        let spoken_words: Vec<(f64, f64, String)> = t["words"]
            .as_array()
            .map(|ws| {
                ws.iter()
                    .filter(|w| w["type"] == "word")
                    .map(|w| {
                        (
                            w["start"].as_f64().unwrap_or(0.0),
                            w["end"].as_f64().unwrap_or(0.0),
                            w["text"].as_str().unwrap_or("").to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        review.events = t["words"]
            .as_array()
            .map(|ws| {
                ws.iter()
                    .filter(|w| w["type"] == "audio_event")
                    .filter_map(|w| w["text"].as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let script = normalize_words(&clip.text);
        let spoken: Vec<String> = spoken_words
            .iter()
            .flat_map(|(_, _, w)| normalize_words(w))
            .collect();
        let (accuracy, missing, added) = script_diff(&script, &spoken);
        review.accuracy_pct = Some(accuracy * 100.0);
        review.missing = missing;
        review.added = added;
        if duration > 0.0 {
            review.wps = Some(spoken.len() as f64 / duration);
        }
        for pair in spoken_words.windows(2) {
            let gap = pair[1].0 - pair[0].1;
            if gap >= 0.9 {
                review.pauses.push(Pause {
                    at: pair[0].1,
                    len: gap,
                });
            }
        }
    }

    review.coaching = coach(&review);
    Ok(review)
}

/// Turn measurements into the feedback lines the UI shows.
fn coach(r: &Review) -> Vec<String> {
    let mut notes = Vec::new();
    if !r.fits {
        notes.push(format!(
            "⏱ runs {:.1}s against a {:.1}s window — tighten the read or the scene stretches",
            r.duration, r.window
        ));
    }
    if r.clipped {
        notes.push("🔴 clipping — back off the mic or lower the gain".into());
    } else if r.max_db < -12.0 {
        notes.push("🔉 quiet peak level — get closer to the mic".into());
    }
    if r.mean_db < -30.0 {
        notes.push("🔉 overall level is low — closer to the mic or raise input gain".into());
    }
    if let Some(acc) = r.accuracy_pct {
        if acc < 92.0 && !r.missing.is_empty() {
            notes.push(format!(
                "📜 dropped from the script: {}",
                r.missing.join(", ")
            ));
        }
        if r.added.len() > 2 {
            notes.push(format!("🗣 ad-libbed: {}", r.added.join(", ")));
        }
    }
    if let Some(wps) = r.wps {
        if wps > 3.9 {
            notes.push(format!(
                "🏃 {wps:.1} words/sec — racing; let the beats breathe"
            ));
        } else if wps < 2.2 {
            notes.push(format!(
                "🐢 {wps:.1} words/sec — dragging; bring the energy up"
            ));
        }
    }
    for p in &r.pauses {
        notes.push(format!(
            "💀 dead air at {:.1}s ({:.1}s) — intentional?",
            p.at, p.len
        ));
    }
    for e in &r.events {
        notes.push(format!("🎧 background sound picked up: {e}"));
    }
    if notes.is_empty() {
        notes.push("✅ clean take — levels good, script covered, pace on target. Ship it.".into());
    }
    notes
}

/// mean/max dBFS via ffmpeg volumedetect (stderr parse).
fn audio_levels(path: &Path) -> Result<(f64, f64)> {
    let out = std::process::Command::new("ffmpeg")
        .args([
            "-i",
            path.to_str().context("path")?,
            "-af",
            "volumedetect",
            "-f",
            "null",
            "-",
        ])
        .output()
        .context("running ffmpeg volumedetect")?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    let grab = |key: &str| -> f64 {
        stderr
            .lines()
            .find(|l| l.contains(key))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().trim_end_matches(" dB").parse().ok())
            .unwrap_or(0.0)
    };
    Ok((grab("mean_volume"), grab("max_volume")))
}

/// Lowercase alphanumeric+apostrophe tokens — the comparison currency for
/// script-vs-transcript diffing.
fn normalize_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// LCS word alignment → (accuracy vs script, missing words, added words).
fn script_diff(script: &[String], spoken: &[String]) -> (f64, Vec<String>, Vec<String>) {
    let (n, m) = (script.len(), spoken.len());
    if n == 0 {
        return (1.0, vec![], spoken.to_vec());
    }
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if script[i] == spoken[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let (mut missing, mut added) = (Vec::new(), Vec::new());
    while i < n && j < m {
        if script[i] == spoken[j] {
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            missing.push(script[i].clone());
            i += 1;
        } else {
            added.push(spoken[j].clone());
            j += 1;
        }
    }
    missing.extend(script[i..].iter().cloned());
    added.extend(spoken[j..].iter().cloned());
    (lcs[0][0] as f64 / n as f64, missing, added)
}

fn content_type(request: &tiny_http::Request) -> String {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv("content-type"))
        .map(|h| h.value.as_str().to_string())
        .unwrap_or_default()
}

/// Re-scan every clip file on disk so the manifest self-heals even if a
/// previous run (or a hand copy) left it stale.
fn rebuild_manifest(ep: &Episode) -> Result<Vec<ClipInfo>> {
    let mut manifest = Vec::new();
    for scene in &ep.scenes {
        for clip in &scene.clips {
            let id = format!("{}__{}", scene.name, clip.name);
            let path = ep.root.join(format!("audio/clips/{id}.mp3"));
            if path.exists() {
                manifest.push(ClipInfo {
                    scene: scene.name.clone(),
                    clip: clip.name.clone(),
                    file: format!("audio/clips/{id}.mp3"),
                    duration: videoeditor_media::ffprobe_duration(&path)?,
                });
            }
        }
    }
    Ok(manifest)
}

fn next_take_number(dir: &Path) -> u32 {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    // take_007.mp3 / replaced_012.mp3 → 7 / 12
                    let name = e.file_name().to_string_lossy().to_string();
                    let stem = name.strip_suffix(".mp3")?;
                    stem.rsplit('_').next()?.parse::<u32>().ok()
                })
                .max()
                .map(|n| n + 1)
                .unwrap_or(1)
        })
        .unwrap_or(1)
}

/// Clip ids come straight off the URL — allow only manifest-shaped names so
/// they can never traverse out of the episode dir.
fn sanitize_id(raw: &str) -> Result<String> {
    if raw.is_empty()
        || !raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid clip id {raw:?}");
    }
    Ok(raw.to_string())
}

fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let (cmd, args) = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let (cmd, args) = ("cmd", vec!["/c", "start", url]);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (cmd, args) = ("xdg-open", vec![url]);
    std::process::Command::new(cmd).args(args).spawn()?;
    Ok(())
}

fn html(body: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body)
        .with_header(header("content-type", "text/html; charset=utf-8"))
}

fn json<T: Serialize>(value: &T) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(serde_json::to_string(value).unwrap_or_default())
        .with_header(header("content-type", "application/json"))
}

fn header(field: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(field.as_bytes(), value.as_bytes()).expect("static header")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_accepts_manifest_ids_only() {
        assert_eq!(sanitize_id("title__hook").unwrap(), "title__hook");
        assert_eq!(sanitize_id("pass2__pass2").unwrap(), "pass2__pass2");
        assert!(sanitize_id("").is_err());
        assert!(sanitize_id("../../etc/passwd").is_err());
        assert!(sanitize_id("a/b").is_err());
        assert!(sanitize_id("a b").is_err());
    }

    #[test]
    fn script_diff_finds_missing_and_added_words() {
        let script = normalize_words("They hired the strictest security guard in software");
        let spoken = normalize_words("they hired the um strictest guard in software");
        let (acc, missing, added) = script_diff(&script, &spoken);
        assert!(acc > 0.85 && acc < 1.0);
        assert_eq!(missing, vec!["security"]);
        assert_eq!(added, vec!["um"]);
    }

    #[test]
    fn perfect_read_scores_full_accuracy() {
        let script = normalize_words("Zig in a trenchcoat.");
        let spoken = normalize_words("zig in a trenchcoat");
        let (acc, missing, added) = script_diff(&script, &spoken);
        assert_eq!(acc, 1.0);
        assert!(missing.is_empty() && added.is_empty());
    }

    #[test]
    fn coach_praises_a_clean_take() {
        let r = Review {
            duration: 7.0,
            window: 8.4,
            fits: true,
            mean_db: -18.0,
            max_db: -3.0,
            clipped: false,
            transcript: Some("x".into()),
            accuracy_pct: Some(100.0),
            missing: vec![],
            added: vec![],
            wps: Some(3.0),
            pauses: vec![],
            events: vec![],
            coaching: vec![],
        };
        let notes = coach(&r);
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("Ship it"));
    }

    #[test]
    fn coach_flags_clipping_overrun_and_dead_air() {
        let r = Review {
            duration: 10.0,
            window: 8.4,
            fits: false,
            mean_db: -35.0,
            max_db: 0.0,
            clipped: true,
            transcript: Some("x".into()),
            accuracy_pct: Some(80.0),
            missing: vec!["boomer".into()],
            added: vec![],
            wps: Some(4.5),
            pauses: vec![Pause { at: 3.0, len: 1.5 }],
            events: vec!["(cough)".into()],
            coaching: vec![],
        };
        let notes = coach(&r).join("\n");
        assert!(notes.contains("against a"));
        assert!(notes.contains("clipping"));
        assert!(notes.contains("boomer"));
        assert!(notes.contains("racing"));
        assert!(notes.contains("dead air"));
        assert!(notes.contains("cough"));
    }

    #[test]
    fn take_numbers_start_at_one_and_increment() {
        let dir = std::env::temp_dir().join(format!("ve-record-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(next_take_number(&dir), 1);
        fs::write(dir.join("take_001.mp3"), b"x").unwrap();
        assert_eq!(next_take_number(&dir), 2);
        fs::write(dir.join("replaced_007.mp3"), b"x").unwrap();
        assert_eq!(next_take_number(&dir), 8);
        fs::remove_dir_all(&dir).unwrap();
    }
}
