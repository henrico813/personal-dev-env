"""Lay out comments made of marked lines, such as Go's ``//`` and ``#``."""

from __future__ import annotations

from typing import NamedTuple

from ..records import SourceError
from .screen_rows import ParagraphRow, ScreenRow, UnwrappedRow

# The column a rewritten comment's lines may reach, counting indentation.
LINE_WIDTH = 79


class Block(NamedTuple):
    """Sentences joined into one line, or example lines kept as written."""

    example: bool
    text: str


def prepare_line_comment_for_screen(
    text: str, marker: str = "//"
) -> tuple[str, list[ScreenRow]]:
    """Split a ``#`` or ``//`` comment into words and screen rows.

    For the comment ``# Prices are in cents.``, it returns ``Prices are
    in cents.`` for nvim and one row for the screen.

    Args:
        text: The comment as it is in the file, with its marks.
        marker: The mark that starts each line, such as ``//`` or ``#``.

    Returns:
        The words, one line per paragraph, and the rows to draw.

    Raises:
        SourceError: A line does not start with the mark.
    """
    blocks = read_comment_blocks(text, marker)
    rows: list[ScreenRow] = []
    for block in blocks:
        if rows:
            rows.append(UnwrappedRow(fixed=marker))
        if block.example:
            rows += [
                UnwrappedRow(fixed=marker + line)
                for line in block.text.split("\n")
            ]
        else:
            marks = marker + " "
            rows.append(
                ParagraphRow(first=marks, rest=marks, words=block.text)
            )
    words = "\n\n".join(block.text for block in blocks) + "\n"
    return words, rows


def build_comment_lines(new_wording: str, old: str, marker: str = "//") -> str:
    """Return new words as comment lines ready for the file.

    The owner edits a comment without its marks or line breaks, so the
    words need them back before they go in the file.

    Args:
        new_wording: The new words, without marks. Lines that start with
            a tab or two spaces stay as written.
        old: The comment the new words replace, whose indentation they
            copy. It may be suggested wording that is not in the file
            yet.
        marker: The mark that starts each line, such as ``//`` or ``#``.

    Returns:
        The comment lines, or an empty string to delete the comment.
    """
    first_line = old.split("\n", 1)[0]
    indent = first_line[: len(first_line) - len(first_line.lstrip(" \t"))]
    width = LINE_WIDTH - len(indent.replace("\t", "    ")) - len(marker) - 1
    lines: list[str] = []
    for block in group_comment_blocks(new_wording.split("\n")):
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
