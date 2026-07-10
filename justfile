# `just` is the task entry point; the nix flake provides it (`nix develop`).
# CI runs these same recipes — if it passes locally it passes in CI.

run := "cargo run -p videoeditor --release --quiet --"
episode := "examples/hello-bench"

# list available recipes
default:
    @just --list

# release build of the whole workspace
build:
    cargo build --workspace --release

# lint gate: clippy -D warnings + rustfmt + embedded-UI freshness
check: ui-check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

test:
    cargo test --workspace

# everything CI runs
ci: check test build

# rebuild the recorder UI (Leptos → wasm via trunk) and refresh the
# committed dist that videoeditor-record embeds, stamped with the hash of
# the source that produced it (see ui-check)
ui:
    cd crates/videoeditor-record-ui && trunk build
    rm -rf crates/videoeditor-record/ui
    cp -R crates/videoeditor-record-ui/dist crates/videoeditor-record/ui
    just _ui-stamp > crates/videoeditor-record/ui/.source-hash

# drift guard, no wasm toolchain needed: fails when the recorder UI source
# changed but the committed dist wasn't regenerated. Keeps the embedded
# front end trustworthy without any remember-to-run step — CI runs this
# via `just check` on every push.
ui-check:
    @test "$(cat crates/videoeditor-record/ui/.source-hash 2>/dev/null)" = "$(just _ui-stamp)" \
      || { echo "error: committed recorder UI (crates/videoeditor-record/ui) is stale."; \
           echo "       run: nix develop --command just ui   — then commit the refreshed ui/"; exit 1; }

# hash of everything that determines the UI build (source + toolchain pins)
_ui-stamp:
    @find crates/videoeditor-record-ui/src crates/videoeditor-record-ui/index.html \
      crates/videoeditor-record-ui/style.css crates/videoeditor-record-ui/Trunk.toml \
      crates/videoeditor-record-ui/Cargo.toml -type f | LC_ALL=C sort \
      | xargs shasum -a 256 | shasum -a 256 | cut -d' ' -f1

# recorder end-to-end suite: real binary + real Chromium (fake mic).
# Depends on `ui` so the embedded front end is always freshly built from
# source — no manual rebuild step, ever.
e2e: ui
    cargo build -p videoeditor
    cd e2e && playwright test

# generate narration clips (needs ELEVENLABS_API_KEY)
tts episode=episode:
    {{run}} tts {{episode}}

# render all scenes (or one: just render examples/hello-bench --scene title)
render episode=episode *args:
    {{run}} render {{episode}} {{args}}

assemble episode=episode:
    {{run}} assemble {{episode}}

# full pipeline: tts + render + assemble
episode episode=episode:
    {{run}} build {{episode}}

# study a reference video: just analyze path/to/viral.mp4
analyze ref:
    {{run}} analyze "{{ref}}"
