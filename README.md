# videoeditor

Scripted short-video renderer for developers who'd rather write markdown than
open DaVinci. `script.md` in → rendered vertical video out.

- **Rust** orchestrates everything (`videoeditor`, one binary).
- **Web tech** does animation & composition: scenes are HTML/CSS/JS templates,
  rendered frame-by-frame by headless Chrome as pure functions of `(data, t)` —
  deterministic, no flaky screen recording.
- **ffmpeg** does the heavy lifting: scene encodes, concat, audio mix.
- **ElevenLabs** voices the narration and transcribes reference videos.

## Quickstart

```bash
cargo install videoeditor        # or: git clone && cargo build --release
export ELEVENLABS_API_KEY=...    # https://elevenlabs.io → profile → API keys

# render the bundled example end to end
videoeditor build examples/hello-bench
open examples/hello-bench/build/final.mp4

# start your own
videoeditor new my-first-short   # scaffold from formats/meme-benchmark
$EDITOR my-first-short/script.md
videoeditor build my-first-short
```

Requires: **Chrome** (system install; `CHROME_BIN` to override) and **ffmpeg**
on PATH. Scene templates and format skeletons are embedded in the binary and
extracted to `~/.cache/videoeditor/<version>/` on first run — set
`VIDEOEDITOR_ROOT` to point at a checkout when hacking on templates.

There's also a nix flake (`nix develop`) that pins the toolchain + ffmpeg.

## Pipeline

```
script.md ──parse──► timeline plan
   │
   ├─ videoeditor tts       [CHUNK:] → ElevenLabs → audio/clips/<scene>__<chunk>.mp3 + clips.json
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
[CHUNK: explain | at=0.19]
Narration text until the next marker. `at` = seconds from scene start
(omit to auto-place after the previous chunk). `tempo=1.05` speeds the clip.

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

## License

MIT. Syntax highlighting via [highlight.js](https://highlightjs.org)
(BSD-3-Clause, vendored in `templates/scenes/_vendor/`).
