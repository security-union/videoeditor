//! Episode timeline model + `script.md` parser — the source of truth for an
//! episode. Think of this crate as videoeditor's `libavformat`: it turns the
//! authored container (markdown) into a typed timeline the rest of the
//! pipeline consumes.
//!
//! Grammar (line-oriented markers):
//!
//! ```text
//! ---
//! title: My Episode
//! fps: 30
//! voice_id: pNInz6obpgDQGcFmaJgB   # "Adam" — an ElevenLabs public preset
//! music: assets/music/bed.mp3
//! ---
//!
//! [SCENE: name | template=code-meme duration=6.42]
//! [DATA: code=assets/code/threads.rs lang=rust bench="μ: 150µS|σ: 50µS" bench_at=5.8]
//! [CLIP: threads | at=0.19]
//! Narration text until the next marker.
//! ```
//!
//! `template=video-clip` scenes play a source clip (DATA: src=...) instead of a
//! web template, keeping their native audio unless `audio=false`.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct Episode {
    pub root: PathBuf,
    /// Root directory holding the built-in `templates/` and `formats/`
    /// (resolved by the CLI).
    pub assets_root: PathBuf,
    /// Template resolution layers, most specific first: the episode dir
    /// itself → frontmatter `packs:` → `$VIDEOEDITOR_PACK_PATH` entries →
    /// the built-ins. See [`Episode::resolve_template`].
    pub template_roots: Vec<PathBuf>,
    pub meta: Meta,
    pub scenes: Vec<Scene>,
    pub total_duration: f64,
}

#[derive(Debug, Serialize)]
pub struct Meta {
    pub title: String,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
    /// Template packs this episode uses (frontmatter `packs:`,
    /// comma-separated paths relative to the episode dir).
    pub packs: Vec<String>,
    pub voice_id: Option<String>,
    pub model_id: String,
    /// ElevenLabs voice_settings — low stability reads livelier, less robotic.
    pub voice_stability: f64,
    pub voice_similarity: f64,
    pub voice_style: f64,
    pub music: Option<String>,
    pub music_gain_db: f64,
}

#[derive(Debug, Serialize)]
pub struct Scene {
    pub name: String,
    pub template: String,
    pub duration: f64,
    /// Absolute start time in the final timeline (computed).
    pub start: f64,
    pub data: Map<String, Value>,
    pub clips: Vec<Clip>,
}

#[derive(Debug, Serialize)]
pub struct Clip {
    pub name: String,
    pub text: String,
    /// Offset from scene start, seconds. None = after previous clip.
    pub at: Option<f64>,
    pub tempo: f64,
}

/// One generated narration clip, as recorded in `audio/clips.json`.
/// Written by the TTS stage, consumed by assembly.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClipInfo {
    pub scene: String,
    pub clip: String,
    pub file: String,
    pub duration: f64,
}

impl Scene {
    pub fn is_video_clip(&self) -> bool {
        self.template == "video-clip"
    }
    pub fn data_str(&self, key: &str) -> Option<&str> {
        self.data.get(key).and_then(|v| v.as_str())
    }
    pub fn data_f64(&self, key: &str) -> Option<f64> {
        self.data.get(key).and_then(|v| v.as_f64())
    }
}

impl Episode {
    /// Canonical path of a rendered scene video inside `build/scenes/`.
    pub fn scene_mp4(&self, idx: usize, scene: &Scene) -> PathBuf {
        self.root
            .join(format!("build/scenes/{:02}_{}.mp4", idx, scene.name))
    }

    pub fn read_clip_manifest(&self) -> Result<Vec<ClipInfo>> {
        let p = self.root.join("audio/clips.json");
        if !p.exists() {
            return Ok(vec![]);
        }
        Ok(serde_json::from_str(&fs::read_to_string(p)?)?)
    }

