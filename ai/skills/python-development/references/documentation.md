# Python documentation mechanics

Load and follow the `code-documentation` skill first. It decides whether source
or test prose is useful and how much explanation the code needs. This reference
covers Python docstring format and executable examples only.

## Contents

- [Follow the repository's format](#follow-the-repositorys-format)
- [PEP 257 docstring conventions](#pep-257-docstring-conventions)
- [Use doctest selectively](#use-doctest-selectively)

## Follow the repository's format

Use the established Google, NumPy, Sphinx, plain reStructuredText, or other
docstring style. Do not mix formats in one project or add parameter sections that
repeat an annotated signature. Keep public behavior, important side effects,
failures, resource ownership, and non-obvious limits discoverable where callers
look.

For tests, let names, visible inputs, and assertions explain the behavior. Add a
docstring or comment only when a supported reason, boundary, or regression is
still unclear.

## PEP 257 docstring conventions

Adapted and condensed from [PEP 257 — Docstring Conventions](https://peps.python.org/pep-0257/),
by David Goodger and Guido van Rossum. Reviewed September 30, 2026. PEP 257 has
been placed in the public domain. This is not a verbatim copy.

PEP 257 standardizes docstring semantics and high-level structure, not markup.
Its conventions are recommendations rather than Python syntax requirements, but
documentation tools may rely on them.

### What is a docstring?

A docstring is the first string statement in a module, function, class, or
method; Python exposes it as `__doc__`. Modules should normally have docstrings,
as should exported functions and classes, public methods, and `__init__`. A
package may be documented in its `__init__.py` module docstring.

Tools may also extract, though Python does not assign to `__doc__`, an attribute
docstring immediately following a simple top-level assignment in a module,
class, or `__init__`, and an additional docstring immediately following another
docstring.

Always use triple double quotes. Use a raw triple-double-quoted string when the
docstring contains backslashes. Docstrings are either one-line or multiline.

### One-line docstrings

Use a one-liner only for an obvious case that truly fits on one line. Use triple
quotes, put both delimiters on that line, and add no surrounding blank line.
Write a phrase ending in a period that commands or states the effect, such as
`"""Return the configured path."""`, not “Returns ...”.

Do not repeat an introspectable Python signature. For a C function where
introspection is unavailable, a signature may be appropriate. Mention the
nature of a return value because introspection cannot determine it.

### Multiline docstrings

Start with a one-line summary, then a blank line, then details. Keep the summary
to one line because indexing tools may use it. It may begin on the opening-quote
line or the next line. Indent the entire docstring like its opening quotes.

Insert a blank line after a class docstring, whether one-line or multiline, to
offset it from the first method.

A script docstring should work as its usage message and document the program,
command syntax, environment variables, and files. It may be extensive and
should both guide a new user and summarize every option and argument.

A module docstring should generally list exported classes, exceptions,
functions, and other objects with one-line summaries. A package docstring should
also list exported modules and subpackages.

A function or method docstring should summarize behavior and, when applicable,
document arguments, return values, side effects, raised exceptions, calling
restrictions, optional arguments, and whether keyword arguments are part of the
public interface.

A class docstring should summarize behavior and list public methods and instance
variables. List any subclass interface separately. Document construction in
`__init__`; document each method in its own docstring. If behavior is mostly
inherited, say so and summarize differences. “Override” means replacing a base
method without calling it; “extend” means calling the base method in addition to
new behavior.

Use argument names with their real case, not uppercase Emacs notation. Because
names may be passed as keywords, spell them exactly; listing each argument on a
separate line is best. Unless the whole docstring fits on one line, put closing
quotes on their own line.

### Handling docstring indentation

Processing tools remove indentation from the second and later lines equal to
the minimum indentation of nonblank lines after the first. They remove any
indentation on the first line, preserve later relative indentation, expand tabs,
strip trailing whitespace from lines, and remove leading and trailing blank
lines. Write indentation so this normalization produces the intended text.

## Use doctest selectively

Doctest suits short, deterministic examples where the displayed interaction is
valuable and output remains stable. Keep setup small and show behavior a caller
needs. Avoid volatile timestamps, addresses, unordered representations, or large
error text. Configure comparison flags deliberately rather than assuming every
byte is always exact.

Run doctests through the repository's configured pytest or Sphinx command. Do
not hide a broken example by disabling collection; fix it or clearly mark
non-executable pseudocode outside doctest syntax.
