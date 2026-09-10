# Rust examples

This is a Rust 2021 package containing the worked examples used by the skill.
It has no third-party dependencies. Keep using your project's own edition,
supported compiler, dependency choices, and layout in production; this package
does not prescribe them.

## Read an example

Start with the [example index](../references/examples.md). It links each design
question to its explanation and full source. Modules group separate teaching
examples; the `retry-count` binary uses only the retry parser.

## Check the code

From this directory, run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --offline -- -D warnings
cargo test --offline
cargo doc --no-deps --offline
```

`cargo fmt --all` applies formatting when changes are needed. `cargo test` runs
local tests, integration tests, and the library's doctests. Do not replace it
with `cargo test --all-targets` and assume doctests are still included.

The package includes `Cargo.lock`; use `--locked` for reproducible checks.
No dependency downloads are needed, but Rust, Cargo, rustfmt, and Clippy must
already be installed. The exact supported compiler must still be checked for
any project that adopts the code; Rust 2021 is an edition, not a complete MSRV
promise.

The command-line example can be run with:

```sh
cargo run --offline --bin retry-count -- " 3 "
```

It prints `3` and succeeds. Missing or invalid input returns a failure exit
status and writes an error to stderr. The process-level tests cover this behavior.

For the unsafe byte-copy tests, use Miri through the repository's supported
nightly setup when available. Do not run process-launching CLI tests under Miri
and assume it supports them; target the applicable library tests, for example
`cargo +nightly miri test --lib foreign::tests` with Miri installed. Passing Miri
is not a proof of soundness.

## Validation status

Package structure, local links and headings, frontmatter, and the Cargo manifest
were checked during this revision. Formatting, Clippy, tests, and documentation
checks pass offline. Agent activation, Miri, and the proposed evaluation cases
have not been run.

References: [cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html),
[Clippy](https://doc.rust-lang.org/clippy/usage.html), and
[Miri](https://github.com/rust-lang/miri).
