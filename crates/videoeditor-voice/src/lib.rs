//! Voice I/O: text-to-speech (one MP3 per `[CLIP:]`, name-keyed, plus a
//! `clips.json` manifest with probed durations) and speech-to-text for
//! reference-video transcription and take coaching.
//!
//! Two backends each, local by default:
//!
//! - TTS: **piper** (default — a piper voice via sherpa-onnx, no API key) or
//!   **elevenlabs**. Picked by frontmatter `tts:`, then `VIDEOEDITOR_TTS`.
//! - STT: **whisper** (default — whisper.cpp, no API key) or **elevenlabs**
//!   (Scribe). Picked by `VIDEOEDITOR_STT`.
//!
//! Both STT backends return the same transcript shape:
//! `{"text": …, "words": [{"type":"word","text","start","end"}, …]}`.

mod elevenlabs;
mod piper;
mod whisper;

pub use elevenlabs::api_key;

use anyhow::{Result, bail};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use videoeditor_timeline::{ClipInfo, Episode, Meta};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TtsBackend {
    Piper,
    ElevenLabs,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SttBackend {
    Whisper,
    ElevenLabs,
}

/// Resolve the TTS backend: frontmatter `tts:` → `VIDEOEDITOR_TTS` → piper.
pub fn tts_backend(meta: &Meta) -> Result<TtsBackend> {
    let name = meta
        .tts
        .clone()
        .or_else(|| env::var("VIDEOEDITOR_TTS").ok())
        .unwrap_or_else(|| "piper".to_string());
    match name.as_str() {
        "piper" => Ok(TtsBackend::Piper),
        "elevenlabs" => Ok(TtsBackend::ElevenLabs),
        other => bail!("unknown TTS backend {other:?} — use \"piper\" or \"elevenlabs\""),
    }
}

/// Resolve the STT backend: `VIDEOEDITOR_STT` → whisper.
pub fn stt_backend() -> Result<SttBackend> {
    match env::var("VIDEOEDITOR_STT").as_deref() {
        Err(_) | Ok("whisper") => Ok(SttBackend::Whisper),
        Ok("elevenlabs") => Ok(SttBackend::ElevenLabs),
        Ok(other) => bail!("unknown STT backend {other:?} — use \"whisper\" or \"elevenlabs\""),
    }
}

/// Human-readable name of the resolved STT backend (for progress lines).
pub fn stt_name() -> &'static str {
    match stt_backend() {
        Ok(SttBackend::Whisper) => "whisper",
        Ok(SttBackend::ElevenLabs) => "elevenlabs",
        Err(_) => "unknown",
    }
}

/// Can `stt()` run right now (binary + model present, or API key set)?
pub fn stt_available() -> bool {
    match stt_backend() {
        Ok(SttBackend::Whisper) => whisper::available(),
        Ok(SttBackend::ElevenLabs) => api_key().is_ok(),
        Err(_) => false,
    }
}

/// Transcribe an audio file (word-level timestamps) with the resolved backend.
pub fn stt(audio: &Path) -> Result<Value> {
    match stt_backend()? {
        SttBackend::Whisper => whisper::stt(audio),
        SttBackend::ElevenLabs => elevenlabs::stt(audio),
    }
}

/// Generate narration clips for every `[CLIP:]` (skips existing files unless
/// `force`) and write the `audio/clips.json` manifest.
pub fn run(ep: &Episode, only_clip: Option<&str>, force: bool) -> Result<()> {
    let clips_dir = ep.root.join("audio/clips");
    fs::create_dir_all(&clips_dir)?;

    let has_narration = ep.scenes.iter().any(|s| !s.clips.is_empty());
    if !has_narration {
        println!("tts: no clips in script, nothing to do");
        return Ok(());
    }

    let backend = tts_backend(&ep.meta)?;
    println!(
        "tts: backend {}",
        match backend {
            TtsBackend::Piper => "piper (local)",
            TtsBackend::ElevenLabs => "elevenlabs",
        }
    );
    let mut manifest: Vec<ClipInfo> = Vec::new();

    for scene in &ep.scenes {
        for clip in &scene.clips {
            let id = format!("{}__{}", scene.name, clip.name);
            let path = clips_dir.join(format!("{id}.mp3"));
            let selected = only_clip.is_none_or(|c| c == id || c == clip.name);
            if selected && (force || !path.exists()) {
                if clip.text.is_empty() {
                    bail!("clip {id} has no narration text");
                }
                println!("tts: {id} ({} chars)", clip.text.len());
                match backend {
                    TtsBackend::Piper => piper::synth(&clip.text, &path)?,
                    TtsBackend::ElevenLabs => elevenlabs::synth(&ep.meta, &clip.text, &path)?,
                }
            }
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

    let manifest_path = ep.root.join("audio/clips.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
    println!(
        "tts: wrote {} ({} clips)",
        manifest_path.display(),
        manifest.len()
    );
    for w in ep.fit_check(&manifest) {
        println!("tts: ⚠ {w}");
    }
    Ok(())
}

/// Is `bin` runnable — an existing path, or a name found on `$PATH`?
pub(crate) fn find_in_path(bin: &str) -> bool {
    let p = Path::new(bin);
    if p.components().count() > 1 {
        return p.exists();
    }
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|dir| dir.join(bin).exists()))
        .unwrap_or(false)
}
