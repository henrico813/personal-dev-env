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
from .line_comments import show_line_comment, write_line_comment
from .python import (
    PYTHON_GENERATED_MARKER,
    is_python_comment_writable,
    join_python_prose,
    list_python_warnings,
    parse_python_comments,
    show_python_comment,
    write_python_comment,
)


@dataclass(frozen=True)
class Language:
    """How the checker reads the files of one suffix, such as ``.py``.

    ``list_warnings`` is ``None`` for a language ``--list`` does not take,
    and ``list_functions`` for one whose names another checker checks.
    ``show_text`` and ``write_plain`` are ``None`` for a language ``show``
    does not take, and ``finish_rewrite`` for one ``apply`` does not.
    ``finish_rewrite`` takes the old and new source and the file, and
    returns the new source as the write leaves it, or raises
    ``SourceError`` when the write would be unsafe. ``changes`` and
    ``apply`` take comments only while ``reviews_on`` is true.
    """

    name: str
    read_comments: Callable[[str, str], list[Comment]]
    join_prose: Callable[[str], str]
    generated_marker: re.Pattern[str]
    is_writable: Callable[[Comment], bool]
    list_warnings: Callable[[str, str], list[str]] | None = None
    list_functions: Callable[[str, str], list[Function]] | None = None
    reviews_on: bool = False
    show_text: Callable[[str], tuple[str, list[dict[str, str]]]] | None = None
    write_plain: Callable[[str, str], str] | None = None
    finish_rewrite: Callable[[str, str, str], str] | None = None
    # Each behavior of the shared sample suite it has no case for, and why.
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
        show_text=show_line_comment,
        write_plain=write_line_comment,
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
        show_text=show_python_comment,
        write_plain=write_python_comment,
    ),
}

# Code suffixes the checker cannot read yet, so their comments reach the
# owner another way. A language added to LANGUAGES leaves this list.
UNSUPPORTED_SUFFIXES = (".js", ".lua", ".rs", ".sh", ".ts")
