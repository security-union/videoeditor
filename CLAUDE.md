# CLAUDE.md

Rust workspace: `videoeditor` — scripted short-video renderer (markdown in,
vertical video out via headless Chrome + ffmpeg + ElevenLabs).

## Layout

- `crates/videoeditor` — CLI + scene orchestration; embeds `templates/` and
  `formats/` (extracted to `~/.cache/videoeditor/<version>/` at runtime;
  `VIDEOEDITOR_ROOT` overrides).
- `crates/videoeditor-timeline` — script.md parser → `Episode`/`Scene`/`Clip`.
- `crates/videoeditor-chrome` — CDP driver (long-lived headless Chrome; NEVER
  single-shot `--screenshot`, it hangs on macOS).
- `crates/videoeditor-media` — all ffmpeg/ffprobe invocations + assembly.
- `crates/videoeditor-voice` — ElevenLabs TTS/STT (`ELEVENLABS_API_KEY`).
- `crates/videoeditor-genai` — typed image-generation clients: xAI Grok
  Imagine (`XAI_API_KEY`, reference images) + Google Imagen (`AI_STUDIO`);
  Veo/Grok video is the planned next tenant.
- `crates/videoeditor-record` — `record` subcommand: local web recorder
  (tiny_http REST server); takes transcode via ffmpeg into `audio/clips/`.
  Serves the UI from `ui/` — a COMMITTED trunk build of the next crate.
  Never edit `ui/` by hand; `just e2e` regenerates it automatically and
  `just check` fails if it's stale vs the UI source (`ui/.source-hash`).
  This crate publishes to crates.io WITH the dist — installs need no wasm.
- `crates/videoeditor-record-ui` — the recorder front end: Leptos (CSR) →
  wasm via trunk. `publish = false`: not standalone, its build output
  ships inside videoeditor-record. `nix develop` provides
  trunk/wasm-bindgen-cli (the `wasm-bindgen` crate pin must match the CLI
  version — bump together). Regenerate the dist only via the nix
  toolchain (`nix develop --command just ui`) so bytes stay stable.
- `e2e/` — Playwright suite for the recorder (`just e2e`, rebuilds the UI
  first): drives the real binary + real Chromium with a fake mic through
  record → coach → keep → approve. UI changes are not done until it passes.
- `examples/hello-bench` — smallest end-to-end episode; keep it rendering.

## Commands

- Nix is the preferred dependency path: `nix develop` (dev shell; puts a
  source-fresh `videoeditor` on PATH) / `nix build .#videoeditor` (the
  install artifact CI gates on). Details: docs/nix.md. Keep `flake.lock`
  committed; `nix flake update` only as a deliberate PR.

- `just check` — clippy -D warnings + fmt --check (CI runs the same recipe).
- `just test` / `cargo test --workspace` — parser + fit-check tests live in
  videoeditor-timeline.
- After ANY voice or timing change: re-run tts → heed the fit-check warnings
  (`⚠ narration overlap/truncated`) → recompute scene durations → re-render.
- `cargo run -p videoeditor -- build examples/hello-bench` — end-to-end smoke
  (needs Chrome, ffmpeg, ELEVENLABS_API_KEY).

## Rules

- Scene templates are PURE functions of (data, t) — no CSS animations, no
  timers, no network. Everything derives from `SCENE.d` and `SCENE.t`.
- Templates resolve in layers: episode dir → frontmatter `packs:` →
  `$VIDEOEDITOR_PACK_PATH` → built-ins (`videoeditor pack list <ep>` shows
  provenance). Users never hand-roll templates — `pack init` scaffolds a
  pack whose `templates/CLAUDE.md` is the authoring contract; author
  templates WITH the user per that file.
- Releases go through release-plz (PRs to main; never bump versions by hand).
- Never commit media renders or API keys. Committed images go through git-lfs.
- Production craft for episode content lives in crates/videoeditor/guide.md
  (embedded; printed by `videoeditor guide`) — the single source of truth.
  Never duplicate its rules into other docs; point at the command instead.
