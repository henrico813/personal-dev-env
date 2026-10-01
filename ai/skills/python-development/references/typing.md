# Typing and checker workflow

Read this before changing annotations, protocols, ABCs, narrowing, generics, or
type-checker configuration.

## Annotate useful boundaries

Use annotations for public interfaces and data flows where they improve use or
maintenance. Follow the repository's current coverage and checker strictness.
Do not annotate every local merely to increase a metric, and do not remove useful
runtime validation because a checker accepted the call.

Use precise standard types and `collections.abc` interfaces where callers need
behavior rather than a concrete container. Keep mutable and read-only
capabilities distinct. Avoid `Any` when a union, protocol, generic parameter, or
checked conversion can express the real values.

When untyped input is unavoidable, contain it at a boundary:

1. accept `object` or the external dynamic value;
2. validate shape and value at runtime;
3. narrow to the application type; and
4. keep `Any` from spreading through callers.

## Narrow rather than assert

Use `isinstance`, `is None`, membership, discriminating fields, user-defined
type guards, or exhaustive branching when those checks reflect runtime facts.
Use `cast` only when the program already establishes the fact in a way the
checker cannot see. A cast performs no runtime check.

Do not add `# type: ignore` broadly. Keep suppressions on the narrow expression,
include the checker code when supported, and record a useful reason when the
constraint is not obvious.

## Choose Protocol or ABC

> Structural subtyping can be seen as a static equivalent of duck typing, which is well known to Python programmers.

— [Typing documentation, Protocols and structural subtyping](https://typing.python.org/en/latest/reference/protocols.html)

Use a `Protocol` when callers need a small shape and implementations should not
inherit from a shared base. Use an ABC when runtime identity, registration,
shared implementation, or framework requirements are part of the design. A
runtime-checkable protocol checks attribute presence, not full signatures or
behavior. Shared behavior tests remain necessary for important substitutes.

## Respect version gates

Check the minimum Python version before using:

- `X | Y` unions or built-in generic syntax in projects supporting older Python;
- `typing.Self` or `asyncio.TaskGroup` before Python 3.11;
- `type` statements and type-parameter syntax before Python 3.12; or
- `typing.TypeIs` before Python 3.13.

Use `typing_extensions` only when the repository already depends on it or the
task justifies adding it. Do not raise `requires-python` solely to shorten an
annotation.

## Run the configured checker

Use the repository's mypy, pyright, basedpyright, or other configured command and
environment. Checker dialects and strictness differ; do not churn annotations to
make an unconfigured checker happy. When none is configured, run:

```sh
uv run --with mypy mypy <changed files>
```

Run inside the project environment so dependencies resolve. Separate errors
introduced by the change from pre-existing failures. Do not add checker config
or dependencies unless tooling setup is part of the task.
