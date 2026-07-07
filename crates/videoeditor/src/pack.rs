//! Template packs: self-contained directories of scene templates a creator
//! owns and versions independently of the engine.
//!
//! A pack mirrors the engine's asset layout (`<pack>/templates/scenes/*.html`
//! plus the `_lib`/`_vendor` scene runtime, vendored at init so the pack
//! renders standalone). Episodes opt in via frontmatter
//! (`packs: ../my-pack`), machines via `$VIDEOEDITOR_PACK_PATH`.

use crate::assets;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

const EXAMPLE_TEMPLATE: &str = r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<link rel="stylesheet" href="_lib/meme.css">
<script type="application/json" id="template-info">
{
  "description": "starter template: one big meme-styled title popping in",
  "keys": {
    "title": "the headline text",
    "title_at": "pop time (ms)"
  },
  "demo": { "title": "HELLO FROM MY PACK", "title_at": 500, "duration": 2 }
}
</script>
<style>
  .stage {
    width: 100vw; height: 100vh; background: #0b0e14;
    display: flex; align-items: center; justify-content: center;
  }
  .title { font-size: 96px; text-align: center; padding: 0 40px; }
</style>
</head>
<body>
<div class="stage"><div class="title meme-text" id="titleEl"></div></div>
<script src="_lib/scene.js"></script>
<script>
  // Contract: a template is a PURE FUNCTION of (data, t) — no CSS animations,
  // no timers. setupScene(d) builds static DOM once; renderScene(tMs) applies
  // the state for time t (idempotent).
  //
  // Use from a scene:
  //   [SCENE: intro | template=my-scene duration=2.5]
  //   [DATA: title="HELLO FROM MY PACK" title_at=300]
  window.setupScene = (d) => {
    titleEl.textContent = d.title || 'MY SCENE';
  };
  window.renderScene = (t) => {
    applyPop(titleEl, SCENE.d.title_at ?? 300, t);
  };
</script>
</body>
</html>
"#;

const PACK_CLAUDE_MD: &str = r#"# CLAUDE.md — you are this pack's template engineer

This directory is a videoeditor template pack: HTML scene templates rendered
frame-by-frame by headless Chrome. **The human should never hand-roll
frame-by-frame animation — that is your job.** They describe the look and the
beats; you write, render, and QA the template. Fully assisted, every time.

## The contract (non-negotiable)

A template is a PURE FUNCTION of `(SCENE.d, SCENE.t)`:

- `window.setupScene(d)` — build static DOM once from the merged `[DATA:]`
  map. Image values arrive as `data:` URIs, `code=` files as `d.codeText`;
  `d.width`, `d.height`, `d.duration` (s), `d.sceneName` are always injected.
- `window.renderScene(tMs)` — apply the visual state for time t. Pure and
  idempotent: calling it twice for the same t must paint identical pixels.
- **Never**: CSS animations/transitions, timers, `Math.random()`, network,
  video elements. The engine seeks arbitrary t and screenshots — anything
  time-driven outside `renderScene(t)` renders as garbage or flakes.

## The runtime you build on (`_lib/scene.js`, vendored here)

COMPOSE THESE PROVEN BLOCKS before writing custom curves — they are pure
functions of t, tuned on real episodes:

- `applyPop(el, atMs, t)` / `popAt` — meme overshoot pop-in (THE default).
- `applyEnter(el, atMs, t, {from:'up'|'down'|'left'|'right', dur, dist})` —
  fade + slide-in for text blocks.
- `applySlam(el, atMs, t, {from, rot})` — stamp slams huge→rest (dunk move).
- `shakeAt(atMs, t, {amp, cycles, dur})` — decaying wobble after a landing;
  returns `{x, rot}` offsets to compose into a transform.
- `pulse(t, {period, amp})` — looping attention scale; ONE per scene, max.
- `applyCount(el, atMs, t, {to, dur, decimals, prefix, suffix})` — number
  counts up to its value (the screen holds the digits).
- `splitWords(el, "lines|split by pipes")` once in setup, then
  `popWords(el, atMs, t, {wordMs})` — word-by-word benchmark-text pop.
- `typeText(el, full, atMs, t, {cps})` — plain-text typewriter with cursor.
- `kenBurns(t, durMs, {zoom, driftX, driftY})` — slow push-in transform
  string; keeps static panels alive.
- `prog(t, atMs, durMs, ease.outBack)` + `ease.{linear,outCubic,inOutSine,outBack}`
  — clamped eased progress for anything custom.
- `flicker(t, seed)` — deterministic noise in [-1, 1] (layered sines).
- `_lib/meme.css` — `.meme-text` (Impact, white, black outline).
- `_vendor/highlight.min.js` + theme CSS — syntax highlighting for code.
- Cue convention: expose every timed element as a `[DATA:]` key ending in
  `_at` (document ms vs s in a comment) so timing is recomputed from measured
  narration, never hardcoded.

The built-in `stat-card.html` is the reference composition — enter, countUp,
shake, pulse, slam, and kenBurns working together in ~30 lines of render code.

## Your working loop with the human

1. **Interview first**: mood, palette, typography, one reference image or
   video they like, and what must pop WHEN the narration says it.
2. Write the template into `templates/scenes/<name>.html`.
3. **Debug without the engine**: open the file in a browser with
   `?d=<base64url JSON>&t=<ms>` — `_lib/scene.js` hydrates from the query
   string. Step t through the beat times.
