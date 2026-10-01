# Python Development Skill

This bundle provides guidance for planning, implementing, refactoring, testing,
and reviewing Python code. `SKILL.md` defines when and how agents use it.

## Python compatibility

Check `requires-python`, classifiers, CI matrices, runtime deployment, and tool
configuration before using version-sensitive syntax or APIs. A newer local
interpreter does not permit raising the project's minimum Python version.

### Python 3.10

Python 3.10 added `X | Y` union annotations and structural pattern matching.
Use them only when the project supports 3.10 or newer; do not replace clear
branching with pattern matching merely because it is available. See
[What's New in Python 3.10](https://docs.python.org/3/whatsnew/3.10.html).

### Python 3.11

Python 3.11 added `asyncio.TaskGroup`, `ExceptionGroup`, and `typing.Self`.
Keep older-runtime support in mind before using them, and do not treat a task
group as a work-capacity limit. See
[What's New in Python 3.11](https://docs.python.org/3/whatsnew/3.11.html).

### Python 3.12

Python 3.12 added `type` statements and type-parameter syntax such as
`class Box[T]`. Keep older syntax when the supported range requires it. See
[What's New in Python 3.12](https://docs.python.org/3/whatsnew/3.12.html).

### Python 3.13

Python 3.13 added `typing.TypeIs` and made free-threaded CPython builds
available experimentally. Free-threaded builds do not justify assumptions about
every deployed runtime or extension. See
[What's New in Python 3.13](https://docs.python.org/3/whatsnew/3.13.html).

## Tool notes

Follow repository configuration first. When none exists, `SKILL.md` provides
narrow Ruff, mypy, and pytest commands. Ruff handles formatting plus selected
style and correctness checks. Run mypy in the project environment so installed
dependencies and local packages resolve.

`ty` remains a 0.0.x project; it was 0.0.82 on September 17, 2026. Revisit it
after a stable release rather than making it this skill's fallback checker.
Use pyright or basedpyright only when the repository configures it.

## Sources and licensing

Sources and retained claims were reviewed September 30, 2026. PEP 8 is the
primary style guide. Official Python and pytest documentation settle behavior;
repository conventions settle style. See [source notes](references/sources.md)
for each supporting source's role and quotation terms.

The [PEP 8 reference](references/pep8-guide.md) is a condensed adaptation of
[PEP 8](https://peps.python.org/pep-0008/). The docstring section in
[documentation](references/documentation.md) condenses
[PEP 257](https://peps.python.org/pep-0257/). Both PEPs state that they have
been placed in the public domain. The remaining source excerpts are short,
unchanged, attributed quotations adjacent to independent guidance.

The worked examples were written for this skill. They are not presented as
source examples copied from the linked guides.

## Maintenance

Update the PEP 8 adaptation section by section and retain every source heading,
qualification, and exception. Recheck compatibility notes, source wording,
links, and licensing separately. Keep `SKILL.md` concise by moving detailed
mechanics into directly linked references. Run the link checker, example checks,
build, installer tests, and evaluation cases after material revisions.
