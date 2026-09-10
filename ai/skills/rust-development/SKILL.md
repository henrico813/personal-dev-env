---
name: rust-development
description: >-
  Writes, reviews, refactors, and tests maintainable Rust code. Use for Rust
  implementation, debugging, API design, code review, documentation, tests,
  modules, Cargo configuration, dependencies, and features. Covers standard
  Rust practices with explanations suitable for a junior maintainer, including
  ownership, traits, errors, resource cleanup, concurrency, and unsafe code.
  Does not apply to unrelated languages or tasks.
---

# Rust development

Write straightforward, idiomatic Rust. Prioritize simplicity, maintainability,
and testability. The maintainer is learning Rust.
Explain unfamiliar features and the decisions behind them. Keep important Rust
practices even when they need explanation. The result should be code the
maintainer can understand, test, and change.

## Contents

| Topic | Section |
| --- | --- |
| Starting and finishing a task | [Workflow](#workflow), [checks](#before-finishing), [handoff](#finish) |
| Readable code and explanations | [Writing for the maintainer](#1-code-a-junior-engineer-can-understand) |
| Variables, functions, and test names | [Naming](#2-naming) |
| API docs and comments | [Documentation](#3-documentation) |
| Functions, structs, enums, and composition | [Data and behavior](#4-objects-structs-and-enums) |
| Standard traits and custom interfaces | [Traits](#5-traits) |
| Syntax, borrows, moves, and clones | [Syntax and ownership](#6-syntax-and-ownership) |
| Errors, guards, and completion | [Errors and cleanup](#7-errors-and-resource-cleanup) |
| Formatting, linting, and test commands | [Tool checks](#8-linting-and-formatting) |
| Behavior tests, tables, and test locations | [Tests](#9-test-organization) |
| Modules, imports, and public paths | [File organization](#10-file-organization) |
| Dependencies, features, and compiler support | [Cargo](#11-cargo-and-dependencies) |
| Threads, async, shared state, and performance | [Concurrency](#12-shared-state-async-and-performance) |
| Raw memory and foreign code | [Unsafe and FFI](#13-unsafe-code-and-ffi) |

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

Before changing or reviewing any area below, read its matching reference. Apply
the relevant checks, not every technique. These local references are required
for the listed tasks; fetching every external guide is not.

| Task touches | Read first |
| --- | --- |
| Functions, structs, composition, or a new custom abstraction | [Design choices](references/design-choices.md) |
| Borrowing, moves, lifetimes, receiver types, or callbacks | [Ownership](references/ownership.md) |
| Data models, trait implementations, public APIs, or re-exports | [API design](references/api-design.md) |
| Tests or test structure | [Testing](references/testing.md) |
| API documentation or an explanation of Rust code | [Documentation](references/documentation.md) |
| Error handling, resource ownership, or cleanup | [Errors and resources](references/errors-resources.md) |
| Dependencies, features, workspaces, or supported Rust versions | [Cargo](references/cargo.md) |
| Async code, tasks, threads, locks, or shared mutable state | [Concurrency](references/concurrency.md) |
| Unsafe code, raw memory, or foreign-function interfaces (FFI) | [Unsafe and FFI](references/unsafe.md) |

Read the relevant section of a reference, not every file for every change. A
comment typo does not require a concurrency review. The examples explain
particular decisions; do not copy their entire architecture into another project.
Full source files and commands are in [the example package](examples/README.md).

## 1. Code a junior engineer can understand

Prefer clear steps, descriptive intermediate values, and ordinary control flow.
Use small functions with a clear purpose, but do not split a readable operation
into helpers that force the reader to jump between files. Simple iterator chains
are welcome; use a loop when it makes branching or state changes easier to follow.

When introducing an unfamiliar Rust concept, briefly explain what it does and
why this code needs it. Pair advice against unnecessary complexity with the
conditions that would justify it. For example, a custom trait needs a reason
such as interchangeable behavior or an extension point. It does not need a rule
that every struct must have a matching trait. Describe ownership in terms of who
keeps a value and who borrows it. Put lasting constraints in the code's
documentation; put introductory syntax explanations in the handoff, not comments on every line. Do not avoid
useful Rust idioms merely because they are unfamiliar.

Explain the requirement before the technique. Show a small example, explain why
it fits, and name a useful alternative when the requirement could differ. For
instance, show a concrete function before explaining when a trait helps. Do not
call an alternative “bad” when it is simply unnecessary for this task. Keep
explanations near the code they explain, rather than collecting unexplained terms
in a glossary at the end. Avoid filler such as “leverage,” “robust solution,” and
“seamless integration.” Say what the code does and what it guarantees.

## 2. Naming

Use `snake_case` for variables, fields, functions, methods, and modules;
`UpperCamelCase` for structs, enums, variants, and traits; and
`SCREAMING_SNAKE_CASE` for constants and statics. Treat acronyms like words:
`HttpClient`, not `HTTPClient`. Keep abbreviations conventional and unambiguous.

Name values for their meaning: `retry_count`, `output_path`, `request_timeout`.
Avoid vague names such as `data`, `temp`, or `manager` when a more specific name
exists. Short conventional names are fine in small, obvious scopes.

For example, `timeout` is ambiguous when its value is an integer. Prefer
`timeout_ms` when the representation really is milliseconds, or a `Duration`
when the code can carry the unit in its type. Do not put the type into a name
such as `report_string` unless the distinction from another representation matters.

Name actions with verbs, such as `parse_config` and `write_report`. Name boolean
queries clearly, such as `is_empty`. Ordinary getters usually omit `get_`:
`report.title()`. Follow Rust's conversion conventions: `as_` for cheap views,
`to_` for conversions that do work, and `into_` for ownership-consuming conversions.

For test names and behavioral test design, also load and follow the
`behavior-focused-testing` skill. Its seven-word maximum is required.

## 3. Documentation

Use `//!` to explain a crate or module and `///` for documented items. Document
public APIs and non-obvious internal requirements. Start with a short sentence
saying what the item does. Explain required inputs, units, side effects, and
important guarantees where they matter.

Include a small usage example for APIs whose use is not obvious. Add `# Errors`,
`# Panics`, and `# Safety` sections when applicable; do not add empty sections.
Keep examples compilable and use documentation tests where practical. Explain
parameters in prose rather than repeating their types in a parameter table.
Link related Rust items with rustdoc links such as ``[`Report`]``. Use `no_run`
only when an example should compile but not execute; use `compile_fail` for an
intentional compiler rejection. Do not hide broken examples with `ignore`.

Comments should explain constraints, intent, or surprising decisions, not
restate the next line. Update documentation when behavior changes. Do not add
change diaries, self-reviews, or claims about following this skill to project docs.

## 4. Objects, structs, and enums

Use structs for related data and `impl` blocks for associated behavior. Rust does
not use class inheritance: compose types by storing one inside another and
calling its methods. Use a free function when there is no meaningful state or
type-specific behavior to attach it to.

Use enums for distinct alternatives instead of conflicting boolean flags. Give
fields the narrowest visibility needed. Keep fields private when callers must
not bypass validation or when representation should remain changeable. Public
fields can suit deliberately exposed, unconstrained data.

Use constructors to establish required guarantees. Add `Default` only when a
sensible default exists. Derive standard traits when their meaning fits the type;
do not add traits, getters, setters, builders, or wrappers automatically.
A constructor with three clear required values can be easier to use than a
builder. A long sequence of boolean arguments can be harder to read than a named
configuration struct. Choose from the caller's needs, not a fixed pattern.
Use a builder when numerous optional settings make construction hard to read.

Use a newtype, a struct wrapping another value, when it prevents mixing IDs,
units, or validated and unvalidated data. A type alias only adds a name; it does
not create a distinct type. Prefer existing meaningful types such as `Duration`
over inventing another wrapper for the same purpose. Keep validation true across
constructors, conversions, setters, and deserialization, not just at creation.

## 5. Traits

A trait describes behavior that implementing types provide; it does not store
instance fields. Reuse standard traits such as `Read`, `Write`, `Display`, and
`From` when they express the intended conversion semantics.

Introduce a custom trait for useful interchangeable behavior, an extension point,
or a dependency that genuinely needs substitution. Do not create one for every
struct or solely to satisfy a mocking framework. Keep traits small and coherent.

Use a concrete type when no abstraction is needed. Use `impl Trait` or a named
generic parameter for compile-time selection; use `dyn Trait` when runtime
selection is useful and the trait is dyn-compatible (usable as a trait object).
Return-position `impl Trait` hides one concrete return type, not arbitrary
unrelated types selected at runtime. A borrowed `&dyn Trait` does not
require a heap allocation. Add only the bounds the operation needs. Use an
associated type for a type chosen by the implementation; use a trait parameter
when implementations need to vary with that parameter. Use `Fn`, `FnMut`, or
`FnOnce` for callbacks according to how they access their captures.

The [summary example](references/design-choices.md#add-a-trait-when-a-caller-needs-shared-behavior)
starts with a report, adds a trait when several types need summary output, and
uses borrowed trait objects when the caller has different item types in one list.
These are different requirements, not a prescribed path toward more abstraction.

Derive standard traits when their generated behavior is correct. `Copy` permits
implicit duplication; `Clone` makes duplication explicit but may still share an
underlying resource. `Debug` is for diagnostics and `Display` is for people.
Equality, hashing, and ordering must agree. Use `From` for infallible conversions
and `TryFrom` when validation or conversion can fail. Read the API checklist for
these requirements before implementing them manually.

## 6. Syntax and ownership

Use the project's Rust edition and supported syntax. Prefer `match`, `if let`,
`let ... else`, and early returns where they clarify the flow. Let inference
handle obvious types; add annotations when they explain intent or resolve
ambiguity. Prefer named steps over deeply nested expressions and custom macros.

Borrow with `&T` to share access without taking ownership; use `&mut T` for
exclusive mutable access. Borrowing lets the original owner keep the value.
A move transfers ownership; it is not a copy of the owned contents. For methods, choose `&self`, `&mut self`, or `self`
according to whether the method borrows, mutates, or takes ownership. Prefer
`&str`, `&[T]`, and `&Path` for inputs that only need borrowed views.

Take ownership when the value must be retained or consumed. Keep ownership
simple before adding explicit lifetimes, `Rc`, `Arc`, or locks. Cloning is valid
when an independent value makes the design clearer and the cost is acceptable;
do not clone blindly to silence the borrow checker. Lifetime annotations describe
relationships between borrows; they do not extend a value's life.
Use `Cell` or `RefCell` only when mutation through shared access serves a real
need; they do not provide thread synchronization. Check the concurrency reference
when adding them. Prefer checked numeric conversions when truncation is invalid.

## 7. Errors and resource cleanup

Use `Option` for absence and `Result` for recoverable failure. Use `?` to propagate
errors when appropriate. Preserve useful error context. Choose typed errors when
callers need to distinguish failures; application-level error wrappers can suit
errors that are mainly reported. Follow the project's existing error approach.
Custom error types should implement `Debug`, `Display`, and `Error`; retain
underlying errors through `source()` where appropriate instead of flattening
all failures into strings. Add actionable context without exposing secrets.

A parse failure should remain distinguishable from a read failure when the
caller handles them differently. A function with one useful existing error type
may simply return it. The [error example](references/errors-resources.md#example-keep-the-underlying-error)
shows when another type becomes useful and what `map_err`, `?`, and `source()` do.

Do not use panics for expected bad input or ordinary I/O failures. Use `expect`
only for a condition the program already guarantees, or for test setup. Its
message should explain the guarantee. Do not silently discard a fallible result.

Use owned resources and guards so normal scope exit, early returns, and `?` clean
up automatically. This is resource acquisition is initialization (RAII): resource
lifetime follows its owner. Fields already clean themselves up; add `Drop` only
for cleanup the owned fields do not provide. Keep destructors non-panicking.
Use an explicit `flush`, `finish`, `close`, or `shutdown` operation when completion
can fail or needs async work. `Drop` cannot return a `Result`, and it is not a
guarantee of successful persistence or cleanup after a process abort.

## 8. Linting and formatting

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

## 9. Test organization

Load and follow `behavior-focused-testing` when writing, updating, or reviewing
tests. This section adds Rust-specific organization and tooling guidance.

Use `#[cfg(test)] mod tests` beside a module's implementation for local tests.
Use `tests/` for integration tests that exercise the library as a consumer.
Place genuinely shared integration-test helpers in `tests/common/mod.rs` and
import them with `mod common;`. Use doctests for API examples. For a CLI, include
process-level tests for important arguments, output, and exit behavior.

Use small table-driven tests when cases share the same setup and assertion.
Identify the failing case in assertion messages. Split tables when cases require
conditional assertions or different setup. Borrow this principle from Uber's Go
guide, not Go's syntax or testing APIs.

Test newtype validation and custom error behavior through supported entry points.
For handwritten equality, hashing, or ordering, test the promised relationships.
Test cleanup and partial failures when they affect observable behavior. Use property
checks or fuzzing when input combinations exceed useful hand-written cases;
do not add tests that merely repeat a standard derive implementation.

## 10. File organization

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
requires them; consult the API checklist before changing public paths or bounds.

## 11. Cargo and dependencies

Reuse the standard library and existing dependencies when they fit. Add a crate
when it avoids substantial fragile code, not for a trivial helper. Check its
maintenance, license, security advisories, supported compiler, features, and
platform requirements. Keep test-only dependencies in `dev-dependencies`.

Respect the project's minimum supported Rust version (MSRV), edition, feature
resolver, and lockfile policy. Use workspace configuration for genuinely shared
settings. Design features to add capabilities rather than disable others; document
supported combinations and check optional dependencies with defaults disabled.
Read the Cargo checklist before changing these areas. Do not claim that a host
build verifies other targets or that one feature selection covers every build.

## 12. Shared state, async, and performance

Use `Rc` for shared ownership within a thread and `Arc` when ownership must be
shared across threads and the contained type supports it. Neither alone makes
arbitrary inner data safe to mutate concurrently. Understand `Send` and `Sync`
before changing cross-thread APIs, and prefer inferred implementations.

Keep lock scopes short. Avoid holding a blocking lock guard across `.await`;
use an appropriate async lock only when the operation genuinely needs it.
Do not run blocking I/O or prolonged CPU work on an async executor's worker
without the runtime's intended mechanism. Give tasks an owner, bounded resource
use, and explicit completion, cancellation, error, and shutdown behavior.
Read the concurrency checklist when this code is required; do not introduce it
into a sequential task without a reason.

Measure before adding custom allocators, intricate lifetime arrangements, or
other performance-driven complexity. Record what the measurement demonstrates.
Prefer an established safe implementation when it meets the requirement.

## 13. Unsafe code and FFI

Prefer safe Rust. When unsafe code or FFI is necessary, read the unsafe checklist
before editing. Keep unsafe operations narrow and explain the concrete safety
conditions next to them. A safe public API must uphold those conditions itself;
a comment cannot transfer memory-safety obligations to a safe caller.

Check pointer validity, lifetimes, aliasing, initialization, ownership, cleanup,
and foreign calling conventions as applicable. Never add an unchecked cast,
`transmute`, or `unsafe impl Send/Sync` merely to suppress a compiler error.
Run relevant tests and Miri where supported. Passing tests is evidence, not a
proof that every permitted call is safe; report unsupported validation clearly.

## Before finishing

Check the areas affected by the change: behavior and failure paths; ownership
and cleanup; names and documentation; trait requirements and public compatibility;
test coverage of meaningful outcomes; and supported build configurations. Skip
irrelevant specialized checks, but not unfamiliar ones.

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
