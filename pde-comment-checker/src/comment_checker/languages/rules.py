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
    build_comment_lines,
    prepare_line_comment_for_screen,
)
from .python import (
    PYTHON_GENERATED_MARKER,
    finish_python_rewrite,
    is_python_comment_writable,
    join_python_prose,
    list_python_warnings,
    parse_python_comments,
    show_python_comment,
    write_python_comment,
)
from .screen_rows import ScreenRow


@dataclass(frozen=True)
class LanguageRules:
    """The comment rules for one programming language, such as Python."""

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
    build_comment: Callable[[str, str], str] | None = None
    finish_rewrite: Callable[[str, str, str], str] | None = None
    behaviors_not_applying: Mapping[str, str] = field(default_factory=dict)


# Each language the checker reads, by file suffix.
LANGUAGES = {
    ".go": LanguageRules(
        name="go",
        read_comments=parse_go_comments,
        join_prose=join_comment_prose,
        generated_marker=GO_GENERATED_MARKER,
        is_writable=is_go_comment_writable,
        list_functions=list_go_functions,
        reviews_on=True,
        prepare_for_screen=prepare_line_comment_for_screen,
        build_comment=build_comment_lines,
        finish_rewrite=finish_go_rewrite,
        behaviors_not_applying=GO_BEHAVIORS_NOT_APPLYING,
    ),
    ".py": LanguageRules(
        name="python",
        read_comments=parse_python_comments,
        join_prose=join_python_prose,
        generated_marker=PYTHON_GENERATED_MARKER,
        is_writable=is_python_comment_writable,
        list_warnings=list_python_warnings,
        prepare_for_screen=show_python_comment,
        build_comment=write_python_comment,
        finish_rewrite=finish_python_rewrite,
    ),
}

# Code suffixes the checker cannot read yet, so their comments reach the
# owner another way. A language added to LANGUAGES leaves this list.
UNSUPPORTED_SUFFIXES = (".js", ".lua", ".rs", ".sh", ".ts")
