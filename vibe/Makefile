.PHONY: build check clean

build:
	cargo build --release

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo build

clean:
	cargo clean
