"""Which language reads which file, by suffix."""

from __future__ import annotations

import re
from collections.abc import Callable
from dataclasses import dataclass

from ..records import Comment, Function
from .go import (
    GO_GENERATED_MARKER,
    join_comment_prose,
    list_go_functions,
    parse_go_comments,
)
from .python import (
    PYTHON_GENERATED_MARKER,
    join_python_prose,
    list_python_warnings,
    parse_python_comments,
)


@dataclass(frozen=True)
class Language:
    """How the checker reads the files of one suffix, such as ``.py``.

    ``list_warnings`` is ``None`` for a language ``--list`` does not take,
    and ``list_functions`` for one whose names another checker checks.
    """

    read_comments: Callable[[str, str], list[Comment]]
    join_prose: Callable[[str], str]
    generated_marker: re.Pattern[str]
    list_warnings: Callable[[str, str], list[str]] | None = None
    list_functions: Callable[[str, str], list[Function]] | None = None


# Each language the checker reads, by file suffix.
LANGUAGES = {
    ".go": Language(
        read_comments=parse_go_comments,
        join_prose=join_comment_prose,
        generated_marker=GO_GENERATED_MARKER,
        list_functions=list_go_functions,
    ),
    ".py": Language(
        read_comments=parse_python_comments,
        join_prose=join_python_prose,
        generated_marker=PYTHON_GENERATED_MARKER,
        list_warnings=list_python_warnings,
    ),
}
