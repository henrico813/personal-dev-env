.PHONY: build check test

build:
	cargo build --locked --release --bin hub

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

test:
	cargo test
