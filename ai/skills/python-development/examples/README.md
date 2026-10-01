# Python examples

This Python 3.12 package contains the worked examples used by the skill. Its
runtime code uses only the standard library. Keep using your project's own
supported versions, dependency choices, and layout in production; this package
does not prescribe them.

## Read an example

Start with the [example index](../references/examples.md). It links each design
question to its explanation and full source. Modules are independent teaching
examples rather than pieces of a starter application. The `worker-count`
command uses only the bounded-count parser.

## Check the code

From this directory, run:

```sh
uv run --locked ruff format --check
uv run --locked ruff check
uv run --locked mypy
uv run --locked pytest
```

The pytest configuration includes doctests from `src`, so the parser example
runs with the rest of the suite. The package includes `uv.lock`; `--locked`
prevents checks from changing dependency resolution.

Run the command-line example with:

```sh
uv run --locked python -m python_skill_examples.cli " 3 "
```

It prints `3` and succeeds. Missing or invalid input returns status 2 and
writes an error to stderr. The process-level tests cover this behavior.

## Validation status

`uv lock` generated the lockfile, and `uv build` produced both the source and
wheel distributions. The four check commands above, the command-line example,
and `uv build` passed on system CPython 3.12.3 and uv-managed CPython 3.13.1.
Agent activation and the proposed evaluation cases were not run.
