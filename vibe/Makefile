.PHONY: build check test integration clean install

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
	cargo test --test pi_model_config -- --ignored

install:
	cargo install --path . --root $(HOME)/.local

clean:
	cargo clean
