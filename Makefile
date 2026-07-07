EPISODE ?= examples/hello-bench
RUN     := cargo run -p videoeditor --release --quiet --

.PHONY: build check test tts render assemble episode analyze new

build:
	cargo build --release

check:
	cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check

test:
	cargo test --workspace

tts:
	$(RUN) tts $(EPISODE)

render:
	$(RUN) render $(EPISODE)

assemble:
	$(RUN) assemble $(EPISODE)

episode:
	$(RUN) build $(EPISODE)

# make analyze REF=path/to/viral.mp4
analyze:
	$(RUN) analyze "$(REF)"
