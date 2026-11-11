.PHONY: build check test

# hub --version prints $HUB_VERSION when set, else the newest hub-v
# tag from git describe, else Cargo.toml's version. See build.rs.
build:
	cargo build --locked --release --bin hub

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

test:
	cargo test