    /// Fit-check: narration must fit the timeline. A single narrator can't
    /// say two clips at once, so a clip whose measured audio is still
    /// playing when the next clip starts is always a bug (it renders as
    /// garbled, overlapping voices). Crossing a SCENE boundary is fine —
    /// continuous narration over a cut is a feature of the format.
    ///
    /// Returns one human-readable warning per violation (empty = fits).
    /// Recipe when it fires: scene duration = clip `at` + audio/tempo + hold,
    /// then re-place downstream `at`s — see PRODUCTION.md Rule 5.
    pub fn fit_check(&self, manifest: &[ClipInfo]) -> Vec<String> {
        let mut spans: Vec<(String, f64, f64)> = Vec::new(); // (name, abs start, abs end)
        for scene in &self.scenes {
            let mut cursor = 0.0f64;
            for clip in &scene.clips {
                let Some(measured) = manifest
                    .iter()
                    .find(|c| c.scene == scene.name && c.clip == clip.name)
                else {
                    continue; // tts not run yet for this clip — nothing to check
                };
                let rel_at = clip.at.unwrap_or(cursor);
                let start = scene.start + rel_at;
                let end = start + measured.duration / clip.tempo;
                cursor = rel_at + measured.duration / clip.tempo + 0.15;
                spans.push((format!("{}/{}", scene.name, clip.name), start, end));
            }
        }
        let mut warnings = Vec::new();
        for pair in spans.windows(2) {
            let (prev, prev_start, prev_end) = &pair[0];
            let (next, next_start, _) = &pair[1];
            if next_start < prev_end {
                warnings.push(format!(
                    "narration overlap: {prev} plays {prev_start:.2}–{prev_end:.2}s but {next} \
                     starts at {next_start:.2}s — voices will talk over each other"
                ));
            }
        }
        if let Some((last, _, last_end)) = spans.last() {
            if *last_end > self.total_duration + 0.05 {
                warnings.push(format!(
                    "narration truncated: {last} ends at {last_end:.2}s but the video ends at \
                     {:.2}s",
                    self.total_duration
                ));
            }
        }
        warnings
    }

    /// Resolve a scene template name to an HTML file by walking
    /// `template_roots` in order — first match wins. Every root is shaped
    /// like the engine root (`<root>/templates/scenes/<name>.html`), so a
    /// pack is self-contained and an episode can override any built-in by
    /// shipping a file of the same name.
    pub fn resolve_template(&self, name: &str) -> Result<PathBuf> {
        resolve_in_roots(&self.template_roots, name)
    }
}