4. Wire it into a scratch episode (`packs: <this pack>` frontmatter) and
   render ONE scene: `videoeditor render <episode> --scene <name>`.
5. **READ the extracted frames** in `<episode>/build/frames/<scene>/` at the
   cue times and the last frame. Layout bugs hide here: clipping, empty
   backdrops, premature pops, unreadable text.
6. Iterate 2–5 until the frames look right, then show the human the frames
   (not the code) and ask for art direction, not implementation.

## Discoverability (do this in every template)

Ship a `<script type="application/json" id="template-info">` block with
`description`, `keys` (each data key, with units — ms vs s matters), and
`demo` (self-contained sample data incl. `duration`; inline `codeText`/data
URIs, no file paths). `videoeditor templates` lists the repertoire from these
blocks and `videoeditor preview <name>` renders the demo into a contact
sheet — a template without one is invisible to users browsing for scenes.

## Self-diagnostics (do this in every template)

Define `window.sceneWarnings()` returning an array of strings; the engine
calls it after setup and prints each as a `⚠` during render. Check whatever
can silently go wrong in YOUR layout — text overflowing a clipped container
is the classic (compare `scrollWidth`/`scrollHeight` against the container).
See the built-in `code-meme.html` for a reference implementation.

## Design rules that keep shorts watchable

- One moving element per beat — sequential motion reads clean.
- Anything the viewer must read holds ≥1.5s.
- 1080×1920 portrait by default; design for a phone at arm's length —
  min ~34px code, ~60px labels, ~90px headline.
- Every timed element gets its own `*_at` data key, cued to narration.

`videoeditor pack list <episode>` shows which file each scene template
resolves to (episode dir → packs → $VIDEOEDITOR_PACK_PATH → built-ins).
"#;

const PACK_README: &str = r#"# A videoeditor template pack

Scene templates live in `templates/scenes/<name>.html`. Each is a pure
function of (data, t): the engine injects the merged `[DATA:]` map, then
screenshots the page at each frame time. `_lib/` holds the scene runtime
(vendored — the pack renders standalone) and `_vendor/` the syntax
highlighter.

Use the pack from an episode by declaring it in `script.md` frontmatter:

```markdown
---
title: My Episode
packs: ../path-to-this-pack
---

[SCENE: intro | template=my-scene duration=2.5]
[DATA: title="HELLO" title_at=300]
```

Resolution is layered, most specific first: the episode's own `templates/`
→ frontmatter `packs:` (in order) → `$VIDEOEDITOR_PACK_PATH` → the engine
built-ins. A pack template with the same name as a built-in overrides it.
Check what an episode resolves with `videoeditor pack list <episode>`.

**Don't hand-write frame-by-frame animation.** Open Claude Code in this
directory — the bundled CLAUDE.md turns it into this pack's template
engineer: describe the look and the beats, and it writes, renders, and
frame-QAs the template with you.
"#;

/// Scaffold a self-contained pack: example template + vendored scene runtime.
/// Also works on an episode dir — an episode's `templates/` is just the most
/// specific pack layer, so this is how a single video gets unique templates.
pub fn init(dir: &Path) -> Result<()> {
    let scenes = dir.join("templates/scenes");
    if scenes.join("_lib").exists() || scenes.join("my-scene.html").exists() {
        bail!("{} is already a pack", scenes.display());
    }
    fs::create_dir_all(&scenes)?;

    // vendor the scene runtime so the pack renders without the engine root
    let engine_scenes = assets::find_root()?.join("templates/scenes");
    for lib in ["_lib", "_vendor"] {
        assets::copy_tree(&engine_scenes.join(lib), &scenes.join(lib))
            .with_context(|| format!("vendoring {lib} from {}", engine_scenes.display()))?;
    }

    fs::write(scenes.join("my-scene.html"), EXAMPLE_TEMPLATE)?;
    fs::write(dir.join("README.md"), PACK_README)?;
    fs::write(dir.join("CLAUDE.md"), PACK_CLAUDE_MD)?;
    let usage = if dir.join("script.md").exists() {
        "this is an episode dir — its templates/ is resolution layer 1, no `packs:` line needed"
    } else {
        "declare it in an episode with `packs: <path-to-pack>` frontmatter"
    };
    println!(
        "pack: scaffolded {} (example template: my-scene)\n\
         pack: {usage}\n\
         pack: don't hand-write animation — open Claude Code here; CLAUDE.md \
         makes it this pack's template engineer",
        dir.display()
    );
    Ok(())
}

/// Show an episode's template resolution: layers in order, then every scene's
/// template and the file it resolves to.
pub fn list(ep: &videoeditor_timeline::Episode) -> Result<()> {
    println!("resolution layers (most specific first):");
    for (i, root) in ep.template_roots.iter().enumerate() {
        let n = fs::read_dir(root.join("templates/scenes"))
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
                    .count()
            })
            .unwrap_or(0);
        println!("  {}. {} ({n} templates)", i + 1, root.display());
    }
    println!("\nscene templates:");
    for scene in &ep.scenes {
        if scene.is_video_clip() {
            println!("  {:<12} video-clip (ffmpeg passthrough)", scene.name);
            continue;
        }
        match ep.resolve_template(&scene.template) {
            Ok(path) => println!(
                "  {:<12} {} → {}",
                scene.name,
                scene.template,
                path.display()
            ),
            Err(_) => println!("  {:<12} {} → NOT FOUND", scene.name, scene.template),
        }
    }
    Ok(())
}
