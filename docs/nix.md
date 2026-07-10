# Nix: deterministic installs, every way to run, releases

## Why nix is the preferred path

The flake builds the binary AND its whole media toolchain from the committed
`flake.lock`: the same rustc, the same ffmpeg, the same browser on every
machine — the same lockfile renders the same pixels. The installed binary is
wrapped so its pinned dependencies are always found; nothing to `apt`/`brew`
install, no version drift.

The pinned browser: nixpkgs `chromium` on Linux; on macOS, the free-licensed
**Chrome for Testing** from the playwright browser bundle (nixpkgs' `chromium`
is Linux-only and `google-chrome` is unfree). `CHROME_BIN` overrides
everywhere if you prefer your own. Templates and formats ship inside the
package as a store path (`VIDEOEDITOR_ROOT` baked into the wrapper), so they
can never go stale.

The pinned speech stack: whisper.cpp (`whisper-cli` + ggml-base.en weights)
for speech-to-text and a piper voice (en_US-lessac-medium via sherpa-onnx)
for narration, so `tts`, `analyze`, and the recorder's coach all run
offline with no API key. `WHISPER_MODEL` / `PIPER_VOICE` override the baked
models; the elevenlabs backends remain an opt-in (`tts: elevenlabs`
frontmatter, `VIDEOEDITOR_STT=elevenlabs`).

## Every way to run it

```bash
nix profile install github:security-union/videoeditor   # permanent, on PATH
nix profile install .                                    # same, from a local clone
nix profile upgrade videoeditor                          # refresh after changes
nix shell .#videoeditor                                  # binary on PATH for this shell only
nix run . -- build my-short                              # one-off (apps.default)
nix run .#videoeditor -- --help                          # same, addressing the package
nix build .#videoeditor && ./result/bin/videoeditor      # build without installing
```

## The dev shell

`nix develop` (or `direnv allow` — `.envrc` is committed) provides rustc,
cargo, clippy, rustfmt, rust-analyzer, ffmpeg, just — **and a `videoeditor`
command** that execs `cargo run --release` against the checkout you entered
from. It is always source-fresh: no stale store binary shadowing your edits;
the first call compiles, cargo caches the rest. `just --list` shows the task
recipes; CI runs the same ones.

The dev shell exports `CHROME_BIN` pointing at the pinned browser. It's an
ordinary Chrome for Testing binary, so any CDP client can share it — if you
script against templates with puppeteer or playwright, aim them at the same
binary instead of letting them download their own
(`PUPPETEER_SKIP_DOWNLOAD=1` +
`puppeteer.launch({ executablePath: process.env.CHROME_BIN })`, or
`PLAYWRIGHT_BROWSERS_PATH`): one browser build, identical pixels everywhere.

## Packaging & releases

Two distribution channels, fed from the same tag:

- **The flake** (preferred): fully pinned as above; CI builds it sandboxed on
  ubuntu + macos for every PR.
- **crates.io**: releases are automated with
  [release-plz](https://release-plz.dev) — merging the release PR bumps
  versions, updates changelogs, tags, and publishes all five crates. The
  crate is a complete install (templates embedded, self-extracted to
  `~/.cache/videoeditor/<version>/`), but ffmpeg and Chrome come from your
  system: ffmpeg's GPL-licensed builds can't ship inside an MIT binary, and
  staying a thin orchestrator keeps the binary megabytes instead of hundreds.

Keep `flake.lock` committed; `nix flake update` only as a deliberate PR.