/// First `<root>/templates/scenes/<name>.html` that exists, roots in order.
pub fn resolve_in_roots(roots: &[PathBuf], name: &str) -> Result<PathBuf> {
    let rel = format!("templates/scenes/{name}.html");
    for root in roots {
        let candidate = root.join(&rel);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    bail!(
        "template `{name}` not found; searched (in order):\n{}",
        roots
            .iter()
            .map(|r| format!("  {}", r.join(&rel).display()))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Compute the template resolution layers for an episode.
/// Order: episode dir → declared packs (relative to the episode dir; a
/// missing declared pack is an error, not a silent skip) → colon-separated
/// `pack_path` entries (from `$VIDEOEDITOR_PACK_PATH`) → the built-in root.
pub fn template_roots(
    episode_root: &Path,
    packs: &[String],
    pack_path: Option<&str>,
    assets_root: &Path,
) -> Result<Vec<PathBuf>> {
    let mut roots = vec![episode_root.to_path_buf()];
    for pack in packs {
        let dir = if Path::new(pack).is_absolute() {
            PathBuf::from(pack)
        } else {
            episode_root.join(pack)
        };
        let dir = dir
            .canonicalize()
            .with_context(|| format!("pack `{pack}` (declared in frontmatter) not found"))?;
        roots.push(dir);
    }
    for entry in pack_path
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
    {
        roots.push(PathBuf::from(entry));
    }
    roots.push(assets_root.to_path_buf());
    Ok(roots)
}

pub fn load(episode_dir: &Path, assets_root: &Path) -> Result<Episode> {
    let root = episode_dir
        .canonicalize()
        .with_context(|| format!("episode dir not found: {}", episode_dir.display()))?;
    let script_path = root.join("script.md");
    let src = fs::read_to_string(&script_path)
        .with_context(|| format!("cannot read {}", script_path.display()))?;

    let (front, body) = split_frontmatter(&src)?;
    let meta = parse_meta(&front)?;
    let mut scenes = parse_scenes(&body)?;

    let mut cursor = 0.0;
    for s in &mut scenes {
        s.start = cursor;
        cursor += s.duration;
    }

    let pack_path = std::env::var("VIDEOEDITOR_PACK_PATH").ok();
    let template_roots = template_roots(&root, &meta.packs, pack_path.as_deref(), assets_root)?;

    Ok(Episode {
        assets_root: assets_root.to_path_buf(),
        template_roots,
        root,
        meta,
        total_duration: cursor,
        scenes,
    })
}

fn split_frontmatter(src: &str) -> Result<(String, String)> {
    let mut lines = src.lines();
    if lines.next().map(str::trim) != Some("---") {
        bail!("script.md must start with `---` frontmatter");
    }
    let mut front = String::new();
    for line in lines.by_ref() {
        if line.trim() == "---" {
            let body: String = lines.collect::<Vec<_>>().join("\n");
            return Ok((front, body));
        }
        front.push_str(line);
        front.push('\n');
    }
    bail!("unterminated frontmatter")
}

fn parse_meta(front: &str) -> Result<Meta> {
    let mut title = String::new();
    let mut fps = 30u32;
    let mut width = 1080u32;
    let mut height = 1920u32;
    let mut packs = Vec::new();
    let mut voice_id = None;
    let mut model_id = "eleven_multilingual_v2".to_string();
    let mut voice_stability = 0.4;
    let mut voice_similarity = 0.8;
    let mut voice_style = 0.45;
    let mut music = None;
    let mut music_gain_db = -20.0;

    for line in front.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim().to_string());
        match k {
            "title" => title = v,
            "fps" => fps = v.parse().context("fps")?,
            "width" => width = v.parse().context("width")?,
            "height" => height = v.parse().context("height")?,
            "packs" => {
                packs = v
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            }
            "voice_id" => voice_id = Some(v),
            "model_id" => model_id = v,
            "voice_stability" => voice_stability = v.parse().context("voice_stability")?,
            "voice_similarity" => voice_similarity = v.parse().context("voice_similarity")?,
            "voice_style" => voice_style = v.parse().context("voice_style")?,
            "music" => music = Some(v),
            "music_gain_db" => music_gain_db = v.parse().context("music_gain_db")?,
            _ => {}
        }
    }
    Ok(Meta {
        title,
        fps,
        width,
        height,
        packs,
        voice_id,
        model_id,
        voice_stability,
        voice_similarity,
        voice_style,
        music,
        music_gain_db,
    })
}

fn parse_scenes(body: &str) -> Result<Vec<Scene>> {
    let mut scenes: Vec<Scene> = Vec::new();
    let mut clip_text: Vec<String> = Vec::new();

    fn flush_clip(scenes: &mut [Scene], buf: &mut Vec<String>) {
        if let Some(scene) = scenes.last_mut() {
            if let Some(clip) = scene.clips.last_mut() {
                if clip.text.is_empty() {
                    clip.text = buf.join(" ").trim().to_string();
                }
            }
        }
        buf.clear();
    }

    let mut in_comment = false;
    for line in body.lines() {
        let trimmed = line.trim();
        // HTML comments are authoring notes, never narration
        if in_comment {
            if trimmed.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("<!--") {
            in_comment = !trimmed.contains("-->");
            continue;
        }
        if let Some(marker) = parse_marker(trimmed, "SCENE") {
            flush_clip(&mut scenes, &mut clip_text);
            let (name, attrs) = marker;
            let template = attr_str(&attrs, "template")
                .with_context(|| format!("scene `{name}` missing template="))?;
            let duration = attr_f64(&attrs, "duration")
                .with_context(|| format!("scene `{name}` missing duration="))?;
            let mut data = Map::new();
            for (k, v) in &attrs {
                if k != "template" && k != "duration" {
                    data.insert(k.clone(), to_value(v));
                }
            }
            scenes.push(Scene {
                name,
                template,
                duration,
                start: 0.0,
                data,
                clips: vec![],
            });
        } else if let Some((_, attrs)) = parse_marker(trimmed, "DATA") {
            let scene = scenes.last_mut().context("[DATA:] before any [SCENE:]")?;
            for (k, v) in attrs {
                scene.data.insert(k, to_value(&v));
            }
        } else if let Some((name, attrs)) = parse_marker(trimmed, "CLIP") {
            flush_clip(&mut scenes, &mut clip_text);
            let scene = scenes.last_mut().context("[CLIP:] before any [SCENE:]")?;
            scene.clips.push(Clip {
                name,
                text: String::new(),
                at: attr_f64(&attrs, "at").ok(),
                tempo: attr_f64(&attrs, "tempo").unwrap_or(1.0),
            });
        } else if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // unknown marker ([IMAGE:], [SFX:], comments) — ignored, not narrated
        } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
            clip_text.push(trimmed.to_string());
        }
    }
    flush_clip(&mut scenes, &mut clip_text);

    if scenes.is_empty() {
        bail!("script.md contains no [SCENE:] markers");
    }
    Ok(scenes)
}

