# Linting and formatting

Use `rustfmt` and the repository's formatting settings. Fix relevant compiler and
Clippy warnings; do not hand-format against the formatter. Add opinionated lints
selectively. Never enable the entire `clippy::restriction` group. Keep justified
lint exceptions narrow and explain them.

Follow the repository's checks. For an ordinary Cargo workspace without a
prescribed workflow, start with:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`--check` verifies formatting without changing files. Format edited code before
finishing. Run focused tests during development and broader checks afterward.
When APIs or docs change, also run `cargo doc --workspace --no-deps` or the
repository's documentation check; keep doctests in the test run. Check supported
feature and platform combinations where relevant; `--all-targets` does not cover them all,
and `--all-features` is not automatically an appropriate substitute.

Do not replace the test command with `cargo test --all-targets` and assume it
includes documentation tests. Inspect failures before attributing them to the
change. Report unavailable tools and pre-existing failures instead of hiding them.
