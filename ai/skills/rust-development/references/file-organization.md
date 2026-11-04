# File organization

Organize modules by responsibility, not by categories such as `structs`, `traits`,
and `helpers`. Keep related types and their implementations together. Split a
file when it contains distinct responsibilities, not at an arbitrary line count.
Avoid catch-all `utils.rs` modules and one-file-per-type rules.

For a growing CLI, a common layout is:

```text
Cargo.toml
src/
  main.rs          # Arguments, setup, and exit behavior.
  lib.rs           # Library entry point and module declarations.
  config.rs        # Configuration behavior and its local tests.
  report.rs        # Report types and behavior.
tests/
  cli.rs           # Observable command-line behavior.
```

Use only the files needed. A tiny program can stay in `main.rs`; do not create
an empty scaffold. In a binary-plus-library package, import library code rather
than declaring the same modules again in the binary. Prefer `mod` and
`pub(crate)` over unnecessary `pub`; deliberately expose only the supported API.
Follow existing module layout rather than renaming files for style alone.

`mod config;` declares a module; `use crate::config::Config;` brings an existing
item into scope. They do not do the same job. Both `config.rs` with a `config/`
subdirectory and a `config/mod.rs` layout can represent a module with children.
Follow the repository's layout unless changing it solves a concrete problem.

Prefer explicit imports. Group them by standard library, external dependencies,
and this crate when the repository has no established convention. Avoid wildcard
imports except for deliberate preludes or narrow test modules. Keep important
entry points easy to find, with helpers nearby and tests after the implementation.
Use re-exports deliberately to give callers a stable interface, not to hide a
tangled dependency structure. Preserve established paths when compatibility
requires them; consult the [API checklist](api-design.md) before
changing public paths or bounds.
