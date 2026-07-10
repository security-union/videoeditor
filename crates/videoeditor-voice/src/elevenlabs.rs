//! ElevenLabs cloud backend: TTS (voice_id presets, mp3 out) and Scribe STT.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use videoeditor_timeline::Meta;

pub fn api_key() -> Result<String> {
    env::var("ELEVENLABS_API_KEY")
        .or_else(|_| env::var("ELEVENLAB"))
        .context("set ELEVENLABS_API_KEY (your ElevenLabs API key)")
}

pub fn synth(meta: &Meta, text: &str, out: &Path) -> Result<()> {
    let key = api_key()?;
    let voice = meta
        .voice_id
        .as_deref()
        .context("frontmatter needs voice_id: for ElevenLabs TTS")?;
    let url =
        format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128");
    let resp = ureq::post(&url)
        .set("xi-api-key", &key)
        .send_json(serde_json::json!({
            "text": text,
            "model_id": meta.model_id,
            "voice_settings": {
                "stability": meta.voice_stability,
                "similarity_boost": meta.voice_similarity,
                "style": meta.voice_style,
                "use_speaker_boost": true
            }
        }));
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            bail!(
                "ElevenLabs TTS {code}: {}",
                r.into_string().unwrap_or_default()
            )
        }
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    resp.into_reader().read_to_end(&mut bytes)?;
    fs::write(out, bytes)?;
    Ok(())
}

/// Transcribe with ElevenLabs Scribe (word-level timestamps). Returns the
/// Scribe response as-is — it is already the crate's transcript shape.
pub fn stt(audio: &Path) -> Result<Value> {
    let key = api_key()?;
    let bytes = fs::read(audio)?;
    let boundary = "----videoeditorboundary7d1c9a2f";
    let mut body = Vec::new();
    for (name, value) in [
        ("model_id", "scribe_v1"),
        ("timestamps_granularity", "word"),
    ] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.mp3\"\r\nContent-Type: audio/mpeg\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let resp = ureq::post("https://api.elevenlabs.io/v1/speech-to-text")
        .set("xi-api-key", &key)
        .set(
            "content-type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send_bytes(&body);
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            bail!(
                "ElevenLabs STT {code}: {}",
                r.into_string().unwrap_or_default()
            )
        }
        Err(e) => return Err(e.into()),
    };
    let mut s = String::new();
    resp.into_reader().read_to_string(&mut s)?;
    Ok(serde_json::from_str(&s)?)
}
