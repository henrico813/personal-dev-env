.PHONY: build check test clean install

build:
	cargo build --release

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test
	cargo build

test:
	cargo test

install:
	cargo install --path . --root $(HOME)/.local

clean:
	cargo clean
