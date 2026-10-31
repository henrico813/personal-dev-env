"""The rows the review window draws for a comment."""

from __future__ import annotations

from typing import TypedDict


class ParagraphRow(TypedDict):
    """A paragraph the review window wraps to fit, after a comment mark.

    The window can be narrower than the file, so lines wrapped for the
    file would break again at odd places.

    Attributes:
        first: The mark before the first line, such as the opening
            quotes.
        rest: The mark before each later line, such as ``# ``.
        words: The words to wrap.
    """

    first: str
    rest: str
    words: str


class UnwrappedRow(TypedDict):
    """A row the review window shows exactly, without wrapping.

    Wrapping would join a heading such as ``Args:`` to the next line, so
    the window would show a layout the file will not have.

    Attributes:
        fixed: The row's text.
    """

    fixed: str


ScreenRow = ParagraphRow | UnwrappedRow
