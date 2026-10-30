"""Lay out comments made of marked lines, such as Go's ``//`` and ``#``."""

from __future__ import annotations

from typing import NamedTuple, TypedDict

from ..records import SourceError

# The column a rewritten comment's lines may reach, counting indentation.
LINE_WIDTH = 79


class WrappedRow(TypedDict):
    """Words the review screen wraps to its width, after a mark.

    The review tool shows each comment as rows on the owner's phone.

    Attributes:
        first: The mark before the row's first screen line, such as
            a docstring's opening quotes or ``# ``.
        rest: The mark before each later screen line, such as ``# ``.
        words: The words to wrap.
    """

    first: str
    rest: str
    words: str


class FixedRow(TypedDict):
    """A row the review screen draws as written, such as an example.

    Attributes:
        fixed: The row's text.
    """

    fixed: str


ScreenRow = WrappedRow | FixedRow


class Block(NamedTuple):
    """Sentences joined into one line, or example lines kept as written."""

    example: bool
    text: str


def prepare_line_comment_for_screen(
    text: str, marker: str = "//"
) -> tuple[str, list[ScreenRow]]:
    """Return a comment's words for editing and its rows for the screen.

    The owner edits the words in nvim. Each paragraph becomes one row the
    screen wraps, and each example line a row drawn as written. A row
    holding only the mark separates each paragraph or example from the
    next.

    Args:
        text: The comment exactly as it is in the file, with the mark
            starting each line.
        marker: The mark that starts each line, such as ``//`` or ``#``.

    Returns:
        The comment's words without its marks, with each paragraph on
        one line, and the rows that draw it.
    """
    blocks = read_comment_blocks(text, marker)
    rows: list[ScreenRow] = []
    for block in blocks:
        if rows:
            rows.append(FixedRow(fixed=marker))
        if block.example:
            rows += [
                FixedRow(fixed=marker + line)
                for line in block.text.split("\n")
            ]
        else:
            marks = marker + " "
            rows.append(WrappedRow(first=marks, rest=marks, words=block.text))
    words = "\n\n".join(block.text for block in blocks) + "\n"
    return words, rows


def write_line_comment(
    plain: str, old: str, marker: str = "//", width: int = LINE_WIDTH
) -> str:
    """Return plain text as comment lines indented like the old comment.

    Sentences are rewrapped so that each line, indentation included,
    fits in the width. Lines that start with a tab or two spaces are
    kept as example lines.

    Args:
        plain: The new wording as plain text. Text with no words deletes
            the comment.
        old: The comment's current lines in the file, whose indentation
            the new lines copy.
        marker: The mark that starts each line, such as ``//`` or ``#``.
        width: The last column a line may reach, counting indentation.

    Returns:
        The new comment's lines, or an empty string when the new wording
        has no words.
    """
    first_line = old.split("\n", 1)[0]
    indent = first_line[: len(first_line) - len(first_line.lstrip(" \t"))]
    width -= len(indent.replace("\t", "    ")) + len(marker) + 1
    lines: list[str] = []
    for block in group_comment_blocks(plain.split("\n")):
        if lines:
            lines.append(indent + marker)
        if block.example:
            lines += [
                indent + marker + line for line in block.text.split("\n")
            ]
        else:
            for line in wrap_words(block.text.split(), width):
                lines.append(f"{indent}{marker} {line}")
    return "".join(line + "\n" for line in lines)


def read_comment_blocks(text: str, marker: str) -> list[Block]:
    """Split a comment into blocks, with indentation and marks removed.

    Raises:
        SourceError: A line of ``text`` does not start with ``marker``.
    """
    bodies = []
    for number, line in enumerate(text.removesuffix("\n").split("\n"), 1):
        body = line.lstrip(" \t")
        if not body.startswith(marker):
            raise SourceError(f"line {number} is not a {marker} comment")
        bodies.append(body[len(marker) :])
    return group_comment_blocks(bodies)


def group_comment_blocks(bodies: list[str]) -> list[Block]:
    """Split comment lines, with their marks removed, into blocks.

    A blank line, or a switch between sentences and example lines,
    starts a new block. A line that starts with a tab or two spaces is
    an example line.
    """
    blocks = []
    words: list[str] = []
    examples: list[str] = []
    # An empty line past the end closes the last block.
    for body in [*bodies, ""]:
        is_blank = not body.strip()
        is_example = not is_blank and body.startswith(("\t", "  "))
        if words and (is_blank or is_example):
            blocks.append(Block(example=False, text=" ".join(words)))
            words = []
        if examples and not is_example:
            blocks.append(Block(example=True, text="\n".join(examples)))
            examples = []
        if is_example:
            examples.append(body.rstrip(" \t"))
        elif not is_blank:
            words += body.split()
    return blocks


def wrap_words(words: list[str], width: int) -> list[str]:
    """Join words with single spaces into lines of at most ``width``.

    A word longer than ``width`` gets a line of its own, wider than
    ``width``.
    """
    lines: list[str] = []
    for word in words:
        if lines and len(lines[-1]) + 1 + len(word) <= width:
            lines[-1] += " " + word
        else:
            lines.append(word)
    return lines
