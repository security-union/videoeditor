//! ffmpeg layer — videoeditor's `libavcodec`: everything that shells out to
//! ffmpeg/ffprobe. Scene encodes, video-clip passthrough scenes, final
//! assembly (concat + audio mix), and scene-cut detection.

pub mod assemble;

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;
use videoeditor_timeline::{Episode, Scene};

pub fn ffprobe_duration(path: &Path) -> Result<f64> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .context("ffprobe not found — install ffmpeg")?;
    if !out.status.success() {
        bail!(
            "ffprobe failed on {}: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .with_context(|| format!("unparseable duration for {}", path.display()))
}

/// Run ffmpeg with `-y -v error` prepended; error out with stderr on failure.
pub fn ffmpeg(args: &[&str]) -> Result<()> {
    let out = Command::new("ffmpeg")
        .args(["-y", "-v", "error"])
        .args(args)
        .output()
        .context("ffmpeg not found — install ffmpeg")?;
    if !out.status.success() {
        bail!(
            "ffmpeg {:?} failed:\n{}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// Encode a directory of `f_%05d.png` frames into a scene mp4.
pub fn encode_frames(frames_dir: &Path, fps: u32, duration: f64, out: &Path) -> Result<()> {
    ffmpeg(&[
        "-framerate",
        &fps.to_string(),
        "-i",
        frames_dir.join("f_%05d.png").to_str().unwrap(),
        "-t",
        &duration.to_string(),
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "18",
        "-pix_fmt",
        "yuv420p",
        out.to_str().unwrap(),
    ])
}

/// Render a `template=video-clip` scene: ffmpeg trims/scales the source clip
/// (no Chrome involved), optionally cropping the top and drawing a caption.
pub fn render_clip_scene(ep: &Episode, scene: &Scene, out: &Path) -> Result<()> {
    let src = scene
        .data_str("src")
        .context("video-clip scene needs DATA src=")?;
    let src = ep.root.join(src);
    let seek = scene.data_f64("seek").unwrap_or(0.0);
    let mut vf = String::new();
    // crop_top=0.25 removes the top 25% of the SOURCE first — e.g. to cut a
    // caption baked into a reference crop before re-captioning.
    if let Some(frac) = scene.data_f64("crop_top") {
        vf.push_str(&format!("crop=iw:ih*{}:0:ih*{frac},", 1.0 - frac));
    }
    vf.push_str(&format!(
        "scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},fps={fps},setsar=1",
        w = ep.meta.width,
        h = ep.meta.height,
        fps = ep.meta.fps
    ));
    // caption="PSA for JS devs:" draws meme-style text near the top.
    // Lines split on '|'. Font override: caption_font=/path/to.ttf
    if let Some(caption) = scene.data_str("caption") {
        let font = scene
            .data_str("caption_font")
            .unwrap_or("/System/Library/Fonts/Supplemental/Impact.ttf");
        let size = scene.data_f64("caption_size").unwrap_or(84.0);
        for (i, line) in caption.split('|').enumerate() {
            let text = line
                .replace('\\', "\\\\")
                .replace('\'', "\\'")
                .replace(':', "\\:");
            vf.push_str(&format!(
                ",drawtext=fontfile={font}:text='{text}':fontcolor=white:fontsize={size}:\
                 borderw=10:bordercolor=black:x=(w-text_w)/2:y=h*0.055+{i}*{size}*1.18"
            ));
        }
    }
    ffmpeg(&[
        "-ss",
        &seek.to_string(),
        "-i",
        src.to_str().unwrap(),
        "-t",
        &scene.duration.to_string(),
        "-vf",
        &vf,
        "-an",
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "18",
        "-pix_fmt",
        "yuv420p",
        out.to_str().unwrap(),
    ])
}

/// Extract a video's audio track to mp3 (for STT).
pub fn extract_audio(video: &Path, out: &Path) -> Result<()> {
    ffmpeg(&[
        "-i",
        video.to_str().unwrap(),
        "-vn",
        "-acodec",
        "libmp3lame",
        "-q:a",
        "2",
        out.to_str().unwrap(),
    ])
}

/// Detect scene cuts with ffmpeg's `select=gt(scene,threshold)` filter;
/// returns cut timestamps in seconds.
pub fn scene_cuts(video: &Path, threshold: f32) -> Result<Vec<f64>> {
    let out = Command::new("ffmpeg")
        .args(["-i"])
        .arg(video)
        .args([
            "-vf",
            &format!("select='gt(scene,{threshold})',showinfo"),
            "-f",
            "null",
            "-",
        ])
        .output()
        .context("ffmpeg not found")?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    let mut cuts = Vec::new();
    for line in stderr.lines() {
        if !line.contains("showinfo") {
            continue;
        }
        if let Some(pos) = line.find("pts_time:") {
            let rest = &line[pos + "pts_time:".len()..];
            let num: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if let Ok(t) = num.parse::<f64>() {
                cuts.push(t);
            }
        }
    }
    Ok(cuts)
}
