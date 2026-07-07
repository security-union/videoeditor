//! The template repertoire: discovery (`videoeditor templates`) and visual
//! previews (`videoeditor preview`).
//!
//! Templates self-describe with an inert JSON block:
//! `<script type="application/json" id="template-info">{ description, keys,
//! demo }</script>` — parseable without a browser, readable by the page.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use videoeditor_chrome::{Chrome, find_chrome};

#[derive(Debug, Default, Deserialize)]
pub struct TemplateInfo {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
    #[serde(default)]
    pub demo: Map<String, Value>,
}

/// Extract the `template-info` block from a template's HTML, if present.
pub fn template_info(html_path: &Path) -> Result<Option<TemplateInfo>> {
    let src = fs::read_to_string(html_path)?;
    let Some(tag) = src.find("id=\"template-info\"") else {
        return Ok(None);
    };
    let body_start = tag
        + src[tag..]
            .find('>')
            .context("unterminated template-info tag")?
        + 1;
    let body_end = body_start
        + src[body_start..]
            .find("</script>")
            .context("unterminated template-info block")?;
    let info = serde_json::from_str(src[body_start..body_end].trim())
        .with_context(|| format!("invalid template-info JSON in {}", html_path.display()))?;
    Ok(Some(info))
}

/// Every template visible from the given roots, first-match-wins:
/// (name, resolved path, shadowed-by-earlier-layer paths).
pub fn discover(roots: &[PathBuf]) -> Vec<(String, PathBuf, Vec<PathBuf>)> {
    let mut out: Vec<(String, PathBuf, Vec<PathBuf>)> = Vec::new();
    for root in roots {
        let dir = root.join("templates/scenes");
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut names: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().is_some_and(|x| x == "html")
                    && !p
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('_'))
            })
            .collect();
        names.sort();
        for path in names {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            match out.iter_mut().find(|(n, _, _)| *n == name) {
                Some((_, _, shadowed)) => shadowed.push(path),
                None => out.push((name, path, vec![])),
            }
        }
    }
    out
}

/// `videoeditor templates` — print the full repertoire with descriptions,
/// data keys, and which layer each template comes from.
pub fn list(roots: &[PathBuf]) -> Result<()> {
    println!("resolution layers (most specific first):");
    for (i, root) in roots.iter().enumerate() {
        println!("  {}. {}", i + 1, root.display());
    }
    for (name, path, shadowed) in discover(roots) {
        println!("\n━ {name}  ({})", path.display());
        match template_info(&path)? {
            Some(info) => {
                if !info.description.is_empty() {
                    println!("  {}", info.description);
                }
                for (key, doc) in &info.keys {
                    println!("    {key:<28} {doc}");
                }
            }
            None => println!("  (no template-info block — ask its author to add one)"),
        }
        for s in shadowed {
            println!("  shadows: {}", s.display());
        }
    }
    println!("\npreview any of these: videoeditor preview <name> (or --all)");
    Ok(())
}

/// `videoeditor preview` — render a template's demo data at five time points
/// into a contact-sheet PNG. `--all` renders the entire repertoire.
pub fn preview(roots: &[PathBuf], template: Option<&str>, out_dir: &Path) -> Result<()> {
    let all = discover(roots);
    let targets: Vec<_> = match template {
        Some(name) => {
            let hit = all
                .into_iter()
                .find(|(n, _, _)| n == name)
                .with_context(|| {
                    format!("template `{name}` not found — try `videoeditor templates`")
                })?;
            vec![hit]
        }
        None => all,
    };
    if targets.is_empty() {
        bail!("no templates found in any layer");
    }
    fs::create_dir_all(out_dir)?;

    println!("preview: launching headless Chrome…");
    let mut chrome = Chrome::launch(&find_chrome()?, 1080, 1920)?;
    for (name, path, _) in targets {
        let info = template_info(&path)?.unwrap_or_default();
        let mut data = info.demo.clone();
        let duration = data.get("duration").and_then(|v| v.as_f64()).unwrap_or(3.0);
        data.entry("width".to_string()).or_insert(Value::from(1080));
        data.entry("height".to_string())
            .or_insert(Value::from(1920));
        data.entry("duration".to_string())
            .or_insert(Value::from(duration));
        data.entry("sceneName".to_string())
            .or_insert(Value::String(format!("preview-{name}")));

        let frames = out_dir.join(format!(".frames-{name}"));
        let _ = fs::remove_dir_all(&frames);
        fs::create_dir_all(&frames)?;
        chrome.navigate(&format!("file://{}", path.display()))?;
        chrome.init_scene(&serde_json::to_string(&Value::Object(data))?)?;
        for (i, frac) in [0.0, 0.25, 0.5, 0.75, 0.98].iter().enumerate() {
            chrome.seek(duration * frac * 1000.0)?;
            chrome.screenshot(&frames.join(format!("f_{i:05}.png")))?;
        }
        let sheet = out_dir.join(format!("{name}.png"));
        videoeditor_media::ffmpeg(&[
            "-i",
            frames.join("f_%05d.png").to_str().unwrap(),
            "-vf",
            "scale=270:480,tile=5x1",
            "-frames:v",
            "1",
            sheet.to_str().unwrap(),
        ])?;
        let _ = fs::remove_dir_all(&frames);
        println!("preview: {} → {}", name, sheet.display());
    }
    Ok(())
}