/// Parse `[KIND: name | k=v k="quoted v" ...]` → (name, attrs).
/// `[DATA: k=v ...]` markers carry no name: the whole body is attrs (a `|`
/// may legitimately appear inside a quoted value, e.g. bench="a|b").
fn parse_marker(line: &str, kind: &str) -> Option<(String, Vec<(String, String)>)> {
    let prefix = format!("[{kind}:");
    if !line.starts_with(&prefix) || !line.ends_with(']') {
        return None;
    }
    let inner = &line[prefix.len()..line.len() - 1];
    if kind == "DATA" {
        return Some((String::new(), parse_kv(inner.trim())));
    }
    let (name, attrs_src) = match inner.split_once('|') {
        Some((n, a)) => (n.trim().to_string(), a.trim()),
        None => (inner.trim().to_string(), ""),
    };
    Some((name, parse_kv(attrs_src)))
}

/// Tokenize `k=v k2="v with spaces" flag=true` honoring double quotes.
fn parse_kv(src: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = src.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let mut key = String::new();
        while chars
            .peek()
            .is_some_and(|c| *c != '=' && !c.is_whitespace())
        {
            key.push(chars.next().unwrap());
        }
        if key.is_empty() {
            break;
        }
        if chars.peek() != Some(&'=') {
            out.push((key, "true".into()));
            continue;
        }
        chars.next(); // '='
        let mut val = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            for c in chars.by_ref() {
                if c == '"' {
                    break;
                }
                val.push(c);
            }
        } else {
            while chars.peek().is_some_and(|c| !c.is_whitespace()) {
                val.push(chars.next().unwrap());
            }
        }
        out.push((key, val));
    }
    out
}

fn attr_str(attrs: &[(String, String)], key: &str) -> Result<String> {
    attrs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .with_context(|| format!("missing attr {key}="))
}

fn attr_f64(attrs: &[(String, String)], key: &str) -> Result<f64> {
    attr_str(attrs, key)?
        .parse::<f64>()
        .with_context(|| format!("attr {key}= not a number"))
}

