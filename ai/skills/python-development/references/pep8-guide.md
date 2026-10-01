# PEP 8 — condensed reference

Source: [PEP 8 — Style Guide for Python Code](https://peps.python.org/pep-0008/).
Reviewed September 30, 2026. PEP 8 was written by Guido van Rossum, Barry
Warsaw, and Alyssa Coghlan, and has been placed in the public domain.

This is a condensed adaptation, not a verbatim copy. It retains every source
section and subsection and their rules, preferences, qualifications, and
exceptions while shortening examples and repeated explanation. Consult the
source for its full examples, rationale, footnotes, and historical references.

## Contents

- [Introduction](#introduction)
- [A Foolish Consistency is the Hobgoblin of Little Minds](#a-foolish-consistency-is-the-hobgoblin-of-little-minds)
- [Code Lay-out](#code-lay-out)
  - [Indentation](#indentation)
  - [Tabs or Spaces?](#tabs-or-spaces)
  - [Maximum Line Length](#maximum-line-length)
  - [Should a Line Break Before or After a Binary Operator?](#should-a-line-break-before-or-after-a-binary-operator)
  - [Blank Lines](#blank-lines)
  - [Source File Encoding](#source-file-encoding)
  - [Imports](#imports)
  - [Module Level Dunder Names](#module-level-dunder-names)
- [String Quotes](#string-quotes)
- [Whitespace in Expressions and Statements](#whitespace-in-expressions-and-statements)
  - [Pet Peeves](#pet-peeves)
  - [Other Recommendations](#other-recommendations)
- [When to Use Trailing Commas](#when-to-use-trailing-commas)
- [Comments](#comments)
  - [Block Comments](#block-comments)
  - [Inline Comments](#inline-comments)
  - [Documentation Strings](#documentation-strings)
- [Naming Conventions](#naming-conventions)
  - [Overriding Principle](#overriding-principle)
  - [Descriptive: Naming Styles](#descriptive-naming-styles)
  - [Prescriptive: Naming Conventions](#prescriptive-naming-conventions)
    - [Names to Avoid](#names-to-avoid)
    - [ASCII Compatibility](#ascii-compatibility)
    - [Package and Module Names](#package-and-module-names)
    - [Class Names](#class-names)
    - [Type Variable Names](#type-variable-names)
    - [Exception Names](#exception-names)
    - [Global Variable Names](#global-variable-names)
    - [Function and Variable Names](#function-and-variable-names)
    - [Function and Method Arguments](#function-and-method-arguments)
    - [Method Names and Instance Variables](#method-names-and-instance-variables)
    - [Constants](#constants)
    - [Designing for Inheritance](#designing-for-inheritance)
  - [Public and Internal Interfaces](#public-and-internal-interfaces)
- [Programming Recommendations](#programming-recommendations)
  - [Function Annotations](#function-annotations)
  - [Variable Annotations](#variable-annotations)
- [References](#references)
- [Copyright](#copyright)

## Introduction

PEP 8 gives coding conventions for the Python standard library. It and PEP 257
were adapted from Guido van Rossum's original style essay. The guide changes as
language and practice change. A project's own guide takes precedence when it
conflicts with PEP 8.

## A Foolish Consistency is the Hobgoblin of Little Minds

Readability counts. Consistency with this guide matters, consistency within a
project matters more, and consistency within a module or function matters most.
Use judgment when a recommendation does not apply. In particular, never break
backward compatibility merely to comply.

Ignore a rule when applying it would reduce readability; when surrounding code
uses another style and a local cleanup is inappropriate; when old code predates
the rule and has no other reason to change; or when supported older Python
versions lack the recommended feature.

## Code Lay-out

### Indentation

Use four spaces per indentation level. Align continuation lines vertically
inside delimiters, or use a hanging indent with no argument on the opening line
and enough indentation to distinguish the continuation. Four spaces are
optional for continuation lines.

```python
# Correct
result = function(
    first,
    second,
)

# Incorrect: continuation is indistinguishable from the body.
def function(
    first, second):
    return first + second
```

For multiline conditions whose natural four-space continuation conflicts with
the body, PEP 8 takes no position: no extra indent, a distinguishing comment,
or extra continuation indentation are all acceptable. Closing delimiters may
align with the last line's first non-whitespace character or with the first
character of the line that began the construct.

### Tabs or Spaces?

Prefer spaces. Use tabs only to stay consistent with code already indented with
tabs. Python forbids mixing tabs and spaces for indentation.

### Maximum Line Length

Limit lines to 79 characters and flowing comments or docstrings to 72. A team
that agrees may raise code lines to 99, but should still wrap comments and
docstrings at 72. The standard library keeps 79 and 72.

Prefer implicit continuation inside parentheses, brackets, or braces over a
backslash. A backslash may still be appropriate where implicit continuation is
unavailable or awkward, including some old-version multiline `with` statements
and some `assert` statements. Indent continued lines appropriately.

### Should a Line Break Before or After a Binary Operator?

Either side is permissible if locally consistent. For new code, prefer breaking
before the operator because it keeps each operator next to its operand.

```python
# Preferred for new code
income = (gross_wages
          + taxable_interest
          - ira_deduction)
```

### Blank Lines

Surround top-level functions and classes with two blank lines. Use one blank
line between methods. Extra blank lines may sparingly separate related function
groups; omit them between related one-line definitions when useful. Within a
function, use blank lines sparingly to mark logical sections.

A form feed (`^L`) is valid whitespace and may separate related pages, but some
editors and viewers display it as an unknown glyph.

### Source File Encoding

Core Python code should use UTF-8 without an encoding declaration. The standard
library uses non-UTF-8 encodings only in tests. Use non-ASCII data sparingly and
avoid noisy Unicode and byte-order marks. Standard-library identifiers must be
ASCII and should use English words where feasible; global open-source projects
are encouraged to follow a similar policy.

### Imports

Usually put imports on separate lines; importing multiple names from one module
is acceptable.

```python
import os
import sys
from subprocess import PIPE, Popen
```

Place imports after module comments and the module docstring, before globals and
constants. Group standard-library, third-party, and local imports in that order,
with a blank line between groups.

Prefer absolute imports because they are usually clearer and behave better when
`sys.path` is wrong. Explicit relative imports are acceptable, especially when
absolute names would be needlessly verbose in a complex layout. Standard-library
code should avoid complex layouts and use absolute imports.

Importing a class directly is usually acceptable. If that causes a local name
collision, import its module and use the qualified name.

Avoid wildcard imports because readers and tools cannot tell which names they
introduce. A defensible exception is republishing an internal interface as a
public API when the exact names replaced by an optional accelerator are not
known in advance; public/internal-interface rules still apply.

### Module Level Dunder Names

Place module-level dunder names such as `__all__`, `__author__`, and `__version__`
after the module docstring and before imports, except `from __future__` imports.
Future imports must follow the docstring and precede all other code.

## String Quotes

PEP 8 does not prefer single or double quotes. Choose a rule and be consistent;
when a string contains one quote character, use the other delimiter to avoid
backslashes. Always use triple double quotes for triple-quoted strings, matching
PEP 257.

## Whitespace in Expressions and Statements

### Pet Peeves

Avoid extra whitespace:

- immediately inside parentheses, brackets, and braces;
- between a trailing comma and closing parenthesis;
- before commas, semicolons, and colons;
- before a call's opening parenthesis;
- before an indexing or slicing bracket; and
- in repeated spaces used to align assignment or other operators.

Treat a slice colon like the lowest-priority binary operator: complex expressions
on both sides receive equal spacing. In extended slices, space both colons the
same way. Omit spacing next to an omitted slice argument.

```python
ham[1:9]
ham[lower + offset : upper + offset]
ham[: upper_fn(x) : step_fn(x)]
```

### Other Recommendations

Avoid trailing whitespace, including after a line-continuation backslash.
Surround assignment, augmented assignment, comparison, and Boolean operators
with one space. With mixed-precedence arithmetic, consider spaces around the
lowest-priority operators; use judgment, never more than one space, and equal
spacing on both sides.

Function annotations use normal colon spacing and spaces around `->`. Do not
space `=` in keyword arguments or unannotated defaults. Do space `=` when a
default accompanies an annotation.

```python
def read(path: str, limit: int = 100, raw=False) -> bytes: ...
```

Generally avoid compound statements. A small `if`, `for`, or `while` body may
sometimes share a line, but never do this for multi-clause statements, and avoid
folding a long body onto one line.

## When to Use Trailing Commas

A trailing comma is optional except in a one-item tuple, where it is required;
parentheses are recommended for clarity. Redundant trailing commas are useful
when version-controlled values, arguments, or imports are expected to grow:
place each item on its own line, include a trailing comma, and put the closing
delimiter on the next line. Except for singleton tuples, do not put a redundant
trailing comma on the same line as the closing delimiter.

## Comments

Wrong comments are worse than absent comments; update them with code. Write
clear, understandable, complete sentences. Capitalize the first word unless it
is a lower-case identifier, and end paragraphs' sentences with periods. In a
multi-sentence comment, one or two spaces may follow sentence-ending periods.
Use English unless the code is certain never to be read by English speakers.

### Block Comments

Indent a block comment to the level of the code it describes. Begin each line
with `# `, except indented text within the comment. Separate paragraphs with a
line containing only `#`.

### Inline Comments

Use inline comments sparingly. Separate one from the statement with at least two
spaces and begin it with `# `. Do not state the obvious; use one when it adds
non-obvious context.

### Documentation Strings

Follow PEP 257. Write docstrings for public modules, functions, classes, and
methods. A non-public method need not have a docstring, but should have a comment
after its `def` line describing what it does.

Put a multiline docstring's closing `"""` on its own line. Keep a one-line
docstring's closing quotes on the same line.

## Naming Conventions

Existing Python library naming is inconsistent. Use these standards for new
modules and packages, including third-party frameworks; prefer internal
consistency when an existing library uses another style.

### Overriding Principle

Public names should reflect how callers use them, not how they are implemented.

### Descriptive: Naming Styles

Recognize these styles independently of their use: `b`, `B`, `lowercase`,
`lower_case_with_underscores`, `UPPERCASE`, `UPPER_CASE_WITH_UNDERSCORES`,
`CapitalizedWords` (CapWords), `mixedCase`, and the discouraged
`Capitalized_Words_With_Underscores`. Capitalize all letters of acronyms in
CapWords, such as `HTTPServerError`.

Short unique prefixes, as in `os.stat()` fields, are uncommon but may show a
relationship to an external API. A library-wide leading prefix, as used by X11,
is generally unnecessary because module and object qualification already group
Python names.

Special underscore forms are:

- `_name`: weak internal-use marker; wildcard import omits it;
- `name_`: avoids a keyword collision;
- `__name`: invokes class name mangling; and
- `__name__`: a documented special name in user-controlled namespaces; never
  invent one.

### Prescriptive: Naming Conventions

#### Names to Avoid

Never use lowercase `l`, uppercase `O`, or uppercase `I` as one-character names;
they resemble `1` and `0`. If tempted to use `l`, use `L` instead.

#### ASCII Compatibility

Standard-library identifiers must be ASCII-compatible under PEP 3131's policy.

#### Package and Module Names

Use short lowercase module names, adding underscores when they improve
readability. Use short lowercase package names; underscores are discouraged.
When a C/C++ extension has a higher-level Python companion, prefix the extension
module with an underscore, as in `_socket`.

#### Class Names

Normally use CapWords. A documented class used mainly as a callable may use the
function naming style. Builtins are usually single or joined lowercase words;
CapWords is used for builtin exceptions and constants.

#### Type Variable Names

Normally use short CapWords names such as `T`, `AnyStr`, or `Num`. Add `_co` for
covariant variables and `_contra` for contravariant variables.

#### Exception Names

Use class naming rules. Add `Error` when the exception represents an error.

#### Global Variable Names

Use function naming rules. Globals should generally be module-internal. Modules
intended for wildcard import should use `__all__` to control exports or prefix
non-public globals with an underscore.

#### Function and Variable Names

Use lowercase words separated by underscores as needed. `mixedCase` is allowed
only where it is already prevalent and backward compatibility requires it.

#### Function and Method Arguments

Use `self` first in instance methods and `cls` first in class methods. For a
keyword collision, generally append one underscore instead of abbreviating or
misspelling; using a synonym may be better still.

#### Method Names and Instance Variables

Follow function naming rules. Prefix non-public methods and instance variables
with one underscore. Use two leading underscores only when a subclassable class
needs name mangling to avoid subclass attribute collisions. Mangling does not
make access impossible and can hinder debugging and `__getattr__`; balance it
against the actual collision risk.

#### Constants

Define constants at module level with uppercase words separated by underscores,
such as `MAX_OVERFLOW`.

#### Designing for Inheritance

Decide whether every attribute is public, non-public, or part of a subclass API.
When unsure, start non-public because later publication is easier than withdrawal.
Python does not treat these names as truly private.

Public attributes have no leading underscore. Append one trailing underscore for
a keyword collision, except that `cls` remains preferred for a known class.
Expose simple public data directly rather than adding accessors. A property can
later add behavior while preserving attribute syntax; try to keep that behavior
side-effect free, though caching is usually acceptable, and do not hide expensive
operations behind attribute syntax.

For attributes subclasses should not use, consider two leading underscores and
no trailing underscores. Mangling uses only the simple class name, so equal class
and attribute names can still collide. It can also hinder debugging and dynamic
attribute handling; not everyone favors it, so weigh both sides.

### Public and Internal Interfaces

Backward-compatibility promises apply only to public interfaces, so distinguish
public from internal names. Documented interfaces are public unless explicitly
marked provisional or internal; undocumented interfaces are presumed internal.

Declare a module's public API with `__all__`; an empty list means no public API.
Still prefix internal names with one underscore. A name is internal if any
containing package, module, or class is internal. Imported names are
implementation details unless explicitly documented as part of the containing
API, as with `os.path` or deliberate re-exports from a package `__init__`.

## Programming Recommendations

- Do not disadvantage other Python implementations. For example, do not rely on
  CPython's fragile in-place string-concatenation optimization; use `''.join()`
  in performance-sensitive code for linear behavior across implementations.
- Compare singletons such as `None` with `is` or `is not`. Do not use truthiness
  when a distinct false value is meaningful. Prefer `is not` to `not ... is`.
- Prefer implementing all six rich comparisons to relying on reflected
  operations in every context. `functools.total_ordering()` can generate missing
  methods. Python may reflect comparisons; `sort()` and `min()` use `<`, while
  `max()` uses `>`.
- Use `def`, not assignment of a `lambda`, when binding a function name; `def`
  supplies a useful name in tracebacks and representations.
- Derive ordinary exceptions from `Exception`, not `BaseException`. Shape an
  exception hierarchy around distinctions callers need to catch. Add `Error`
  only to error exceptions; flow-control exceptions need no such suffix.
- Use `raise NewError(...) from cause` for explicit replacement that retains the
  original traceback. With `from None`, transfer relevant details to the new
  exception.
- Catch specific exceptions where possible. Bare `except:` also catches
  `SystemExit` and `KeyboardInterrupt`; reserve it mainly for reporting a
  traceback or cleanup followed by re-raising, where `try/finally` may be better.
  To catch program errors broadly, use `except Exception:`.
- Prefer the Python 3.3+ OS exception hierarchy to inspecting `errno`.
- Keep `try` suites as small as possible so handlers do not mask errors from
  unrelated operations; use `else` for the success path.
- Use `with` for local resources, or `try/finally`. If a context manager does
  more than acquire and release a resource, expose it through a named operation
  that makes the behavior clear.
- Keep return statements consistent: either all return expressions or none. If
  some return a value, use explicit `return None` for no-value paths and add an
  explicit final return when reachable.
- Use `str.startswith()` and `str.endswith()` rather than slices for prefix and
  suffix checks.
- Use `isinstance()` rather than direct type equality.
- Test empty sequences by truth value, not `len()`.
- Do not rely on significant trailing whitespace in string literals.
- Do not compare Boolean values with `True` or `False` using `==`; normally test
  the value directly. `is True` is worse unless identity itself is required.
- Discourage `return`, `break`, or `continue` that exits a `finally` suite because
  it implicitly cancels an exception propagating through that suite.

### Function Annotations

Use PEP 484 syntax. Earlier experimentation with unrelated annotation meanings
is no longer encouraged, though third-party experiments within PEP 484 are.
The standard library should adopt annotations conservatively, while allowing
them for new code and large refactors.

Code using annotations for another purpose should put `# type: ignore` near the
top; PEP 484 also provides finer suppression. Type checkers are optional tools:
interpreters should neither report type errors nor change runtime behavior from
annotations. Users may ignore checking, but distributed libraries should expect
consumers to check them; PEP 484 stubs may ship with or separately from a
library, with author permission.

### Variable Annotations

Use one space after, and none before, the annotation colon. If the declaration
has a value, put one space around `=`.

```python
count: int
label: str = "unknown"
```

Although variable annotations entered Python in 3.6, this syntax is preferred
for stub files representing every Python version.

## References

PEP 8 cites Barry Warsaw's GNU Mailman style guide, Donald Knuth's *The TeXBook*,
the history of CamelCase, and the typeshed repository. See the source PEP for
full references and footnotes, including the definition of hanging indentation.

## Copyright

PEP 8 states: “This document has been placed in the public domain.”
