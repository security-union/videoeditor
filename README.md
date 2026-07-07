# videoeditor

Scripted short-video renderer for developers who'd rather write markdown than
open DaVinci. `script.md` in → rendered vertical video out.

![five scenes from the bundled hello-bench example](docs/demo.jpg)
*The bundled `examples/hello-bench` episode — five scenes, rendered entirely
from markdown + SVG placeholders. This strip is the actual output.*

- **Rust** orchestrates everything (`videoeditor`, one binary).
- **Web tech** does animation & composition: scenes are HTML/CSS/JS templates,
  rendered frame-by-frame by headless Chrome as pure functions of `(data, t)` —
  deterministic, no flaky screen recording.
- **ffmpeg** does the heavy lifting: scene encodes, concat, audio mix.
- **ElevenLabs** voices the narration and transcribes reference videos.
- **Built to be driven by [Claude Code](https://claude.com/claude-code)** —
  the repo ships the production rulebook an AI director follows (see below).

## Bootstrap

### 1. Install nix (once)

Nix is the preferred path: one command builds the pinned binary AND its whole
media toolchain deterministically from the committed `flake.lock` — the same
rustc, ffmpeg, and browser on every machine. Install it with the
[Determinate Systems installer](https://install.determinate.systems):

```bash
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
```

### 2. Install videoeditor

```bash
nix profile install github:security-union/videoeditor   # install the CLI
# …or try it without installing:
nix run github:security-union/videoeditor -- --help
```

The nix-built binary is wrapped so **every** runtime dependency is pinned —
ffmpeg on all systems, nixpkgs chromium on Linux, and on macOS the
free-licensed **Chrome for Testing** from the playwright browser bundle
(nixpkgs' `chromium` is Linux-only and `google-chrome` is unfree; `CHROME_BIN`
still overrides if you prefer your own). Nothing to apt/brew install, no
version drift: the same flake.lock renders the same pixels everywhere. The
only thing outside the pin is your **`ELEVENLABS_API_KEY`**
([elevenlabs.io](https://elevenlabs.io) → profile → API keys; free tier is
enough for shorts), needed only by `tts` and `analyze` — `parse`, `new`,
`render`, and `assemble` of already-voiced episodes run keyless.

Contributors: `nix develop` (or `direnv allow` — `.envrc` is committed) drops
you in the pinned dev shell with rustc, ffmpeg, just, and rust-analyzer;
`just --list` shows the task recipes.

The pinned browser is an ordinary Chrome for Testing binary and `$CHROME_BIN`
points at it in the dev shell — any CDP client can reuse it. If you script
against the templates with puppeteer or playwright, aim them at the same
binary (`PUPPETEER_SKIP_DOWNLOAD=1` +
`puppeteer.launch({ executablePath: process.env.CHROME_BIN })`, or
`PLAYWRIGHT_BROWSERS_PATH`) instead of letting them download their own —
one browser build, identical pixels everywhere.

### Without nix

`videoeditor` is a single Rust binary that orchestrates tools on your PATH
(the yt-dlp model — templates and formats are embedded and self-extract to
`~/.cache/videoeditor/<version>/`, so the crate is a complete install; ffmpeg
and Chrome you bring yourself):

```bash
cargo install videoeditor        # or: cargo install --path crates/videoeditor
```

| Dependency | Needed for | Check | Get it |
|---|---|---|---|
| ffmpeg + ffprobe | `render`, `assemble`, `analyze` | `ffmpeg -version` | `brew install ffmpeg` · `apt install ffmpeg` · `dnf install ffmpeg` |
| Chrome / Chromium | `render` (web scenes), `grab` | `"$CHROME_BIN" --version` or a normal install | [google.com/chrome](https://www.google.com/chrome/) — auto-detected on macOS and Linux; `CHROME_BIN=/path/to/chrome` to override |
| `ELEVENLABS_API_KEY` | `tts`, `analyze` only | `echo $ELEVENLABS_API_KEY` | [elevenlabs.io](https://elevenlabs.io) |

macOS and Linux are supported; on Windows use WSL.

### 3. Your first video

```bash
export ELEVENLABS_API_KEY=...      # https://elevenlabs.io → profile → API keys

videoeditor new my-first-short     # runnable scaffold: placeholder SVGs + code included
videoeditor build my-first-short   # tts → render → assemble
open my-first-short/build/final.mp4
```

The scaffold renders as-is — replace the placeholder narration, code panels,
and SVG memes as your episode takes shape. (The bundled
`examples/hello-bench` needs a source checkout: `git clone` this repo, then
`videoeditor build examples/hello-bench`.)

Iterating: `videoeditor tts <dir>` re-voices only missing/changed clips
(`--clip name --force` to re-roll one take) and prints ⚠ fit-check warnings
when narration overlaps; `videoeditor render <dir> --scene name` re-renders a
single scene; `videoeditor assemble <dir>` re-mixes in seconds. Hacking on the
HTML templates? Point `VIDEOEDITOR_ROOT` at your checkout and edits apply
without rebuilding.

### 4. Let Claude direct

This tool is designed to pair with **[Claude Code](https://claude.com/claude-code)**
as the director: the repo ships [PRODUCTION.md](PRODUCTION.md) (the craft
rulebook — real benchmark receipts, narration fit-checks, congruence rules,
frame-QA ritual) and a [CLAUDE.md](CLAUDE.md) that teaches the pipeline.
Open Claude Code in your episode directory (keep a copy of PRODUCTION.md
next to it) and ask for an episode:

> "Make a 25-second short: Rust vs Go parsing a 1GB JSON file. Run a real
> benchmark first, then script it, voice it, render it, and QA the frames."

Claude runs the benchmark, writes `script.md`, calls `videoeditor tts`,
recomputes timings from the fit-check warnings, renders scene by scene, and
inspects extracted frames — the same loop a human editor would run, minus
the human.

## Pipeline

```
script.md ──parse──► timeline plan
   │
   ├─ videoeditor tts       [CLIP:] → ElevenLabs → audio/clips/<scene>__<clip>.mp3 + clips.json
   ├─ videoeditor render    [SCENE:] → Chrome frames → ffmpeg → build/scenes/NN_name.mp4
   └─ videoeditor assemble  concat + narration@offsets + clip audio + music → build/final.mp4
```

`videoeditor build <dir>` runs all three. Extras: `analyze` (transcript +
scene-cut timing table of a reference video), `new` (scaffold an episode),
`grab` (fetch a URL through your own logged-in Chrome), `parse` (dump the
resolved plan as JSON).

## Workspace layout (ffmpeg-style: thin CLI over focused libraries)

| Crate | Role | Analogy |
|-------|------|---------|
| [`videoeditor`](crates/videoeditor) | CLI + scene orchestration + embedded templates | `ffmpeg` the binary |
| [`videoeditor-timeline`](crates/videoeditor-timeline) | `script.md` parser → typed timeline model | `libavformat` |
| [`videoeditor-chrome`](crates/videoeditor-chrome) | headless-Chrome CDP driver (frame capture, page grab) | `libavdevice` |
| [`videoeditor-media`](crates/videoeditor-media) | everything that shells out to ffmpeg: encodes, concat, audio mix, scene cuts | `libavcodec` |
| [`videoeditor-voice`](crates/videoeditor-voice) | ElevenLabs TTS + Scribe STT | — |

## script.md grammar

```markdown
---
title: My Short            # metadata frontmatter
fps: 30
width: 1080
height: 1920
voice_id: <elevenlabs id>  # e.g. pNInz6obpgDQGcFmaJgB ("Adam", a public preset)
music: assets/music/bed.mp3
music_gain_db: -20
---

[SCENE: name | template=code-meme duration=6.42]
[DATA: code=assets/code/threads.rs meme=assets/memes/happy.svg pointer=true]
[DATA: bench="Execution time:|μ: 150µS|σ: 50µS" bench_at=5.8]
[CLIP: explain | at=0.19]
Narration text until the next marker. `at` = seconds from scene start
(omit to auto-place after the previous clip). `tempo=1.05` speeds the clip.

[SCENE: outro | template=video-clip duration=2.2]
[DATA: src=assets/clips/punchline.mp4 seek=0]   # keeps native audio; audio=false to mute
```

- Scene `duration` is authoritative — narration is placed inside it. This is
  how you clone a reference video's timing exactly.
- All paths are relative to the episode directory.
- Unknown `[MARKERS:]` and `<!-- comments -->` are ignored (annotate freely).

## Scene templates (`templates/scenes/`)

A template is one HTML file. Contract: the renderer loads the page, injects the
merged `[DATA:]` map via CDP, then per frame calls `__sceneSeek(t)` and
screenshots. Everything visible must derive from `SCENE.d` (data keys, asset
paths already inlined as data: URIs, plus `codeText`, `duration`, `width`,
`height`) and `SCENE.t`. No CSS animations, no timers — pure state.

| Template | Purpose | Data keys |
|----------|---------|-----------|
| `title-card` | X-vs-Y hook: logos, VS, flame, popping title | `left`, `right`, `left_label`, `right_label`, `title`, `*_at` (ms) |
| `code-meme` | top: highlighted code, bottom: meme + popping benchmark | `code`, `lang`, `meme`, `badge`, `label`, `pointer`, `pointer_from/to` (s), `bench` (lines split by `\|`), `bench_at` (s), `typing` |
| `duel-table` | two-column concept duel (X is for…, Y is for…) | `title`, `left/right(+_label)`, `rows="a:b\|…"`, `row_pops="l:r,…"` (s) |
| `scoreboard` | final ranking, winner green / loser red | `title`, `rows="name value\|…"` |
| `video-clip` | not HTML — ffmpeg passthrough of `src` (trim/scale/native audio) | `src`, `seek`, `audio`, `crop_top`, `caption` |

Add a template = drop an HTML file in `templates/scenes/`. Zero engine changes.

## Template packs — your look, outside the engine

The built-ins are a starting point, not your brand. A **pack** is a
self-contained directory of scene templates a creator owns and versions
independently (a git repo of its own, shared between channels, whatever):

```bash
videoeditor pack init creator-a     # scaffold: example template + vendored scene runtime
```

```markdown
---
title: My Episode
packs: ../creator-a, ../shared-lower-thirds   # comma-separated, episode-relative
---

[SCENE: intro | template=my-scene duration=2.5]
```

Resolution is layered, most specific wins — so creator A and creator B render
the same `script.md` grammar through entirely different visual identities:

1. the episode's own `templates/scenes/` (one-off scenes)
2. frontmatter `packs:` in order
3. `$VIDEOEDITOR_PACK_PATH` (colon-separated, machine-level)
4. the engine built-ins

A pack file named like a built-in **overrides** it. `videoeditor pack list
<episode>` prints the layers and exactly which file every scene resolves to;
renders log the source when a template comes from a pack.

**Don't hand-write frame-by-frame animation.** `pack init` drops a `CLAUDE.md`
into the pack that turns Claude Code into that pack's template engineer — it
knows the `(data, t)` contract, the runtime helpers, and the render-one-scene →
inspect-frames QA loop. You describe the look ("neon terminal, CRT flicker,
title slams in when the voice says the name"); Claude writes the template,
renders it, reads the frames, and iterates with you.

## Formats (`formats/<name>/`)

A format is a narrative spine over the same machinery: `spec.md` (the rules) +
`skeleton.md` (what `videoeditor new` copies). First format: **meme-benchmark**
— the viral "X vs Y with receipts" ~20-second shape.

Production craft — how to make these videos actually good (real benchmark
receipts, congruence between audio and screen, pacing, review ritual) — lives
in [PRODUCTION.md](PRODUCTION.md).

## Episode layout

```
my-short/
├── script.md          # source of truth
├── assets/{code,memes,clips,logos,music}/
├── audio/clips/       # generated narration (name-keyed) — regenerable
└── build/             # frames/, scenes/, final.mp4 — disposable
```

## Packaging & releases

Two distribution channels, both fed from the same tag:

- **nix flake (preferred)** — `nix profile install github:security-union/videoeditor`
  builds from the committed `flake.lock`: pinned rustc, pinned ffmpeg, pinned
  browser (chromium on Linux, playwright's Chrome for Testing on macOS),
  binary wrapped so those exact versions are found at runtime. Fully
  deterministic; CI builds the flake on every PR.
- **crates.io** — released with [release-plz](https://release-plz.dev):
  merging the release PR bumps versions, updates changelogs, tags, and
  publishes all five crates. The crate is a complete install (templates
  embedded), but ffmpeg/Chrome come from your system: ffmpeg's GPL-licensed
  builds can't ship inside an MIT binary, and staying a thin orchestrator
  keeps the binary a few megabytes instead of a few hundred.

Developing: `nix develop` (or bring your own rustc + ffmpeg), then
`just --list` for the task recipes — CI runs the same `just check`,
`just test`, `just build` you run locally.

## License

MIT. Syntax highlighting via [highlight.js](https://highlightjs.org)
(BSD-3-Clause, vendored in `templates/scenes/_vendor/`).