/// Numbers become JSON numbers, true/false booleans, everything else strings.
fn to_value(v: &str) -> Value {
    if let Ok(n) = v.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Value::Number(num);
        }
    }
    match v {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(v.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = r#"---
title: Demo
fps: 30
voice_id: pNInz6obpgDQGcFmaJgB
music: assets/music/bed.mp3
---

# heading is ignored
<!-- authoring note, never narrated -->

[SCENE: title | template=title-card duration=2.1]
[DATA: title="X vs Y" title_at=1200]
[CLIP: hook | at=0.15]
X versus Y for TOPIC.

[SCENE: good | template=code-meme duration=6.4]
[DATA: code=assets/code/good.rs bench="μ: 150µS|σ: 50µS" flat=true]
[CLIP: explain | at=0.2 tempo=1.05]
First line.
Second line joins the same clip.

[SCENE: outro | template=video-clip duration=2.2]
[DATA: src=assets/clips/punchline.mp4 seek=0 audio=false]
"#;

    fn parse(script: &str) -> Vec<Scene> {
        let (_, body) = split_frontmatter(script).unwrap();
        parse_scenes(&body).unwrap()
    }

    #[test]
    fn parses_frontmatter_meta() {
        let (front, _) = split_frontmatter(SCRIPT).unwrap();
        let meta = parse_meta(&front).unwrap();
        assert_eq!(meta.title, "Demo");
        assert_eq!(meta.fps, 30);
        assert_eq!(meta.width, 1080);
        assert_eq!(meta.voice_id.as_deref(), Some("pNInz6obpgDQGcFmaJgB"));
        assert_eq!(meta.music.as_deref(), Some("assets/music/bed.mp3"));
    }

    #[test]
    fn parses_scenes_clips_and_data() {
        let scenes = parse(SCRIPT);
        assert_eq!(scenes.len(), 3);

        let title = &scenes[0];
        assert_eq!(title.template, "title-card");
        assert_eq!(title.data_str("title"), Some("X vs Y"));
        assert_eq!(title.data_f64("title_at"), Some(1200.0));
        assert_eq!(title.clips[0].text, "X versus Y for TOPIC.");
        assert_eq!(title.clips[0].at, Some(0.15));

        let good = &scenes[1];
        // quoted value keeps its inner `|`
        assert_eq!(good.data_str("bench"), Some("μ: 150µS|σ: 50µS"));
        assert_eq!(good.data.get("flat"), Some(&Value::Bool(true)));
        assert_eq!(good.clips[0].tempo, 1.05);
        assert_eq!(
            good.clips[0].text,
            "First line. Second line joins the same clip."
        );

        let outro = &scenes[2];
        assert!(outro.is_video_clip());
        assert_eq!(outro.data.get("audio"), Some(&Value::Bool(false)));
        assert_eq!(outro.data_f64("seek"), Some(0.0));
    }

    #[test]
    fn computes_scene_starts() {
        let mut scenes = parse(SCRIPT);
        let mut cursor = 0.0;
        for s in &mut scenes {
            s.start = cursor;
            cursor += s.duration;
        }
        assert_eq!(scenes[1].start, 2.1);
        assert!((cursor - 10.7).abs() < 1e-9);
    }

    #[test]
    fn fit_check_flags_overlap_and_truncation() {
        let mut scenes = parse(SCRIPT);
        let mut cursor = 0.0;
        for s in &mut scenes {
            s.start = cursor;
            cursor += s.duration;
        }
        let ep = Episode {
            root: PathBuf::new(),
            assets_root: PathBuf::new(),
            template_roots: vec![],
            meta: parse_meta(&split_frontmatter(SCRIPT).unwrap().0).unwrap(),
            scenes,
            total_duration: cursor, // 10.7
        };
        let clip = |scene: &str, clip: &str, duration: f64| ClipInfo {
            scene: scene.into(),
            clip: clip.into(),
            file: String::new(),
            duration,
        };
        // fits: hook (starts 0.15) ends before explain starts (2.4 + 0.2)
        let ok = vec![clip("title", "hook", 2.0), clip("good", "explain", 4.0)];
        assert!(ep.fit_check(&ok).is_empty());

        // hook clip runs past explain's start → overlap
        let overlap = vec![clip("title", "hook", 3.0), clip("good", "explain", 4.0)];
        let warnings = ep.fit_check(&overlap);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("overlap"));

        // explain clip runs past the end of the video → truncation
        let long = vec![clip("good", "explain", 20.0)];
        let warnings = ep.fit_check(&long);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("truncated"));
    }

    #[test]
    fn rejects_script_without_scenes() {
        assert!(parse_scenes("just prose, no markers").is_err());
    }

    #[test]
    fn parses_packs_from_frontmatter() {
        let meta = parse_meta("packs: ../creator-a, /abs/b\n").unwrap();
        assert_eq!(meta.packs, vec!["../creator-a", "/abs/b"]);
    }

    #[test]
    fn template_resolution_layers_most_specific_first() {
        let base = std::env::temp_dir().join(format!("ve-packs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let mk = |root: &str, name: &str| {
            let dir = base.join(root).join("templates/scenes");
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(format!("{name}.html")), "<html></html>").unwrap();
        };
        // built-ins have `stock` and `both`; the pack overrides `both` and adds `mine`
        mk("engine", "stock");
        mk("engine", "both");
        mk("pack-a", "both");
        mk("pack-a", "mine");
        fs::create_dir_all(base.join("ep")).unwrap();

        let roots = template_roots(
            &base.join("ep"),
            &["../pack-a".to_string()],
            None,
            &base.join("engine"),
        )
        .unwrap();
        assert_eq!(roots.len(), 3); // episode, pack-a, engine

        let hit = |name: &str| resolve_in_roots(&roots, name).unwrap();
        assert!(hit("stock").starts_with(base.join("engine")));
        assert!(hit("mine").ends_with("pack-a/templates/scenes/mine.html"));
        // pack overrides the built-in of the same name
        assert!(hit("both").ends_with("pack-a/templates/scenes/both.html"));
        // unknown template error lists every searched location
        let err = resolve_in_roots(&roots, "nope").unwrap_err().to_string();
        assert!(err.contains("pack-a") && err.contains("engine"));

        // a declared pack that doesn't exist is an error, not a silent skip
        assert!(
            template_roots(
                &base.join("ep"),
                &["../missing".into()],
                None,
                &base.join("engine")
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn rejects_missing_frontmatter() {
        assert!(split_frontmatter("no frontmatter here").is_err());
    }
}
