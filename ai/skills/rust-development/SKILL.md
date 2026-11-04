---
name: rust-development
description: >-
  Plans, writes, reviews, refactors, and tests maintainable Rust code. Use when
  planning changes to Rust code or working on Rust implementation, debugging,
  API design, code review, documentation, tests, modules, Cargo configuration,
  dependencies, or features. Covers ownership, traits, errors, resource cleanup,
  concurrency, and unsafe code with explanations suitable for a junior
  maintainer. Do not use it merely because a repository contains Rust.
---

# Rust development

Write straightforward, idiomatic Rust. Prioritize simplicity, maintainability,
and testability. The maintainer is learning Rust.
Explain unfamiliar features and the decisions behind them. Keep important Rust
practices even when they need explanation. The result should be code the
maintainer can understand, test, and change.

## Workflow

Read the relevant code, tests, repository instructions, `Cargo.toml`, toolchain
settings, and CI commands before changing anything. Identify the required
behavior and the production API that provides it. Respect the supported Rust
version, platforms, features, and existing compatibility promises.

Make the smallest complete change. Update affected tests and documentation,
then verify it. Do not introduce unrelated refactors, dependencies, public APIs,
or toolchain upgrades.

Use Microsoft's guide as the primary design reference. Consult rust-analyzer's
style guide and *How to Test* first for readability and testing; use *Effective
Rust* for harder tradeoffs. These are defaults, not reasons to disregard the
project's needs. Do not sacrifice correctness or memory safety for a style preference.
Official language and tool documentation settles questions of behavior; project
conventions settle style choices. Do not silently change a compatibility promise.

Read these references for almost every change:

- [Readable code](references/readable-code.md) before writing or
  explaining code.
- [Naming](references/naming.md) before naming anything, tests
  included.
- [Testing](references/testing.md) before adding or changing a test.
- [Linting and formatting](references/linting-formatting.md) for the
  commands to run before finishing.

Before changing or reviewing any area below, read its matching reference. Apply
the relevant checks, not every technique. These local references are required
for the listed tasks; fetching every external guide is not.

| Topic | Reference | When to read it |
| --- | --- | --- |
| Readable code and explanations | [Readable code](references/readable-code.md) | Before writing or explaining any Rust code. |
| Variables, functions, and test names | [Naming](references/naming.md) | Before naming a value, function, type, module, or test. |
| API docs and comments | [Documentation](references/documentation.md) | API documentation or an explanation of Rust code. Before writing any comment. |
| Functions, structs, enums, and composition | [Design choices](references/design-choices.md) | Functions, structs, composition, or a new custom abstraction. Before adding a struct, enum, builder, or newtype. |
| Standard traits and custom interfaces | [API design](references/api-design.md) | Data models, trait implementations, public APIs, or re-exports. Before adding a trait, derive, or `pub` item. |
| Syntax, borrows, moves, and clones | [Ownership](references/ownership.md) | Borrowing, moves, lifetimes, receiver types, or callbacks. Before adding `.clone()`, a lifetime, or `RefCell`, or fixing a borrow error. |
| Errors, guards, and completion | [Errors and resources](references/errors-resources.md) | Error handling, resource ownership, or cleanup. Before adding a `Result`, `?`, `unwrap`, or a cleanup guard. |
| Formatting, linting, and test commands | [Linting and formatting](references/linting-formatting.md) | Before running checks or finishing any change. |
| Behavior tests, tables, and test locations | [Testing](references/testing.md) | Tests or test structure. Before adding, changing, or reviewing a test. |
| Modules, imports, and public paths | [File organization](references/file-organization.md) | Before adding a file, `mod`, or `pub use`. |
| Dependencies, features, and compiler support | [Cargo](references/cargo.md) | Dependencies, features, workspaces, or supported Rust versions. Before editing `Cargo.toml`. |
| Threads, async, shared state, and performance | [Concurrency](references/concurrency.md) | Async code, tasks, threads, locks, or shared mutable state. Before adding `async`, `spawn`, `Arc`, or `Mutex`, or tuning speed. |
| Raw memory and foreign code | [Unsafe and FFI](references/unsafe.md) | Unsafe code, raw memory, or foreign-function interfaces (FFI). Before writing `unsafe` or `extern`. |

Read the relevant section of a reference, not every file for every change. A
comment typo does not require a concurrency review. The examples explain
particular decisions; do not copy their entire architecture into another project.
Full source files and commands are in [the example package](examples/README.md).

## Before finishing

Check the areas affected by the change: behavior and failure paths; ownership
and cleanup; names and documentation; trait requirements and public compatibility;
test coverage of meaningful outcomes; and supported build configurations. Skip
irrelevant specialized checks, but not unfamiliar ones.
Run the format, lint and test commands in
[Linting and formatting](references/linting-formatting.md).

### Check where each new public item is used

Run this check before finishing a change and when reviewing one. List
each public item the change adds, including fields, variants, methods
and handwritten trait implementations. Next to each, name the code in
this change that uses it. Leave out standard derives such as
`#[derive(Debug)]`, because the
[traits rules](references/api-design.md#traits) and the
[error rules](references/errors-resources.md#errors-and-resource-cleanup) decide those.

- Cut an item that nothing uses, or move it to the change that first
  needs it.
- Count an item as unused when only its own tests use it.
- Keep an unused item when the task asks for that public API, either
  for a caller the task names or for other crates. Name that caller, or
  say "other crates", in the handoff.

The compiler does not warn about unused public items. In a library, its
`dead_code` lint skips any `pub` item another crate can reach. To find
unused items, follow these steps:

1. Temporarily mark the new items and their `pub use` lines
   `pub(crate)`. Rust does not allow `pub(crate)` on a single enum
   variant, so for a new variant mark the whole enum `pub(crate)`.
2. Run `cargo check`. It warns about unused types, variants, fields,
   methods, and custom traits nothing implements or calls. It does not
   warn about an unused implementation of a standard trait such as
   `Display` or `From`, so check those by hand.
3. Undo the visibility change.

Read the results of step 2 this way:

- Plain `cargo check` builds neither `tests/` nor `#[cfg(test)]`
  modules. An item that only tests use gets a warning and counts as
  unused.
- A warning that a type is "more private than" a public function using
  it is expected. The visibility change causes it.
- If `main.rs` or another crate in the workspace fails with an "is
  private" error (E0603 for a type, E0624 for a method, E0616 for a
  field), that code uses the item. Nothing broke.

## Finish

Report what changed, which checks ran, their results, and any remaining limits.
Explain the Rust concepts needed to maintain this change, using the code itself.
A small change may need one sentence; a new ownership model or trait boundary
may need an example and a comparison. Do not append a tutorial to an unrelated fix. For reviews, lead with actionable findings and separate bugs from preferences.

## Supporting references

Use the [example index](references/examples.md) to find a worked example and its
full source. The [example package](examples/README.md) includes unit tests,
integration tests, and doctests; it is not a starter architecture to copy.
Read [sources](references/sources.md) when a decision needs source guidance or the
guides disagree. Read
[skill evaluation](references/evaluation.md) only when testing or revising this
skill. Do not load every reference for every task.
