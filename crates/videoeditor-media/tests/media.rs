//! Integration tests against real (tiny, LFS-tracked) media fixtures.
//! They self-skip when ffprobe is unavailable (nix sandbox, bare CI) or when
//! the fixture is an un-fetched LFS pointer (checkout without `lfs: true`).

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn media_available(path: &Path) -> bool {
    let ffprobe = Command::new("ffprobe")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let real_content = std::fs::read(path)
        .map(|b| !b.starts_with(b"version https://git-lfs"))
        .unwrap_or(false);
    if !ffprobe || !real_content {
        eprintln!("skipping media test: ffprobe or LFS fixture unavailable");
        return false;
    }
    true
}

#[test]
fn probes_video_duration_and_detects_the_cut() {
    let mp4 = fixture("tiny.mp4");
    if !media_available(&mp4) {
        return;
    }
    let d = videoeditor_media::ffprobe_duration(&mp4).expect("ffprobe duration");
    assert!((d - 1.0).abs() < 0.1, "expected ~1.0s, got {d}");

    // fixture is 0.5s red then 0.5s blue — exactly one hard cut at 0.5s
    let cuts = videoeditor_media::scene_cuts(&mp4, 0.12).expect("scene cuts");
    assert_eq!(cuts.len(), 1, "expected one cut, got {cuts:?}");
    assert!((cuts[0] - 0.5).abs() < 0.1, "cut at {}", cuts[0]);
}

#[test]
fn probes_audio_duration() {
    let mp3 = fixture("tiny.mp3");
    if !media_available(&mp3) {
        return;
    }
    let d = videoeditor_media::ffprobe_duration(&mp3).expect("ffprobe duration");
    assert!((d - 0.5).abs() < 0.15, "expected ~0.5s, got {d}");
}
