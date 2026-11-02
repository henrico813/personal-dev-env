.PHONY: build check screen test

build:
	cargo build --locked --release --bin pde-comment-review-next

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

# Opens the Review Screen on a made-up Review Session. It saves nothing.
screen:
	cargo run --quiet --example made_up_review

test:
	cargo test
