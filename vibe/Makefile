.PHONY: build check clean install

build:
	cargo build --release

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo build

install:
	cargo install --path . --root $(HOME)/.local

clean:
	cargo clean
