//! Piper local TTS backend: a piper voice (vits onnx) run through
//! `sherpa-onnx-offline-tts`. `PIPER_VOICE` points at the voice directory
//! (model.onnx + tokens.txt + espeak-ng-data); the nix package/dev shell pin
//! one (en_US-lessac-medium). More voices:
//! <https://github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models>.

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn bin() -> String {
    env::var("SHERPA_TTS_BIN").unwrap_or_else(|_| "sherpa-onnx-offline-tts".to_string())
}

pub fn voice_dir() -> Result<PathBuf> {
    let dir = env::var("PIPER_VOICE").map(PathBuf::from).context(
        "set PIPER_VOICE to a piper voice directory (the nix install pins one; \
         otherwise unpack a vits-piper-* release from \
         github.com/k2-fsa/sherpa-onnx, or use tts: elevenlabs)",
    )?;
    if !dir.is_dir() {
        bail!(
            "PIPER_VOICE points at {} which is not a directory",
            dir.display()
        );
    }
    Ok(dir)
}

/// Synthesize `text` to mp3: sherpa-onnx renders a wav, ffmpeg encodes it to
/// the pipeline's mp3 format.
pub fn synth(text: &str, out: &Path) -> Result<()> {
    let voice = voice_dir()?;
    let onnx = fs::read_dir(&voice)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|x| x == "onnx"))
        .with_context(|| format!("no .onnx voice model in {}", voice.display()))?;

    let wav = out.with_extension("wav");
    let run = Command::new(bin())
        .arg(format!("--vits-model={}", onnx.display()))
        .arg(format!(
            "--vits-tokens={}",
            voice.join("tokens.txt").display()
        ))
        .arg(format!(
            "--vits-data-dir={}",
            voice.join("espeak-ng-data").display()
        ))
        .arg(format!("--output-filename={}", wav.display()))
        .arg(text)
        .output()
        .with_context(|| {
            format!(
                "{} not found — install sherpa-onnx (the nix install bundles it) \
                 or set SHERPA_TTS_BIN / tts: elevenlabs",
                bin()
            )
        })?;
    if !run.status.success() || !wav.exists() {
        bail!(
            "piper TTS failed: {}{}",
            String::from_utf8_lossy(&run.stderr),
            String::from_utf8_lossy(&run.stdout)
        );
    }
    let encode = videoeditor_media::wav_to_mp3(&wav, out);
    let _ = fs::remove_file(&wav);
    encode
}
