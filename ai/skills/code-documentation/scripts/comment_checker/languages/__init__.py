"""Which language reads which file, by suffix."""

from __future__ import annotations

import re
from collections.abc import Callable, Mapping
from dataclasses import dataclass, field

from ..records import Comment, Function
from .go import (
    GO_BEHAVIORS_NOT_APPLYING,
    GO_GENERATED_MARKER,
    is_go_comment_writable,
    join_comment_prose,
    list_go_functions,
    parse_go_comments,
)
from .python import (
    PYTHON_GENERATED_MARKER,
    is_python_comment_writable,
    join_python_prose,
    list_python_warnings,
    parse_python_comments,
)


@dataclass(frozen=True)
class Language:
    """How the checker reads the files of one suffix, such as ``.py``.

    ``list_warnings`` is ``None`` for a language ``--list`` does not take,
    and ``list_functions`` for one whose names another checker checks.
    ``changes`` lists comments for the owner only while ``reviews_on`` is
    true.
    """

    name: str
    read_comments: Callable[[str, str], list[Comment]]
    join_prose: Callable[[str], str]
    generated_marker: re.Pattern[str]
    is_writable: Callable[[Comment], bool]
    list_warnings: Callable[[str, str], list[str]] | None = None
    list_functions: Callable[[str, str], list[Function]] | None = None
    reviews_on: bool = False
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
        behaviors_not_applying=GO_BEHAVIORS_NOT_APPLYING,
    ),
    ".py": Language(
        name="python",
        read_comments=parse_python_comments,
        join_prose=join_python_prose,
        generated_marker=PYTHON_GENERATED_MARKER,
        is_writable=is_python_comment_writable,
        list_warnings=list_python_warnings,
    ),
}

# Code suffixes the checker cannot read yet, so their comments reach the
# owner another way. A language added to LANGUAGES leaves this list.
UNSUPPORTED_SUFFIXES = (".js", ".lua", ".rs", ".sh", ".ts")
