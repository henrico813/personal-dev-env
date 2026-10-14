#!/usr/bin/env python3
"""Flag Go comments that break a code-documentation rule.

Reads whole-line Go ``//`` comments and names the code each one sits on.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

DECLARATION = re.compile(
    r"(?:func\s+|type\s+|const\s+|var\s+|package\s+)?(\w+)"
)
METHOD = re.compile(r"func\s+\(\s*(?:\w+\s+)?\*?\s*(\w+)[^)]*\)\s*(\w+)")
BLOCK = re.compile(r"(const|var)\s*\($")


@dataclass(frozen=True)
class Comment:
    """One comment and the name of the code it sits on.

    Names look like ``Send``, ``Cart.Total`` for a method, ``Cart.Items``
    for a struct field or map entry, and ``const Red`` for a comment
    above a ``const (`` block. A comment inside a function body, or with
    a blank line after it, has the name ``""``. ``header`` is true for a
    comment above the ``package`` line with a blank line after it, where
    license text usually sits. A package doc comment touches the
    ``package`` line, so it is named, not a header.
    """

    file: str
    line: int
    name: str
    prose: str
    header: bool = False


def go_comments(source: str, file: str) -> list[Comment]:
    """Return each run of whole-line ``//`` comments in a Go file.

    Args:
        source: The Go file's text.
        file: Its path relative to the repository, used in findings.

    Returns:
        The comments in file order, each with the name of the code
        below it.
    """
    lines = source.splitlines(keepends=True)
    comments: list[Comment] = []
    outer, kind, seen_package = "", "", False
    start: int | None = None

    for number, line in enumerate(lines + ["\n"], 1):
        text = line.strip()
        if text.startswith("//"):
            start = number if start is None else start
            continue
        if start is not None:
            run = lines[start - 1 : number - 1]
            end = number > len(lines)
            name = "" if end else go_name(lines, number, outer, kind)
            comments.append(
                Comment(
                    file,
                    start,
                    name,
                    go_prose(run),
                    not seen_package and not name,
                )
            )
            start = None
        if line[:1].isalpha():
            outer, kind = declaration(text), text.split()[0]
            seen_package = seen_package or kind == "package"
            if BLOCK.match(text):
                kind = "block"
        elif line[:1] == "}" or (line[:1] == ")" and kind == "block"):
            # A ")" at column 0 also closes a multi-line signature, which
            # is still inside the function.
            outer, kind = "", ""
    return comments


def declaration(text: str) -> str:
    """Name a declaration that starts at column 0.

    Args:
        text: The declaration line, stripped.

    Returns:
        ``Recv.Method`` for a method, otherwise the declared name.
    """
    method = METHOD.match(text)
    if method:
        return f"{method[1]}.{method[2]}"
    word = DECLARATION.match(text)
    return word[1] if word else ""


def go_name(lines: list[str], number: int, outer: str, kind: str) -> str:
    """Name the code on 1-based line ``number``, just below a comment.

    Args:
        lines: Every line of the file.
        number: The line below the comment, counted from 1.
        outer: Name of the declaration the line sits in, if any.
        kind: ``block`` inside ``const (`` or ``var (``, ``func`` inside
            a function, otherwise the declaration's keyword.

    Returns:
        The comment's name, or ``""`` when it has none.
    """
    line = lines[number - 1]
    text = line.strip()
    if not text:
        return ""
    if not line[0].isspace():
        if BLOCK.match(text) and number < len(lines):
            first = DECLARATION.match(lines[number].strip())
            return f"{text.split()[0]} {first[1] if first else ''}"
        return declaration(text)
    word = DECLARATION.match(text)
    if kind == "func" or not word:
        return ""
    if kind == "block":
        return word[1]
    return f"{outer}.{word[1]}"


def go_prose(run: list[str]) -> str:
    """Join comment lines into one paragraph, leaving out code and lists.

    Args:
        run: The comment's lines, with their ``//`` markers.

    Returns:
        The comment's prose on one line.
    """
    words = []
    for line in run:
        text = line.strip()[2:]
        if text.strip() and not text.startswith(("\t", "  ")):
            words.append(text.strip())
    return " ".join(words)
