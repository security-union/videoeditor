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

# lint gate: clippy -D warnings + rustfmt
check:
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --all --check

test:
    cargo test --workspace

# everything CI runs
ci: check test build

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
