# Packaging and project configuration

Read this before changing `pyproject.toml`, build backends, package layout,
dependencies, lockfiles, supported versions, entry points, or logging setup.

## Inspect the project first

Read `pyproject.toml`, `requires-python`, classifiers, build-system requirements,
tool configuration, lockfiles, CI matrices, and release commands. Check whether
the repository is an application, library, plugin, or internal tool. Do not
replace an established backend or dependency manager as part of an unrelated
change.

`requires-python` is an installation constraint. Keep syntax, standard-library
APIs, dependencies, and tooling compatible with that range. A local interpreter
or lockfile generated on one version does not prove every supported version.

## Keep metadata and dependencies accurate

Declare runtime dependencies in project dependencies. Put test, lint, type, and
build-only tools in the repository's development groups. Use optional dependency
groups for real optional capabilities, not every import. Review licenses,
maintenance, supported Python versions, native build requirements, and
transitive changes before adding a package.

Use the existing lockfile policy. Generate lockfiles with the project's tool and
review manifest and resolution changes together. Avoid broad updates when a
targeted lock operation solves the task. Do not hand-edit resolved versions or
hashes.

## Choose a layout for a reason

> The src layout helps prevent accidental usage of the in-development copy of the code.

— [PyPA, src layout vs flat layout](https://packaging.python.org/en/latest/discussions/src-layout-vs-flat-layout/)

A source layout is useful for distributable packages because tests are less
likely to import the working-tree package by accident. A flat layout can remain
appropriate for small applications and existing projects. Do not restructure a
repository solely because one layout is preferred elsewhere.

Keep related modules together by responsibility. Avoid one module per class and
catch-all `utils.py` files. Preserve public import paths when moving internals;
use deliberate package re-exports when compatibility requires them.

## Define entry points narrowly

Console-script entry points call a zero-argument callable. Keep argument parsing
and process adaptation at the boundary; put testable behavior in a function that
accepts explicit arguments and returns a status. The
[CLI example](../examples/src/python_skill_examples/cli.py) separates these
roles and has process-level tests.

Do not configure global logging during library import. Applications should own
handlers, destinations, levels, and process-wide setup. Libraries may create
named loggers and emit records without deciding global policy.

## Build and inspect artifacts

When packaging changes, run the configured build and test the wheel or source
distribution in a clean environment. Check that required modules, type markers,
data files, licenses, and entry points are present. Test imports and important
commands against the installed artifact, not only the source tree.

Use `uv build` only when the project uses uv or the task selects it. Other
projects may use `python -m build`, Hatch, Poetry, PDM, or backend-specific
commands. Do not add a build tool solely to follow this reference.
