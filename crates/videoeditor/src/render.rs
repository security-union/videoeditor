//! Scene renderer.
//!
//! Web scenes: every frame is a deterministic headless-Chrome screenshot of
//! `templates/scenes/<template>.html?d=<b64url json>&t=<ms>`. Templates are
//! pure functions of (data, t) — no running animations, no flakiness.
//!
//! `video-clip` scenes bypass Chrome: ffmpeg trims/scales the source clip.

use anyhow::{Context, Result};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value};
use std::fs;
use std::io::Write as _;
use std::path::Path;
use videoeditor_chrome::{Chrome, find_chrome};
use videoeditor_timeline::{Episode, Scene};

pub fn run(ep: &Episode, only_scene: Option<&str>) -> Result<()> {
    let scenes_dir = ep.root.join("build/scenes");
    fs::create_dir_all(&scenes_dir)?;

    let needs_chrome = ep
        .scenes
        .iter()
        .any(|s| !s.is_video_clip() && only_scene.is_none_or(|o| o == s.name));
    let mut chrome = if needs_chrome {
        println!("render: launching headless Chrome…");
        Some(Chrome::launch(
            &find_chrome()?,
            ep.meta.width,
            ep.meta.height,
        )?)
    } else {
        None
    };

    for (idx, scene) in ep.scenes.iter().enumerate() {
        if only_scene.is_some_and(|s| s != scene.name) {
            continue;
        }
        let out = ep.scene_mp4(idx, scene);
        println!(
            "render: [{}/{}] {} ({}, {:.2}s)",
            idx + 1,
            ep.scenes.len(),
            scene.name,
            scene.template,
            scene.duration
        );
        if scene.is_video_clip() {
            videoeditor_media::render_clip_scene(ep, scene, &out)?;
        } else {
            render_web_scene(ep, scene, chrome.as_mut().unwrap(), &out)?;
        }
    }
    Ok(())
}

fn render_web_scene(ep: &Episode, scene: &Scene, chrome: &mut Chrome, out: &Path) -> Result<()> {
    let template = ep.resolve_template(&scene.template)?;
    if !template.starts_with(&ep.assets_root) {
        println!("  template: {}", template.display());
    }

    let data = resolve_data(ep, scene)?;

    let frames_dir = ep.root.join("build/frames").join(&scene.name);
    let _ = fs::remove_dir_all(&frames_dir);
    fs::create_dir_all(&frames_dir)?;

    let total_frames = (scene.duration * ep.meta.fps as f64).ceil() as usize;
    chrome.navigate(&format!("file://{}", template.display()))?;
    chrome.init_scene(&serde_json::to_string(&Value::Object(data))?)?;

    // template self-diagnostics at worst-case t (end of scene: Ken Burns
    // push-in is at max zoom) — catches silent clipping the way the
    // narration fit-check catches overlaps
    chrome.seek(scene.duration * 1000.0)?;
    for w in chrome.scene_warnings()? {
        println!("  ⚠ {}: {w}", scene.name);
    }

    for i in 0..total_frames {
        let t_ms = i as f64 / ep.meta.fps as f64 * 1000.0;
        chrome.seek(t_ms)?;
        chrome.screenshot(&frames_dir.join(format!("f_{i:05}.png")))?;
        if i % 30 == 0 || i + 1 == total_frames {
            print!("\r  frames: {}/{total_frames}", i + 1);
            std::io::stdout().flush().ok();
        }
    }
    println!();

    videoeditor_media::encode_frames(&frames_dir, ep.meta.fps, scene.duration, out)
}

/// Prepare the JSON handed to the template: inline code files as `codeText`,
/// inline image assets as data: URIs (sidesteps file:// subresource policy;
/// data is injected via CDP so there is no URL-length concern).
fn resolve_data(ep: &Episode, scene: &Scene) -> Result<Map<String, Value>> {
    let mut data = scene.data.clone();

    if let Some(code_rel) = scene.data_str("code").map(str::to_string) {
        let code_path = ep.root.join(&code_rel);
        let text = fs::read_to_string(&code_path)
            .with_context(|| format!("code file {}", code_path.display()))?;
        data.insert("codeText".into(), Value::String(text));
        if !data.contains_key("lang") {
            if let Some(ext) = code_path.extension().and_then(|e| e.to_str()) {
                let lang = match ext {
                    "rs" => "rust",
                    "py" => "python",
                    "ts" => "typescript",
                    "js" => "javascript",
                    other => other,
                };
                data.insert("lang".into(), Value::String(lang.into()));
            }
        }
    }

    let keys: Vec<String> = data.keys().cloned().collect();
    for k in keys {
        if let Some(Value::String(v)) = data.get(&k) {
            let p = ep.root.join(v);
            if !v.contains('/') || !p.exists() {
                continue;
            }
            let mime = match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
                "png" => Some("image/png"),
                "jpg" | "jpeg" => Some("image/jpeg"),
                "webp" => Some("image/webp"),
                "gif" => Some("image/gif"),
                "svg" => Some("image/svg+xml"),
                _ => None,
            };
            if let Some(mime) = mime {
                let b64 = STANDARD.encode(fs::read(&p)?);
                data.insert(k, Value::String(format!("data:{mime};base64,{b64}")));
            } else {
                data.insert(
                    k,
                    Value::String(format!("file://{}", p.canonicalize()?.display())),
                );
            }
        }
    }

    data.insert("sceneName".into(), Value::String(scene.name.clone()));
    data.insert("duration".into(), Value::from(scene.duration));
    data.insert("width".into(), Value::from(ep.meta.width));
    data.insert("height".into(), Value::from(ep.meta.height));
    Ok(data)
}
