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
    if format_dir.join("assets").is_dir() {
        copy_tree(&format_dir.join("assets"), &dir.join("assets"))?;
    }
    fs::copy(&skeleton, &dest)?;
    println!("scaffolded {} from format `{format}`", dir.display());
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
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
