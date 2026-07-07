//! Locating `templates/` and `formats/`.
//!
//! Both directories are embedded in the binary at compile time, so a plain
//! `cargo install videoeditor` works with no extra setup: on first use they
//! are extracted to `~/.cache/videoeditor/<version>/`.
//!
//! An on-disk root always wins (for hacking on templates). Resolution order:
//! `$VIDEOEDITOR_ROOT` → exe/../../.. (a source checkout's target dir) →
//! cwd → extracted embedded copy.

use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

static TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");
static FORMATS: Dir = include_dir!("$CARGO_MANIFEST_DIR/formats");

pub fn find_root() -> Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    for var in ["VIDEOEDITOR_ROOT", "FORGE_ROOT"] {
        if let Ok(r) = env::var(var) {
            candidates.push(PathBuf::from(r));
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(p) = exe.ancestors().nth(3) {
            // target/<profile>/videoeditor → workspace root; templates live in
            // the CLI crate dir there
            candidates.push(p.join("crates/videoeditor"));
            candidates.push(p.to_path_buf());
        }
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd);
    }
    for c in candidates {
        if c.join("templates/scenes").is_dir() {
            return Ok(c.canonicalize()?);
        }
    }
    extracted_root()
}

/// Extract the embedded templates/formats to a per-version cache dir.
fn extracted_root() -> Result<PathBuf> {
    let base = env::home_dir()
        .map(|h| h.join(".cache/videoeditor"))
        .unwrap_or_else(|| env::temp_dir().join("videoeditor"));
    let root = base.join(env!("CARGO_PKG_VERSION"));
    if !root.join("templates/scenes").is_dir() {
        extract(&TEMPLATES, &root.join("templates"))
            .and_then(|()| extract(&FORMATS, &root.join("formats")))
            .with_context(|| format!("extracting embedded assets to {}", root.display()))?;
        println!(
            "assets: extracted templates + formats to {}",
            root.display()
        );
    }
    Ok(root)
}

fn extract(dir: &Dir, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in dir.entries() {
        let name = entry
            .path()
            .file_name()
            .context("embedded entry without a name")?;
        match entry {
            include_dir::DirEntry::Dir(d) => extract(d, &to.join(name))?,
            include_dir::DirEntry::File(f) => fs::write(to.join(name), f.contents())?,
        }
    }
    Ok(())
}

const EPISODE_CLAUDE_MD: &str = r#"# CLAUDE.md

This directory is a videoeditor episode (`script.md` in → `build/final.mp4`
out) and you are its director.

Before doing anything else, run `videoeditor guide` and follow it exactly —
it is the canonical rulebook (pipeline, script grammar, the director loop,
craft rules). It is embedded in the binary, so it is always current for the
tool version you're running; nothing in this file overrides it.

**Take initiative.** If `script.md` is still the scaffold skeleton (topic
placeholders like "X vs Y — TOPIC" / "MY TOPIC"), don't wait for detailed
instructions — run the `/direct` wizard: interview the user about their
episode, then drive the production loop end to end with them.
"#;

const DIRECT_COMMAND: &str = r#"---
description: Direct this episode end to end — interview, script, voice, render, QA
---

You are directing this videoeditor episode with the user. Run
`videoeditor guide` first and obey it; this command only adds the
human-in-the-loop process on top. Drive every stage yourself — the user
should never have to remember the pipeline.

1. ASSESS — read `script.md`. Fresh skeleton → full interview below.
   In-progress episode → summarize its state (what's scripted, voiced,
   rendered) and ask what to work on.

2. INTERVIEW — a few questions at a time, not a form:
   - Topic and shape: what's the video about? A matchup (X vs Y), an
     announcement, a tip? (`--format` choice may need redoing: meme-benchmark
     vs blank.)
   - Receipts: what REAL data backs the claims? If benchmarks are needed,
     offer to write and run the experiment first — numbers are never invented.
   - Length target, tone, who gets dunked on.
   - Assets: logos/memes/music on hand, or keep the placeholder SVGs?
     Custom look? (`videoeditor templates` to browse; `videoeditor pack init .`
     + templates/CLAUDE.md to author.)
   - Voice: keep the default preset or their ElevenLabs voice_id?

3. SCRIPT — write `script.md` per the guide's craft rules. SHOW the user the
   narration beats and get approval BEFORE running tts (it costs API credits).

4. VOICE — `videoeditor tts .`; fix every ⚠ fit-check warning by recomputing
   durations from the measured clips; re-run until clean.

5. RENDER — one scene at a time (`videoeditor render . --scene <name>`),
   read the frames in `build/frames/<scene>/`, fix ⚠ template warnings and
   layout problems, then show the user 2–3 key frames for art direction.

6. ASSEMBLE — `videoeditor assemble .`, then tell the user to watch
   `build/final.mp4` and iterate on their notes. Done means they watched it.
"#;

/// Scaffold a new episode directory from a format skeleton. Formats ship
/// starter assets (placeholder SVG memes/logos, code panels) so the scaffold
/// renders out of the box; users replace them as the episode takes shape.
pub fn scaffold(dir: &Path, format: &str) -> Result<()> {
    let root = find_root()?;
    let format_dir = root.join("formats").join(format);
    let skeleton = format_dir.join("skeleton.md");
    if !skeleton.exists() {
        bail!("unknown format `{format}` ({} missing)", skeleton.display());
    }
    let dest = dir.join("script.md");
    if dest.exists() {
        bail!("{} already exists", dest.display());
    }
    fs::create_dir_all(dir.join("assets/code"))?;
    fs::create_dir_all(dir.join("assets/memes"))?;
    fs::create_dir_all(dir.join("assets/logos"))?;
    fs::create_dir_all(dir.join("assets/clips"))?;
    fs::create_dir_all(dir.join("assets/music"))?;
    fs::create_dir_all(dir.join("audio/clips"))?;
    fs::create_dir_all(dir.join("build"))?;
    // per-video templates: anything here wins over packs and built-ins
    fs::create_dir_all(dir.join("templates/scenes"))?;
    if format_dir.join("assets").is_dir() {
        copy_tree(&format_dir.join("assets"), &dir.join("assets"))?;
    }
    fs::copy(&skeleton, &dest)?;
    if !dir.join("CLAUDE.md").exists() {
        fs::write(dir.join("CLAUDE.md"), EPISODE_CLAUDE_MD)?;
    }
    // the /direct wizard: drives interview → script → tts → render → assemble
    fs::create_dir_all(dir.join(".claude/commands"))?;
    if !dir.join(".claude/commands/direct.md").exists() {
        fs::write(dir.join(".claude/commands/direct.md"), DIRECT_COMMAND)?;
    }
    println!(
        "scaffolded {} from format `{format}`\n\
         next: open Claude Code here and type /direct — it interviews you and\n\
         drives script → voice → render → final.mp4 (manual path: `videoeditor guide`)",
        dir.display()
    );
    Ok(())
}

pub(crate) fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
