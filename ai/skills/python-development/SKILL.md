---
name: python-development
description: >-
  Use for substantive Python implementation, debugging, review, test design,
  and test work, including unittest, fixtures, mocks, and fakes, plus typing,
  packaging, concurrency, errors, resources, or formatter-only work. Python
  project test design, test review, import patching/mocking, fixture, and fake
  work is in scope even when the request omits Python; if repository inspection
  first reveals Python as the target, load this skill before any implementation,
  design, test, or review decision. Do not use for basic syntax explanations,
  README-only edits, or non-Python tasks.
---

# Python development

Write direct, maintainable Python. Prefer the smallest design that handles the
requirements and preserves the repository's supported versions and public
behavior.

## Contents

| Topic | Section |
| --- | --- |
| Start, scope, and references | [Workflow](#workflow) |
| Style and names | [PEP 8](#1-style-and-names) |
| Functions, classes, and boundaries | [Design](#2-design-choices) |
| Public names and value semantics | [APIs](#3-api-design) |
| Annotations and checkers | [Typing](#4-typing) |
| Exceptions and cleanup | [Errors](#5-errors-and-resources) |
| Tasks, threads, and processes | [Concurrency](#6-concurrency) |
| pytest mechanics | [Tests](#7-testing) |
| Docstrings and doctest | [Documentation](#8-documentation) |
| Projects, builds, and entry points | [Packaging](#9-packaging) |
| Formatting, linting, and checks | [Tools](#10-tool-checks) |
| Final review | [Before finishing](#before-finishing), [Finish](#finish) |

## Workflow

Read repository instructions, relevant code and tests, `pyproject.toml`, lock
files, supported Python versions, CI, and configured checks. Identify the
behavior that must change and the behavior that must remain. Do not edit files
for a review-only request.

Repository-configured tools and local conventions win. PEP 8 is the primary
style guide when the repository does not decide. Official Python and pytest
documentation settle behavior. Do not raise the minimum Python version, change
test frameworks, add dependencies, or introduce tool configuration unless the
task requires it.

Make the smallest complete change. Do not add a class, protocol, wrapper,
registry, plugin system, async layer, or dependency-injection framework for
hypothetical reuse. Add a boundary when it owns a real rule, resource,
variation, or external effect.

Required reference procedure:

1. Classify the task from the request and directly referenced context.
2. When Python is already clear, load this skill before repository research.
3. If inspection first reveals Python as the target, stop and load this skill
   before making a substantive implementation, design, test, or review decision.
4. Match every applicable row below and open every linked Python reference with
   the file-reading tool. Loading this `SKILL.md` or shared testing or
   documentation guidance does not count as reading a listed Python reference.
5. Decide or edit only after all matched references are open.

| Task touches | Read first |
| --- | --- |
| Formatting, naming, imports, or general style | [PEP 8 guide](references/pep8-guide.md) |
| Functions, classes, composition, data models, or boundaries | [Design choices](references/design-choices.md) |
| API wrappers, validation, or missing values | [Design choices](references/design-choices.md) + [API design](references/api-design.md) |
| Public names, exports, value semantics, compatibility, or deprecation | [API design](references/api-design.md) |
| Annotations, narrowing, protocols, ABCs, or checker output | [Typing](references/typing.md) |
| Exceptions, context managers, setup, completion, or cleanup | [Errors and resources](references/errors-resources.md) |
| Async code, tasks, queues, threads, processes, or shared state | [Concurrency](references/concurrency.md) |
| Async task ownership or large work sets | [Concurrency](references/concurrency.md) + [Design choices](references/design-choices.md) |
| Test design, fixtures, import patching/mocking, or fakes | [Testing](references/testing.md); setup/cleanup or partial acquisition also requires [Errors and resources](references/errors-resources.md) |
| Supported Python versions plus test runner or CI | [Testing](references/testing.md) + [Typing](references/typing.md) + [Packaging](references/packaging.md) |
| Docstrings, comments, API prose, or executable examples | [Documentation](references/documentation.md) |
| Exception handling plus docstrings | [Errors and resources](references/errors-resources.md) + [Documentation](references/documentation.md) |
| Project metadata, layouts, dependencies, lockfiles, builds, or commands | [Packaging](references/packaging.md) |

The [example index](references/examples.md) links decisions to complete source.
During ordinary Python work, read `examples/` only as reference; do not run or
copy it as a starter layout. Execute it only when installing, revising, or
validating this skill.

## 1. Style and names

Follow surrounding style unless it creates a correctness problem. For new code
without local guidance, use the condensed [PEP 8 guide](references/pep8-guide.md).
Keep changes focused; do not reformat unrelated existing code.

Use names that state purpose and units. Prefer ordinary control flow and useful
intermediate values over nested expressions or clever machinery. Avoid mutable
defaults and hidden process-wide state. Keep imports free of network,
filesystem, and other external work.

## 2. Design choices

Use a function for a direct operation or transformation. Use a class when state,
identity, invariants, or resource lifetime need one owner. Use a dataclass for a
value whose fields and generated methods have the intended semantics. Use an
enum for mutually exclusive states rather than conflicting flags.

Separate decisions from external effects when that keeps rules testable and
failure handling clear. Prefer composition when independent behaviors vary.
Use inheritance for a real subtype or an established framework extension, not
as a default reuse mechanism. Read [design choices](references/design-choices.md)
before adding an abstraction.

## 3. API design

Treat documented names and deliberate exports as public. Use `__all__` when an
explicit export list helps; do not rely on file placement to hide a public path.
Preserve names, signatures, import paths, errors, and observable behavior when
compatibility requires it. Deprecate with a usable replacement and migration
period rather than silently changing behavior.

Equality, hashing, and ordering must describe the same identity. Review
dataclass-generated methods and handwritten dunders together. Keep `repr`
useful without exposing secrets. See [API design](references/api-design.md).

## 4. Typing

Annotate supported interfaces and difficult data flow where types improve use or
maintenance. Avoid `Any` when a precise type is practical; contain unavoidable
dynamic values at a checked boundary. Narrow with real runtime checks instead of
casts that only silence the checker.

Use a `Protocol` for structural behavior accepted from independent types. Use an
ABC when shared runtime identity, registration, or implementation belongs in the
design. Types do not prove equivalent behavior. Respect version-gated syntax and
the repository's checker. See [typing](references/typing.md).

## 5. Errors and resources

Catch only where the code can recover, add useful context, or translate an
implementation failure into a caller-facing error. Preserve causes with
`raise ... from ...`. Keep `try` suites narrow and do not convert unrelated
failures into empty values.

Use context managers or `try/finally` for local cleanup. Use `ExitStack` for a
dynamic number of staged acquisitions. Make fallible completion such as flush,
close, commit, or shutdown explicit when callers need its result. Read
[errors and resources](references/errors-resources.md).

## 6. Concurrency

Start sequentially. For async work, do not call blocking I/O directly on the
event loop. Own every task, observe its result, and define cancellation,
timeouts, partial effects, and shutdown. A task group manages child lifetime and
failure; it does not limit how many tasks a producer creates. Bound accepted and
queued work separately from active I/O.

Choose threads, processes, or async from the actual runtime and workload. Do not
state a universal GIL rule. Measure before claiming a speedup. Read
[concurrency](references/concurrency.md).

## 7. Testing

Before designing, writing, or reviewing Python tests, fixtures, mocks, or fakes,
read [Testing](references/testing.md), including when choosing between pytest
and unittest.

When the repository runs pytest, write new tests in pytest style and rewrite a
unittest-style test to pytest when the task modifies it. Leave untouched
`unittest.TestCase` tests in place for pytest to collect. Do not turn the task
into a suite migration.

When the repository does not run pytest, do not convert tests, add pytest, or
change CI. `python -m unittest` silently skips pytest-style tests, so write and
edit tests in the existing unittest style and note this in the handoff. See
[Choose the test style](references/testing.md#choose-the-test-style).

Load and follow `behavior-focused-testing` when planning, writing, updating, or
reviewing tests. Its seven-word maximum for test function names is required.
Use `@pytest.mark.parametrize`, fixtures, and
`pytest.param(..., id="descriptive-case")` where their mechanics fit. Read
[testing](references/testing.md) for pytest-specific organization and cleanup.

## 8. Documentation

Load and follow `code-documentation` when planning, writing, changing, or
reviewing source code or tests. Add prose only for supported facts that code,
types, names, and tests do not already make clear. This skill adds Python
docstring and doctest mechanics in [documentation](references/documentation.md).

Use the repository's docstring format. Do not rewrite correct documentation
merely to make it longer or change styles. Keep examples executable when users
depend on them and volatile output will not make them brittle.

## 9. Packaging

Inspect `pyproject.toml`, supported versions, build backend, dependency groups,
lockfiles, package layout, and entry points before changing packaging. Keep
runtime and development dependencies separate. Review generated lock changes;
do not hand-edit them.

Test the built or installed artifact when packaging changes. A `src/` layout can
help prevent accidental imports from the working tree, but do not restructure a
project solely for style. See [packaging](references/packaging.md).

## 10. Tool checks

Use repository commands first. Never add tool configuration or dependencies
unless tooling setup is the task. When the repository configures none:

```sh
uvx ruff format --line-length 79 <new files>
uvx ruff check --line-length 79 --select E,W,F,I,N <changed files>
uv run --with mypy mypy <changed files>
uv run pytest  # only when the repository runs pytest
```

For changed files in an unconfigured repository, fix Ruff findings on touched
lines only and report the rest; never reformat whole existing files. Run mypy in
the project environment so imports resolve, and separate new errors from existing
ones. Use `python -m pytest` when the project does not use uv. Use pyright or
basedpyright only when configured by the repository.

If `uv` or `uvx` is unavailable, use the repository's existing environment and
configured commands, and run `python -m pytest` when possible. Report checks
that cannot run. Never install tools globally without permission.

## Before finishing

Check affected behavior and failure paths, supported Python versions, public
compatibility, resource and task ownership, tests, documentation, packaging,
and configured tools. Inspect the final diff for unrelated formatting, generated
artifacts, caches, and accidental dependency or lockfile changes. Fix failures
caused by the change, rerun the affected checks, and repeat until they pass or
report the remaining blocker.

## Finish

Report what changed, the checks that ran and their results, pre-existing or
environment failures, and remaining limits. For reviews, lead with actionable
findings and distinguish defects from preferences. Do not claim checks or
compatibility that were not verified.

## Supporting references

The [skill README](README.md) contains compatibility, tool, source, licensing,
and maintenance notes. Use [sources](references/sources.md) for attribution.
Use [evaluation](references/evaluation.md) only when installing or revising
this skill. The [example package](examples/README.md) is reference-only except
during installation, revision, or validation.
