"""The data the checker passes between its modules."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Comment:
    """One comment and the name of the code it sits on.

    Go names look like ``Send``, ``Cart.Total`` for a method, ``Cart.Items``
    for a struct field or map entry, and ``const Red`` for a comment above
    a ``const (`` block. Python names look like ``Cart.total`` for a
    docstring, ``# Cart.total`` for a ``#`` comment above it, and
    ``<module>``. A comment has the name ``""`` when it cannot be
    approved, such as inside a function body, with a blank line after it,
    or, after ``unname_unapprovable_comments``, when its name is shared or
    it has an ``unapprovable_reason``.

    ``text`` is the comment exactly as written, which approved text is
    compared with. ``header`` is true for a comment above ``package`` with
    a blank line after it, where license text sits; a package doc comment
    touches ``package``, so it is named instead. In Python, it is true for
    an unnamed comment above the first statement.

    ``unapprovable_reason`` says why a named comment cannot be approved,
    such as a docstring that shares a line with code. In Python,
    ``code_line`` is a docstring's ``def`` or ``class`` line, 0 for the
    module docstring, or the line below a ``#`` comment. ``only_body`` is
    true for a docstring that is its body's only statement, so cutting it
    would break the code.
    """

    file: str
    line: int
    name: str
    text: str
    prose: str
    header: bool = False
    unapprovable_reason: str = ""
    code_line: int = 0
    only_body: bool = False


@dataclass(frozen=True)
class Finding:
    """A problem in one comment, under one of the ``RULES``.

    ``note`` says why the finding is only reported, or is empty. A finding
    with an empty note fails the run when its rule is ``approved``, or,
    with ``--base``, ``filler`` or ``avoided-word``.
    """

    comment: Comment
    rule: str
    message: str
    note: str = ""


@dataclass(frozen=True)
class Function:
    """One function or method declared in a source file.

    ``name`` is a function's name, or a method's type and name joined by
    a dot. ``is_test`` is true for a function ``go test`` runs, and
    ``is_getter`` for a method that takes no arguments, as a getter does.
    """

    file: str
    line: int
    name: str
    is_test: bool = False
    is_getter: bool = False


class SourceError(Exception):
    """A file its parser cannot read, so its comments are unknown."""
