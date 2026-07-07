//! ElevenLabs voice I/O: text-to-speech (one MP3 per `[CHUNK:]`, name-keyed,
//! plus a `clips.json` manifest with probed durations) and Scribe
//! speech-to-text for reference-video transcription.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use videoeditor_timeline::{ClipInfo, Episode, Meta};

pub fn api_key() -> Result<String> {
    env::var("ELEVENLABS_API_KEY")
        .or_else(|_| env::var("ELEVENLAB"))
        .context("set ELEVENLABS_API_KEY (your ElevenLabs API key)")
}

/// Generate narration clips for every `[CHUNK:]` (skips existing files unless
/// `force`) and write the `audio/clips.json` manifest.
pub fn run(ep: &Episode, only_chunk: Option<&str>, force: bool) -> Result<()> {
    let clips_dir = ep.root.join("audio/clips");
    fs::create_dir_all(&clips_dir)?;

    let has_narration = ep.scenes.iter().any(|s| !s.chunks.is_empty());
    if !has_narration {
        println!("tts: no chunks in script, nothing to do");
        return Ok(());
    }

    let voice = ep
        .meta
        .voice_id
        .as_deref()
        .context("frontmatter needs voice_id: for TTS")?;
    let key = api_key()?;
    let mut manifest: Vec<ClipInfo> = Vec::new();

    for scene in &ep.scenes {
        for chunk in &scene.chunks {
            let id = format!("{}__{}", scene.name, chunk.name);
            let path = clips_dir.join(format!("{id}.mp3"));
            let selected = only_chunk.is_none_or(|c| c == id || c == chunk.name);
            if selected && (force || !path.exists()) {
                if chunk.text.is_empty() {
                    bail!("chunk {id} has no narration text");
                }
                println!("tts: {id} ({} chars)", chunk.text.len());
                synth(&key, voice, &ep.meta, &chunk.text, &path)?;
            }
            if path.exists() {
                manifest.push(ClipInfo {
                    scene: scene.name.clone(),
                    chunk: chunk.name.clone(),
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

fn synth(key: &str, voice: &str, meta: &Meta, text: &str, out: &Path) -> Result<()> {
    let url =
        format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128");
    let resp = ureq::post(&url)
        .set("xi-api-key", key)
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

/// Transcribe an audio file with ElevenLabs Scribe (word-level timestamps).
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
