.PHONY: build check test try-screen

build:
	cargo build --locked --release --bin pde-comment-review-next

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

# Builds two made-up reviews in a new temporary folder and opens them with the
# real open command. Answers are written only into that folder.
try-screen:
	folder=$$(mktemp -d) && \
	cargo run --quiet --example made_up_review -- "$$folder" && \
	cd "$$folder/store-review" && \
	XDG_STATE_HOME="$$folder/state" cargo run --quiet \
		--manifest-path "$(CURDIR)/Cargo.toml" --bin pde-comment-review-next -- open; \
	echo "The made-up reviews are in $$folder"

test:
	cargo test
