//! whisper.cpp local STT backend: `whisper-cli` over a pinned ggml model.
//! The nix package/dev shell put both on PATH/env; bare-cargo installs set
//! `WHISPER_BIN` / `WHISPER_MODEL` themselves.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn bin() -> String {
    env::var("WHISPER_BIN").unwrap_or_else(|_| "whisper-cli".to_string())
}

pub fn model() -> Result<PathBuf> {
    let m = env::var("WHISPER_MODEL").map(PathBuf::from).context(
        "set WHISPER_MODEL to a ggml whisper model (the nix install pins one; \
         otherwise download one with whisper-cpp-download-ggml-model, \
         or set VIDEOEDITOR_STT=elevenlabs)",
    )?;
    if !m.exists() {
        bail!(
            "WHISPER_MODEL points at {} which does not exist",
            m.display()
        );
    }
    Ok(m)
}

pub fn available() -> bool {
    model().is_ok() && crate::find_in_path(&bin())
}

/// Transcribe an audio file locally. Any input format — ffmpeg downmixes to
/// the 16 kHz mono wav whisper.cpp expects, then `whisper-cli` runs with
/// word-level segmentation (`--max-len 1 --split-on-word`).
pub fn stt(audio: &std::path::Path) -> Result<Value> {
    let model = model()?;
    let work = audio
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(format!(".whisper-{}", std::process::id()));
    fs::create_dir_all(&work)?;
    let result = stt_in(audio, &model, &work);
    let _ = fs::remove_dir_all(&work);
    result
}

fn stt_in(
    audio: &std::path::Path,
    model: &std::path::Path,
    work: &std::path::Path,
) -> Result<Value> {
    let wav = work.join("audio16k.wav");
    videoeditor_media::to_whisper_wav(audio, &wav)?;

    let out_base = work.join("transcript");
    let out = Command::new(bin())
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .args(["--max-len", "1", "--split-on-word", "-oj", "-np", "-of"])
        .arg(&out_base)
        .output()
        .with_context(|| {
            format!(
                "{} not found — install whisper.cpp (the nix install bundles it) \
                 or set WHISPER_BIN / VIDEOEDITOR_STT=elevenlabs",
                bin()
            )
        })?;
    if !out.status.success() {
        bail!(
            "whisper STT failed on {}: {}",
            audio.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let raw: Value = serde_json::from_str(&fs::read_to_string(out_base.with_extension("json"))?)?;
    Ok(normalize(&raw))
}

/// whisper.cpp JSON → the crate's transcript shape (ElevenLabs-Scribe-like):
/// `{"text": ..., "words": [{"type":"word","text","start","end"}]}` with
/// seconds instead of whisper's millisecond offsets.
pub fn normalize(raw: &Value) -> Value {
    let segments = raw["transcription"].as_array();
    let mut text = String::new();
    let mut words = Vec::new();
    for seg in segments.into_iter().flatten() {
        let seg_text = seg["text"].as_str().unwrap_or("");
        text.push_str(seg_text);
        let word = seg_text.trim();
        if word.is_empty() {
            continue;
        }
        words.push(json!({
            "type": "word",
            "text": word,
            "start": seg["offsets"]["from"].as_f64().unwrap_or(0.0) / 1000.0,
            "end": seg["offsets"]["to"].as_f64().unwrap_or(0.0) / 1000.0,
        }));
    }
    json!({ "text": text.trim(), "words": words })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_whisper_json_to_transcript_shape() {
        let raw = json!({
            "systeminfo": "…",
            "transcription": [
                { "offsets": {"from": 0, "to": 60}, "text": "" },
                { "offsets": {"from": 60, "to": 220}, "text": " The" },
                { "offsets": {"from": 220, "to": 560}, "text": " quick" }
            ]
        });
        let t = normalize(&raw);
        assert_eq!(t["text"], "The quick");
        let words = t["words"].as_array().unwrap();
        assert_eq!(words.len(), 2); // empty segment dropped
        assert_eq!(words[0]["type"], "word");
        assert_eq!(words[0]["text"], "The");
        assert_eq!(words[0]["start"], 0.06);
        assert_eq!(words[1]["end"], 0.56);
    }
}
