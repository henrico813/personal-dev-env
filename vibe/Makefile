.PHONY: build check test integration integration-compatible clean install

build:
	cargo build --release

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test
	node extensions/stderr-progress.test.mjs
	cargo build

test:
	cargo test
	node extensions/stderr-progress.test.mjs

integration:
	cargo test --test vibe_model_selection vibe_forwards_model_selector_to_pi -- --ignored --exact

integration-compatible:
	cargo test --test vibe_model_selection vibe_discovers_compatible_model -- --ignored --exact

install:
	cargo install --path . --root $(HOME)/.local

clean:
	cargo clean
