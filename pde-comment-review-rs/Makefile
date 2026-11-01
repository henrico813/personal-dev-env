.PHONY: build check test

build:
	cargo build --locked --release --bin pde-comment-review-next

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

test:
	cargo test
