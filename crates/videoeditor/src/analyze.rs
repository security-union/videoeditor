//! Reference-video analysis: the "understand the viral" half of the tool.
//! Extracts audio → ElevenLabs Scribe STT (word timestamps) → ffmpeg scene-cut
//! detection → analysis.json + a human-readable timing table.

use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn run(video: &Path, out: Option<&Path>, threshold: f32) -> Result<()> {
    let video = video.canonicalize().context("video not found")?;
    let out_dir = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| video.parent().unwrap().join("analysis"));
    fs::create_dir_all(&out_dir)?;

    let duration = videoeditor_media::ffprobe_duration(&video)?;

    println!("analyze: extracting audio…");
    let audio = out_dir.join("audio.mp3");
    videoeditor_media::extract_audio(&video, &audio)?;

    println!("analyze: transcribing (ElevenLabs Scribe)…");
    let transcript = videoeditor_voice::stt(&audio)?;
    fs::write(
        out_dir.join("transcript.json"),
        serde_json::to_string_pretty(&transcript)?,
    )?;

    println!("analyze: detecting scene cuts (threshold {threshold})…");
    let cuts = videoeditor_media::scene_cuts(&video, threshold)?;

    let analysis = json!({
        "video": video.display().to_string(),
        "duration": duration,
        "cuts": cuts,
        "transcript": transcript,
    });
    fs::write(
        out_dir.join("analysis.json"),
        serde_json::to_string_pretty(&analysis)?,
    )?;

    // human-readable segment table: words grouped into cut segments
    let words: Vec<(f64, String)> = transcript["words"]
        .as_array()
        .map(|ws| {
            ws.iter()
                .filter(|w| w["type"] == "word")
                .map(|w| {
                    (
                        w["start"].as_f64().unwrap_or(0.0),
                        w["text"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let mut bounds = vec![0.0];
    bounds.extend(cuts.iter().copied());
    bounds.push(duration);
    println!(
        "\n# Timing map — {} ({:.2}s)\n",
        video.file_name().unwrap().to_string_lossy(),
        duration
    );
    for pair in bounds.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if b - a < 0.4 {
            continue; // flicker, not a scene
        }
        let seg: Vec<&str> = words
            .iter()
            .filter(|(t, _)| *t >= a && *t < b)
            .map(|(_, w)| w.as_str())
            .collect();
        println!(
            "{a:6.2} → {b:6.2} ({:5.2}s, {:4.1}%)  {}",
            b - a,
            (b - a) / duration * 100.0,
            seg.join(" ")
        );
    }
    println!(
        "\nanalyze: wrote {}",
        out_dir.join("analysis.json").display()
    );
    Ok(())
}
