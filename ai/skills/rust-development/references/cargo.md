# Cargo, dependencies, and builds

Read this before changing dependencies, features, workspaces, build configuration,
or supported Rust versions.

## Contents

- [Inspect the existing build](#inspect-the-existing-build)
- [Add dependencies for a concrete benefit](#add-dependencies-for-a-concrete-benefit)
- [Keep features additive](#keep-features-additive)
- [Use workspaces without hiding configuration](#use-workspaces-without-hiding-configuration)
- [Verify the supported builds](#verify-the-supported-builds)

## Inspect the existing build

Check `Cargo.toml`, `Cargo.lock`, toolchain files, `.cargo/config.toml`, CI, and any
`build.rs`. Identify the minimum supported Rust version (MSRV), edition, target
platforms, feature resolver, default members, and required system libraries.
Do not silently raise the MSRV or change the edition because a newer example
uses newer syntax. A pinned development toolchain and the minimum supported
compiler are different promises; test both when the project promises both.

Reference: [Cargo: Rust version](https://doc.rust-lang.org/cargo/reference/rust-version.html).

## Add dependencies for a concrete benefit

A dependency removes code you would otherwise maintain, but adds code and
compatibility requirements maintained elsewhere. Compare those costs. Reusing a
well-established parser is different from adding a crate for a three-line
helper. “No dependencies” and “use a crate for everything” are both poor defaults.


Prefer the standard library or a dependency already in use when it fits. A
well-maintained crate can be simpler and safer than hand-written parsing,
cryptography, or platform code; a dependency for a trivial helper may not be.
Before adding one, check its relevant API, maintenance, license, security advisories,
transitive dependencies, enabled features, compiler support, and target support.
Use current primary sources for these changing facts. Do not declare a dependency
secure merely because no advisory was found.

Put test-only dependencies in `dev-dependencies` and build-script dependencies
in `build-dependencies`. Make a dependency optional only when a useful optional
capability needs it. Review the manifest and lockfile diff together. Avoid broad
`cargo update` runs when a targeted change solves the task. Do not hand-edit
resolved checksums or versions in the lockfile.

References: [Cargo: specifying dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
and [Cargo.toml versus Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html).

## Keep features additive

Features should add capabilities without removing existing behavior or breaking
the base API. Cargo can combine feature requests from multiple dependents;
`default-features = false` on one dependency edge does not guarantee defaults
stay off when another edge enables them. Avoid mutually exclusive features when
possible. For optional standard-library support, prefer an additive `std` feature
over a feature that disables it.

Document the defaults, optional dependencies, and supported combinations. Use
runtime configuration for runtime choices rather than a web of build flags.
Distinguish local `#[cfg(test)]` code from a deliberately exported testing feature.
Use `cargo tree -e features` to inspect unexpected feature activation.

Reference: [Cargo: features](https://doc.rust-lang.org/cargo/reference/features.html).

### Example: build choices versus runtime choices

Suppose users can turn diagnostic output on and off without rebuilding. That
belongs in a command-line option or configuration value. A Cargo feature would
require a different binary and complicate the supported build combinations.

A feature can make sense when a capability adds an optional dependency or target
requirement. Test the smaller build too: testing only with every feature enabled
can hide an accidental dependency on code that should be optional.

For an existing feature named `json`, these commands select different builds;
they are examples, not commands for the dependency-free teaching package:

```sh
cargo test --workspace
cargo test --workspace --no-default-features
cargo test --workspace --no-default-features --features json
```

Only run combinations the package promises to support. A passing default build
says nothing by itself about the feature-disabled build. Cargo may also combine
requests from several dependents, so features must coexist where possible.

## Use workspaces without hiding configuration

A package is described by a `Cargo.toml`. It can contain a library crate and one
or more binary crates. A workspace groups packages that share coordination such
as a lockfile and target directory. A module is an organization unit inside a
crate; it does not need its own manifest.

Split a module before splitting a package when organization is the only problem.
A separate package becomes useful when code needs independent reuse, different
dependencies, or distinct build requirements. The teaching package intentionally
keeps its small examples together rather than making a workspace member for
every concept.

Use shared workspace dependencies, package settings, and lints when member crates
truly share them. Members must opt into the appropriate inherited settings; a
root declaration alone does not apply every setting everywhere. Keep resolver
and root-only configuration in their proper location. Check selected members,
profiles, and feature combinations instead of assuming a command at the root
builds every relevant configuration. Do not split crates without a meaningful
boundary such as independent reuse, dependencies, or build requirements.

Reference: [Cargo: workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html).

## Verify the supported builds

Run the repository's normal checks, then select relevant configurations:

| Change | Additional check |
| --- | --- |
| Optional dependency or feature | Test the default build and the supported no-default and explicit-feature configurations. |
| Compiler or syntax requirements | Build with the advertised minimum compiler, not only the developer's compiler. |
| Platform-specific code | Cross-check the target where possible, and run platform or hardware tests where required. Compilation alone does not prove runtime behavior. |
| Published library API | Build consumer examples and docs; check public API and feature compatibility. |
| Dependency update | Review the resolved changes and run the repository's dependency and security checks. |

Use `--all-features` only for compatible combinations. It does not replace testing
minimal features. Use `--locked` in checks that must use an existing committed
lockfile; a failure may mean an intentional manifest change needs a reviewed
lockfile update. Respect the repository's lockfile policy; do not assert that
library repositories must never commit one. Published libraries are also used
with consumers' dependency resolutions, so a local locked build is not the
entire compatibility check.

References: [Cargo: features](https://doc.rust-lang.org/cargo/reference/features.html),
[lockfiles](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html), and
[SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html).
