.PHONY: build check test clean install

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

install:
	cargo install --path . --root $(HOME)/.local

clean:
	cargo clean
