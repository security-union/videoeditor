//! Final assembly: concat scene videos, then mix narration clips (at absolute
//! offsets), native audio of video-clip scenes, and the music bed.

use crate::ffmpeg;
use anyhow::{Context, Result, bail};
use std::fs;
use videoeditor_timeline::Episode;

pub fn run(ep: &Episode) -> Result<()> {
    let build = ep.root.join("build");
    fs::create_dir_all(&build)?;

    // 1. concat scene videos (identical encode params → stream copy)
    let mut list = String::new();
    for (idx, scene) in ep.scenes.iter().enumerate() {
        let p = ep.scene_mp4(idx, scene);
        if !p.exists() {
            bail!("missing {} — run `videoeditor render` first", p.display());
        }
        list.push_str(&format!("file '{}'\n", p.display()));
    }
    let list_path = build.join("concat.txt");
    fs::write(&list_path, list)?;
    let concat = build.join("video_concat.mp4");
    ffmpeg(&[
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        list_path.to_str().unwrap(),
        "-c",
        "copy",
        concat.to_str().unwrap(),
    ])?;

    // 2. build the audio mix graph
    let manifest = ep.read_clip_manifest()?;
    for w in ep.fit_check(&manifest) {
        println!("assemble: ⚠ {w}");
    }
    let mut inputs: Vec<String> = vec![concat.display().to_string()];
    let mut chains: Vec<String> = Vec::new();
    let mut mix_labels: Vec<String> = Vec::new();
    let norm = "aresample=44100,aformat=sample_fmts=fltp:channel_layouts=stereo";

    // narration clips
    for scene in &ep.scenes {
        let mut cursor = 0.0f64;
        for chunk in &scene.chunks {
            let Some(clip) = manifest
                .iter()
                .find(|c| c.scene == scene.name && c.chunk == chunk.name)
            else {
                bail!(
                    "no TTS clip for {}/{} — run `videoeditor tts`",
                    scene.name,
                    chunk.name
                );
            };
            let rel_at = chunk.at.unwrap_or(cursor);
            cursor = rel_at + clip.duration / chunk.tempo + 0.15;
            let abs_ms = ((scene.start + rel_at) * 1000.0).round() as i64;
            let idx = inputs.len();
            inputs.push(ep.root.join(&clip.file).display().to_string());
            let label = format!("n{idx}");
            let tempo = if (chunk.tempo - 1.0).abs() > 1e-6 {
                format!("atempo={},", chunk.tempo)
            } else {
                String::new()
            };
            chains.push(format!(
                "[{idx}:a]{tempo}{norm},adelay={abs_ms}:all=1[{label}]"
            ));
            mix_labels.push(label);
        }
    }

    // native audio from video-clip scenes
    for scene in &ep.scenes {
        if !scene.is_video_clip() {
            continue;
        }
        if scene.data.get("audio").and_then(|v| v.as_bool()) == Some(false) {
            continue;
        }
        let src = ep
            .root
            .join(scene.data_str("src").context("video-clip src")?);
        let seek = scene.data_f64("seek").unwrap_or(0.0);
        let abs_ms = (scene.start * 1000.0).round() as i64;
        let idx = inputs.len();
        inputs.push(src.display().to_string());
        let label = format!("c{idx}");
        chains.push(format!(
            "[{idx}:a]atrim={}:{},asetpts=PTS-STARTPTS,{norm},adelay={abs_ms}:all=1[{label}]",
            seek,
            seek + scene.duration
        ));
        mix_labels.push(label);
    }

    // music bed
    if let Some(music_rel) = &ep.meta.music {
        let music = ep.root.join(music_rel);
        if music.exists() {
            let idx = inputs.len();
            inputs.push(music.display().to_string());
            let label = format!("m{idx}");
            // aloop repeats the bed so it always covers the episode length
            chains.push(format!(
                "[{idx}:a]{norm},aloop=loop=-1:size=2000000,atrim=0:{},volume={}dB[{label}]",
                ep.total_duration, ep.meta.music_gain_db
            ));
            mix_labels.push(label);
        } else {
            println!(
                "assemble: music {} not found — skipping bed",
                music.display()
            );
        }
    }

    let final_mp4 = build.join("final.mp4");
    if mix_labels.is_empty() {
        ffmpeg(&[
            "-i",
            concat.to_str().unwrap(),
            "-c",
            "copy",
            "-an",
            final_mp4.to_str().unwrap(),
        ])?;
    } else {
        let mix_inputs: String = mix_labels.iter().map(|l| format!("[{l}]")).collect();
        // NB: trim to episode length INSIDE the graph — an output-side `-t`
        // combined with -c:v copy + filter_complex silently drops the mixed
        // narration on ffmpeg 7.1 (only the longest input survives).
        // apad afterwards so the audio track spans the full video even when
        // narration ends early (players handle a short track inconsistently).
        let filter = format!(
            "{};{}amix=inputs={}:duration=longest:normalize=0,atrim=0:{dur},apad=whole_dur={dur}[mix]",
            chains.join(";"),
            mix_inputs,
            mix_labels.len(),
            dur = ep.total_duration
        );
        if std::env::var("VIDEOEDITOR_DEBUG").is_ok() {
            println!("assemble: inputs = {inputs:#?}");
            println!("assemble: filter = {filter}");
        }
        let mut args: Vec<String> = Vec::new();
        for input in &inputs {
            args.push("-i".into());
            args.push(input.clone());
        }
        args.extend([
            "-filter_complex".into(),
            filter,
            "-map".into(),
            "0:v".into(),
            "-map".into(),
            "[mix]".into(),
            "-c:v".into(),
            "copy".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "192k".into(),
            final_mp4.display().to_string(),
        ]);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        ffmpeg(&arg_refs)?;
    }

    println!(
        "assemble: {} ({:.2}s, {} scenes, {} audio tracks)",
        final_mp4.display(),
        ep.total_duration,
        ep.scenes.len(),
        mix_labels.len()
    );
    Ok(())
}
