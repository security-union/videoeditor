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
        (Post, path) if path.starts_with("/api/take/") => {
            let id = sanitize_id(&path["/api/take/".len()..])?;
            let mut body = Vec::new();
            request.as_reader().read_to_end(&mut body)?;
            let mime = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("content-type"))
                .map(|h| h.value.as_str().to_string())
                .unwrap_or_default();
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
