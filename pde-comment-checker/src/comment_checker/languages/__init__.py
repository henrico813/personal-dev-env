"""Which language reads which file, by suffix."""

from __future__ import annotations

import re
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field

from ..records import Comment, Function
from .go import (
    GO_BEHAVIORS_NOT_APPLYING,
    GO_GENERATED_MARKER,
    finish_go_rewrite,
    is_go_comment_writable,
    join_comment_prose,
    list_go_functions,
    parse_go_comments,
)
from .line_comments import (
    ScreenRow,
    lay_out_line_comment,
    prepare_line_comment_for_screen,
)
from .python import (
    PYTHON_GENERATED_MARKER,
    finish_python_rewrite,
    is_python_comment_writable,
    join_python_prose,
    lay_out_python_comment,
    list_python_warnings,
    parse_python_comments,
    prepare_python_comment_for_screen,
)


@dataclass(frozen=True)
class Language:
    """How the checker reads the files of one suffix, such as ``.py``.

    Attributes:
        name: The language's name, such as ``python``.
        read_comments: Returns each comment in a file, given its source
            and its path.
        join_prose: Returns a comment's words as one line of prose.
        generated_marker: Finds the line that marks a generated file.
        is_writable: Returns whether a comment can be rewritten without
            changing the code around it.
        list_warnings: Returns the warnings the compiler gives for a
            file. It is ``None`` for a language ``--list`` does not
            take.
        list_functions: Returns each function in a file, for the
            function name rules. It is ``None`` for a language whose
            names another checker checks.
        reviews_on: Whether the ``changes`` and ``apply`` commands take
            this language's comments for the owner's review.
        prepare_for_screen: Returns a comment's words for the owner to
            edit, and the rows the review screen draws for it. It is
            ``None`` for a language ``show`` does not take.
        lay_out_wording: Returns new wording laid out like the old
            comment it replaces. It is ``None`` for a language ``show``
            does not take.
        finish_rewrite: Returns the new source as the write leaves it,
            given the old source, the new source and the file. It raises
            ``SourceError`` when the write would be unsafe. It is
            ``None`` for a language ``apply`` does not take.
        behaviors_not_applying: Each behavior of the shared sample suite
            the language has no case for, and why.
    """

    name: str
    read_comments: Callable[[str, str], list[Comment]]
    join_prose: Callable[[str], str]
    generated_marker: re.Pattern[str]
    is_writable: Callable[[Comment], bool]
    list_warnings: Callable[[str, str], list[str]] | None = None
    list_functions: Callable[[str, str], list[Function]] | None = None
    reviews_on: bool = False
    prepare_for_screen: Callable[[str], tuple[str, list[ScreenRow]]] | None = (
        None
    )
    lay_out_wording: Callable[[str, str], str] | None = None
    finish_rewrite: Callable[[str, str, str], str] | None = None
    behaviors_not_applying: Mapping[str, str] = field(default_factory=dict)


# Each language the checker reads, by file suffix.
LANGUAGES = {
    ".go": Language(
        name="go",
        read_comments=parse_go_comments,
        join_prose=join_comment_prose,
        generated_marker=GO_GENERATED_MARKER,
        is_writable=is_go_comment_writable,
        list_functions=list_go_functions,
        reviews_on=True,
        prepare_for_screen=prepare_line_comment_for_screen,
        lay_out_wording=lay_out_line_comment,
        finish_rewrite=finish_go_rewrite,
        behaviors_not_applying=GO_BEHAVIORS_NOT_APPLYING,
    ),
    ".py": Language(
        name="python",
        read_comments=parse_python_comments,
        join_prose=join_python_prose,
        generated_marker=PYTHON_GENERATED_MARKER,
        is_writable=is_python_comment_writable,
        list_warnings=list_python_warnings,
        reviews_on=True,
        prepare_for_screen=prepare_python_comment_for_screen,
        lay_out_wording=lay_out_python_comment,
        finish_rewrite=finish_python_rewrite,
    ),
}

# Code suffixes the checker cannot read yet, so their comments reach the
# owner another way. A language added to LANGUAGES leaves this list.
UNSUPPORTED_SUFFIXES = (".js", ".lua", ".rs", ".sh", ".ts")
