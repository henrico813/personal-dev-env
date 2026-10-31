"""Read comments and their names out of a Python file."""

from __future__ import annotations

import ast
import enum
import io
import re
import sys
import tokenize
import warnings
from collections import Counter
from collections.abc import Iterator
from typing import NamedTuple

from ..records import Comment, SourceError
from .go import join_comment_prose
from .line_comments import (
    LINE_WIDTH,
    build_comment_lines,
    prepare_line_comment_for_screen,
    wrap_words,
)
from .screen_rows import ParagraphRow, ScreenRow, UnwrappedRow

PYTHON_DEFINITIONS = (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)

DocstringOwner = (
    ast.Module | ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef
)

PYTHON_GENERATED_MARKER = re.compile(
    r"^# (?:Code generated .* DO NOT EDIT\.$|@generated\b)", re.M
)

# A run of only tool directives, such as noqa, gets no name.
PYTHON_DIRECTIVE = re.compile(
    r"#\s*(?:noqa\b|nosec\b|pyre-ignore|(?:type|pragma|fmt|ruff|pylint|mypy"
    r"|pyright|isort|flake8|ty|yapf):)"
)

# A shebang or an encoding line, which belongs to the file, not the code.
PYTHON_SHEBANG_OR_ENCODING = re.compile(r"\s*#(?:!|.*coding[:=])")

# The prefix and quotes that open a docstring, or the quotes that end it.
DOCSTRING_QUOTES = re.compile(
    r"""^\s*[rRuU]?("{3}|'{3}|["'])|("{3}|'{3}|["'])\s*$"""
)

# A Google-style section heading on its own line, such as Args.
SECTION_HEADING = re.compile(r"[A-Z][a-z]+(?: [A-Za-z][a-z]+)?:")

# A whole docstring, with its indentation, prefix, quotes and body.
WHOLE_DOCSTRING = re.compile(
    r"(?P<indent>[ \t]*)(?P<prefix>[rRuU]?)(?P<quotes>\"{3}|'{3}|[\"'])"
    r"(?P<body>.*)(?P=quotes)[ \t]*\n?",
    re.S,
)

# A named escape, such as \N{EM DASH}, whose spaces must not break a line.
NAMED_ESCAPE = re.compile(r"\\N\{[^}]*\}")

# A backslash pair, with group 1 set when Python reads it as another
# character. A doubled backslash is how a backslash is kept, so it is
# not counted.
DOCSTRING_ESCAPE = re.compile(
    r"\\(?:([\n'\"abfnrtv]|[0-7]{1,3}|x[0-9a-fA-F]{2}|N\{[^}]*\}"
    r"|u[0-9a-fA-F]{4}|U[0-9a-fA-F]{8})|[\s\S])"
)

# Headings whose lines are examples, kept as written.
EXAMPLE_HEADINGS = ("Example:", "Examples:")


def parse_python_comments(source: str, file: str) -> list[Comment]:
    """Return each docstring and each run of whole-line ``#`` comments.

    A ``#`` run takes the name of the code just below it, such as
    ``# Cart.total``, unless that code is inside a function or the run
    is only tool directives.

    Raises:
        SourceError: Python cannot parse ``source``.
    """
    try:
        # Python would print warnings, such as an invalid escape.
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            tree = ast.parse(source)
            tokens = list(
                tokenize.generate_tokens(io.StringIO(source).readline)
            )
    except (SyntaxError, ValueError, tokenize.TokenError) as error:
        raise SourceError(describe_python_error(error)) from error
    # Lines split only at line feeds, as ast counts them, so a form feed
    # cannot shift a name.
    lines = source.split("\n")
    # Each line with its line end, so a comment's text is exact.
    lines_with_ends = [line + "\n" for line in lines[:-1]] + lines[-1:]
    docstring_owners: list[tuple[DocstringOwner, str]] = [(tree, "<module>")]
    name_by_first_line: dict[int, str] = {}
    for node, name, in_function in list_python_named_statements(tree.body):
        if isinstance(node, PYTHON_DEFINITIONS):
            docstring_owners.append((node, name))
        if not in_function:
            # When one line holds two statements, the first names the
            # comment above.
            name_by_first_line.setdefault(find_first_line(node), name)
    comments = []
    for owner, name in docstring_owners:
        span = find_docstring_span(owner, tokens)
        if span:
            first, last, unapprovable_reason = span
            text = "".join(lines_with_ends[first - 1 : last])
            code_line, only_body = 0, False
            if isinstance(owner, PYTHON_DEFINITIONS):
                code_line, only_body = owner.lineno, len(owner.body) == 1
            comments.append(
                Comment(
                    file=file,
                    line=first,
                    name=name,
                    text=text,
                    prose=join_python_prose(text),
                    unapprovable_reason=unapprovable_reason,
                    code_line=code_line,
                    only_body=only_body,
                )
            )
    first_statement_line = min(
        map(find_first_line, tree.body), default=len(lines) + 1
    )
    for first, last in find_comment_runs(tokens, lines):
        text = "".join(lines_with_ends[first - 1 : last])
        name = name_by_first_line.get(last + 1, "")
        holds_only_directives = all(
            PYTHON_DIRECTIVE.search(line) for line in lines[first - 1 : last]
        )
        # Python reads a coding line only on line 1 or 2.
        if holds_only_directives or any(
            PYTHON_SHEBANG_OR_ENCODING.match(line)
            for line in lines[first - 1 : 2]
        ):
            name = ""
        comments.append(
            Comment(
                file=file,
                line=first,
                name=f"# {name}" if name else "",
                text=text,
                prose=join_python_prose(text),
                header=not name and last < first_statement_line,
                code_line=last + 1,
            )
        )
    return sorted(comments, key=lambda comment: comment.line)


def is_python_comment_writable(comment: Comment) -> bool:
    """Return whether rewriting ``comment`` keeps its code and directives.

    A docstring that shares a line with code, or a ``#`` run with a tool
    directive such as ``# noqa``, a shebang or a coding line, would lose
    part of the file. A docstring must be one string literal, so new text
    cannot add code after it, such as ``; return 1``.
    """
    is_hash_run = comment.text.lstrip().startswith("#")
    has_directive = is_hash_run and PYTHON_DIRECTIVE.search(comment.text)
    # Python reads a shebang or coding line only on line 1 or 2.
    file_lines = comment.text.splitlines()[: max(0, 3 - comment.line)]
    has_file_line = is_hash_run and any(
        PYTHON_SHEBANG_OR_ENCODING.match(line) for line in file_lines
    )
    is_one_literal = is_hash_run or is_one_string_literal(comment.text)
    return is_one_literal and not (
        comment.unapprovable_reason or has_directive or has_file_line
    )


class PartKind(enum.Enum):
    """The kind of one line of a docstring, as the owner edits it.

    Attributes:
        PARAGRAPH: A paragraph's sentences joined into one line.
        HEADING: A section heading, such as ``Args:``.
        ENTRY: One line under a heading, such as an argument, with its
            wrapped lines joined.
        KEPT: A line kept as written, such as an example.
        BLANK: An empty line between paragraphs or sections.
    """

    PARAGRAPH = enum.auto()
    HEADING = enum.auto()
    ENTRY = enum.auto()
    KEPT = enum.auto()
    BLANK = enum.auto()


class Part(NamedTuple):
    """One line of a docstring, as the owner edits it.

    Attributes:
        kind: What the line is, such as a heading.
        text: The line's words, without the docstring's indentation.
    """

    kind: PartKind
    text: str


def prepare_python_comment_for_screen(
    text: str,
) -> tuple[str, list[ScreenRow]]:
    """Return a comment's words to edit, and the rows the window draws.

    Args:
        text: The comment as it is in the file, either ``#`` lines or
            one whole docstring.

    Returns:
        The words without marks or quotes, and the rows to draw.

    Raises:
        SourceError: The text is neither ``#`` lines nor one whole
            docstring.
    """
    if text.lstrip().startswith("#"):
        return prepare_line_comment_for_screen(text, "#")
    docstring, parts = read_docstring_parts(text)
    return join_parts_as_text(parts), draw_docstring_rows(docstring, parts)


def draw_docstring_rows(
    docstring: re.Match[str], parts: list[Part]
) -> list[ScreenRow]:
    """Return the rows the review window draws for a docstring.

    The rows look like the docstring in the code. A one-line docstring
    stays on one row. A longer one gets a row of its own for the closing
    quotes.

    Args:
        docstring: The docstring's regex match, which gives its prefix,
            quotes and body.
        parts: The docstring's lines, as ``read_docstring_parts``
            returns them.

    Returns:
        The rows in order, from opening quotes to closing quotes.
    """
    opening = docstring["prefix"] + docstring["quotes"]
    quotes = docstring["quotes"]
    if len(parts) == 1 and "\n" not in docstring["body"]:
        words = parts[0].text + quotes
        return [ParagraphRow(first=opening, rest="", words=words)]
    rows = [draw_part_row(part) for part in parts]
    # The opening quotes cannot share a row drawn as written, such as a
    # heading, so they get a row of their own above it.
    if parts and parts[0].kind in (PartKind.PARAGRAPH, PartKind.ENTRY):
        rows[0] = draw_part_row(parts[0], opening)
    else:
        rows.insert(0, UnwrappedRow(fixed=opening))
    return rows + [UnwrappedRow(fixed=quotes)]


def draw_part_row(part: Part, first: str = "") -> ScreenRow:
    """Return the review window's row for one line of a docstring.

    Paragraphs and lines under a heading wrap. A line under a heading
    indents its wrapped lines two spaces. Other lines are drawn as
    written.

    Args:
        part: The docstring line to draw.
        first: The mark before a wrapped row's first line, such as the
            opening quotes.

    Returns:
        The row for the line.
    """
    if part.kind is PartKind.PARAGRAPH:
        return ParagraphRow(first=first, rest="", words=part.text)
    if part.kind is PartKind.ENTRY:
        return ParagraphRow(first=first, rest="  ", words=part.text)
    return UnwrappedRow(fixed=part.text)


def build_python_comment(new_wording: str, old: str) -> str:
    """Return new words laid out like the comment they replace.

    The owner and agents write new words without ``#`` marks or quotes.
    This puts them back before the words go in the file.

    Args:
        new_wording: The new words.
        old: The comment the new words replace, either ``#`` lines or
            one whole docstring. It may be suggested wording that is not
            in the file yet.

    Returns:
        The new comment, or an empty string to delete the comment.

    Raises:
        SourceError: The new words hold a carriage return or a NUL, or
            ``build_docstring`` refuses them.
    """
    # Python reads a carriage return as a line end and refuses a NUL.
    if "\r" in new_wording or "\0" in new_wording:
        raise SourceError("the text holds a carriage return or a NUL")
    if old.lstrip().startswith("#"):
        return build_comment_lines(new_wording, old, "#")
    return build_docstring(new_wording, old)


def build_docstring(new_wording: str, old: str) -> str:
    """Return new words laid out as a docstring like the old one.

    The new docstring keeps the old one's indent, quote mark and any
    ``r`` prefix. A docstring opened with one quote mark gets triple
    quotes when it needs more lines or holds that mark.

    Args:
        new_wording: The new words, without quotes.
        old: The docstring the new words replace. It may be suggested
            wording that is not in the file yet.

    Returns:
        The new docstring, or an empty string to delete it.

    Raises:
        SourceError: The old text is not one whole docstring, or the new
            words hold the closing quotes.
    """
    old_docstring = read_docstring_parts(old)[0]
    parts = read_plain_parts(new_wording)
    if not parts:
        return ""
    closing = old_docstring["quotes"][0] * 3
    if closing in join_parts_as_text(parts):
        raise SourceError(
            f"the text holds {closing}, which ends the docstring"
        )
    text = fit_docstring_on_one_line(parts, old_docstring)
    if text is None:
        text = wrap_docstring_lines(parts, old_docstring)
    return text


def read_docstring_parts(text: str) -> tuple[re.Match[str], list[Part]]:
    """Return a docstring's match and its lines as the owner edits them.

    Args:
        text: The docstring exactly as it is in the file.

    Returns:
        The match, which gives the indentation, prefix, quotes and body,
        and the docstring's lines without quotes or indentation.

    Raises:
        SourceError: The text is not one whole docstring.
    """
    docstring = WHOLE_DOCSTRING.fullmatch(text)
    if not (docstring and is_one_string_literal(text)):
        raise SourceError("is neither # lines nor one whole docstring")
    parts: list[Part] = []
    # Lines deeper than this follow an Example heading or a line ending in
    # two colons.
    example_indent: int | None = None
    entry_indent: int | None = None
    in_section = False
    for number, line in enumerate(docstring["body"].split("\n")):
        # The first line starts after the quotes, so it has no indent.
        if number:
            line = line.removeprefix(docstring["indent"])
        stripped = line.strip()
        depth = len(line) - len(line.lstrip(" \t"))
        if not stripped:
            in_section, entry_indent = False, None
            parts.append(Part(PartKind.BLANK, ""))
            continue
        if example_indent is not None and depth <= example_indent:
            example_indent = None
        if example_indent is not None or is_kept_line(line):
            parts.append(Part(PartKind.KEPT, line.rstrip()))
            continue
        if depth == 0 and SECTION_HEADING.fullmatch(stripped):
            parts.append(Part(PartKind.HEADING, stripped))
            in_section = stripped not in EXAMPLE_HEADINGS
            example_indent = None if in_section else 0
            continue
        words = " ".join(stripped.split())
        if in_section and (entry_indent is None or depth <= entry_indent):
            entry_indent = depth
            parts.append(Part(PartKind.ENTRY, words))
        elif parts and parts[-1].kind in (PartKind.ENTRY, PartKind.PARAGRAPH):
            parts[-1] = Part(parts[-1].kind, f"{parts[-1].text} {words}")
        else:
            kind = PartKind.ENTRY if in_section else PartKind.PARAGRAPH
            parts.append(Part(kind, words))
        if stripped.endswith("::"):
            example_indent = depth
    return docstring, trim_blank_parts(parts)


def is_one_string_literal(text: str) -> bool:
    """Return whether ``text`` is a single string literal and nothing else.

    ``"a" "b"`` is one string to Python but two literals, and the plain
    text of the two would change the string's value.
    """
    layout = {tokenize.NEWLINE, tokenize.NL, tokenize.INDENT}
    layout |= {tokenize.DEDENT, tokenize.ENDMARKER}
    try:
        tokens = tokenize.generate_tokens(io.StringIO(text.strip()).readline)
        kinds = [token.type for token in tokens if token.type not in layout]
    except (tokenize.TokenError, SyntaxError):
        return False
    return kinds == [tokenize.STRING]


def read_plain_parts(new_wording: str) -> list[Part]:
    """Return the lines of a docstring's new words, by kind.

    Lines that start with a tab or two spaces, doctest lines and lines
    ending in a backslash stay as written. Lines under a heading such as
    ``Args:`` stay separate up to the next blank line.

    Args:
        new_wording: The new words, without quotes, as the owner or an
            agent wrote them.

    Returns:
        The lines, with each paragraph joined into one.
    """
    parts: list[Part] = []
    in_section = False
    for line in new_wording.split("\n"):
        stripped = line.strip()
        if not stripped:
            in_section = False
            parts.append(Part(PartKind.BLANK, ""))
        elif line.startswith(("\t", "  ")) or is_kept_line(line):
            parts.append(Part(PartKind.KEPT, line.rstrip()))
        elif SECTION_HEADING.fullmatch(stripped):
            parts.append(Part(PartKind.HEADING, stripped))
            in_section = stripped not in EXAMPLE_HEADINGS
        elif in_section:
            parts.append(Part(PartKind.ENTRY, " ".join(stripped.split())))
        elif parts and parts[-1].kind is PartKind.PARAGRAPH:
            words = " ".join(stripped.split())
            parts[-1] = Part(PartKind.PARAGRAPH, f"{parts[-1].text} {words}")
        else:
            parts.append(Part(PartKind.PARAGRAPH, " ".join(stripped.split())))
    return trim_blank_parts(parts)


def is_kept_line(line: str) -> bool:
    """Return whether a docstring line must keep its line breaks.

    Joining the line after a backslash to it would leave a backslash
    before a space, which Python reads as an invalid escape.
    """
    stripped = line.strip()
    return stripped.startswith((">>>", "...")) or stripped.endswith("\\")


def trim_blank_parts(parts: list[Part]) -> list[Part]:
    """Return ``parts`` with no blank part first, last or twice in a row."""
    trimmed: list[Part] = []
    for part in parts:
        if part.kind is not PartKind.BLANK or (
            trimmed and trimmed[-1].kind is not PartKind.BLANK
        ):
            trimmed.append(part)
    if trimmed and trimmed[-1].kind is PartKind.BLANK:
        trimmed.pop()
    return trimmed


def join_parts_as_text(parts: list[Part]) -> str:
    """Return a docstring's lines as the text the owner edits.

    Args:
        parts: The docstring's lines, as ``read_docstring_parts`` or
            ``read_plain_parts`` returns them.

    Returns:
        Each part's text on a line of its own, or one line break when
        there are no parts.
    """
    return "".join(part.text + "\n" for part in parts) or "\n"


def fit_docstring_on_one_line(
    parts: list[Part], old: re.Match[str]
) -> str | None:
    """Return a one-paragraph docstring on one line, when it fits.

    A docstring opened with one quote mark gets triple quotes when it
    holds that mark.

    Args:
        parts: The new docstring's lines.
        old: The old docstring's regex match, which gives the indent,
            prefix and quotes.

    Returns:
        The docstring's line, or ``None`` when it needs more lines.
    """
    summary = parts[0].text
    quote = old["quotes"][0]
    # The closing quotes would join a last quote or be escaped by a "\".
    if (
        len(parts) != 1
        or parts[0].kind is not PartKind.PARAGRAPH
        or summary.endswith((quote, "\\"))
    ):
        return None
    quotes = quote * 3 if quote in summary else old["quotes"]
    line = f"{old['prefix']}{quotes}{summary}{quotes}"
    if len(line) > LINE_WIDTH - len(old["indent"].replace("\t", "    ")):
        return None
    return f"{old['indent']}{line}\n"


def wrap_docstring_lines(parts: list[Part], old: re.Match[str]) -> str:
    """Return a docstring on several lines, in triple quotes.

    Each line under a heading is indented four spaces, and its wrapped
    lines four more. The closing quotes get a line of their own.

    Args:
        parts: The new docstring's lines.
        old: The old docstring's match, which gives the indentation,
            prefix and quote character.

    Returns:
        The docstring as it would appear in the file.
    """
    indent, prefix = old["indent"], old["prefix"]
    quotes = old["quotes"][0] * 3
    width = LINE_WIDTH - len(indent.replace("\t", "    "))
    lines = [indent + prefix + quotes]
    for number, part in enumerate(parts):
        if part.kind is PartKind.PARAGRAPH:
            opening = len(prefix + quotes) if number == 0 else 0
            wrapped = wrap_first_and_rest(part.text, width - opening, width)
            if number == 0:
                lines[0] += wrapped.pop(0)
            lines += [indent + line for line in wrapped]
        elif part.kind is PartKind.ENTRY:
            wrapped = wrap_first_and_rest(part.text, width - 4, width - 8)
            lines.append(f"{indent}    {wrapped[0]}")
            lines += [f"{indent}        {line}" for line in wrapped[1:]]
        else:
            lines.append(indent + part.text if part.text else "")
    lines.append(indent + quotes)
    return "".join(line + "\n" for line in lines)


def wrap_first_and_rest(words: str, first: int, rest: int) -> list[str]:
    """Wrap a paragraph, with its own width for the first line.

    A word ending in a backslash stays on the line of the next word,
    because a backslash at a line end joins the lines and changes the
    text. A named escape such as ``\\N{EM DASH}`` stays whole, because a
    split one does not compile.

    Args:
        words: One paragraph, with single spaces between words.
        first: The first line's width.
        rest: The width of every later line.

    Returns:
        The wrapped lines.
    """
    # build_python_comment refuses wording with a NUL, so a NUL can
    # stand in for a space where the line must not break.
    words = words.replace("\\ ", "\\\0")
    words = NAMED_ESCAPE.sub(lambda name: name[0].replace(" ", "\0"), words)
    lines = wrap_words(words.split(), first)
    lines = lines[:1] + wrap_words(" ".join(lines[1:]).split(), rest)
    return [line.replace("\0", " ") for line in lines]


def describe_python_error(error: Exception) -> str:
    """Return why Python cannot read a file, with Python's version."""
    line = error.lineno if isinstance(error, SyntaxError) else None
    where = f" at line {line}" if line else ""
    version = f"{sys.version_info.major}.{sys.version_info.minor}"
    return f"cannot be read (Python {version}: {type(error).__name__}{where})"


def list_python_named_statements(
    body: list[ast.stmt], name_prefix: str = "", in_function: bool = False
) -> Iterator[tuple[ast.stmt, str, bool]]:
    """Yield each def, class and one-name assignment with its dotted name.

    The third value says whether it sits in a function.

    Args:
        name_prefix: The dotted name of the code around ``body``, and a dot.
    """
    for node in body:
        child_prefix, children_in_function = name_prefix, in_function
        target: ast.expr | None = None
        if isinstance(node, PYTHON_DEFINITIONS):
            yield node, name_prefix + node.name, in_function
            child_prefix = f"{name_prefix}{node.name}."
            children_in_function = in_function or not isinstance(
                node, ast.ClassDef
            )
        elif isinstance(node, ast.AnnAssign):
            target = node.target
        elif isinstance(node, ast.Assign) and len(node.targets) == 1:
            target = node.targets[0]
        if isinstance(target, ast.Name):
            yield node, name_prefix + target.id, in_function
        # An if, try, with or loop block adds nothing to a name.
        child_statements = []
        for child in ast.iter_child_nodes(node):
            if isinstance(child, ast.stmt):
                child_statements.append(child)
            elif isinstance(child, (ast.ExceptHandler, ast.match_case)):
                child_statements += child.body
        yield from list_python_named_statements(
            child_statements,
            child_prefix,
            children_in_function,
        )


def find_first_line(node: ast.stmt) -> int:
    """Return a statement's first line, counting its decorators."""
    if isinstance(node, PYTHON_DEFINITIONS) and node.decorator_list:
        return node.decorator_list[0].lineno
    return node.lineno


def find_docstring_span(
    owner: DocstringOwner, tokens: list[tokenize.TokenInfo]
) -> tuple[int, int, str] | None:
    """Return a docstring's first and last line, and why it is unapprovable.

    A docstring sharing a line with code cannot be approved, since its
    approved text would include that code. Other docstrings get ``""``.
    Returns ``None`` when ``owner`` has no docstring.
    """
    if ast.get_docstring(owner, clean=False) is None:
        return None
    statement = owner.body[0]
    first, last = statement.lineno, statement.end_lineno or statement.lineno
    # Tokens that only lay out code, which a docstring line may hold.
    layout = {tokenize.INDENT, tokenize.DEDENT, tokenize.NEWLINE, tokenize.NL}
    code_tokens = [
        token
        for token in tokens
        if first <= token.start[0] <= last and token.type not in layout
    ]
    if [token.type for token in code_tokens] == [tokenize.STRING]:
        return first, last, ""
    return first, last, "its docstring shares a line with other code"


def find_comment_runs(
    tokens: list[tokenize.TokenInfo], lines: list[str]
) -> list[tuple[int, int]]:
    """Return the first and last line of each run of whole-line comments."""
    runs: list[tuple[int, int]] = []
    for token in tokens:
        row, column = token.start
        if token.type != tokenize.COMMENT or lines[row - 1][:column].strip():
            continue
        if runs and runs[-1][1] == row - 1:
            runs[-1] = (runs[-1][0], row)
        else:
            runs.append((row, row))
    return runs


def join_python_prose(text: str) -> str:
    """Join a docstring or ``#`` comment into one line of prose.

    Quotes, section headings, doctests and examples are left out.
    """
    if text.lstrip().startswith("#"):
        return join_comment_prose(text, "#")
    words = []
    # Lines indented deeper than this belong to an example.
    example_indent: int | None = None
    for line in DOCSTRING_QUOTES.sub("", text).split("\n"):
        stripped = line.strip()
        indent = len(line) - len(line.lstrip())
        if not stripped or (
            example_indent is not None and indent > example_indent
        ):
            continue
        example_indent = None
        if stripped in ("Example:", "Examples:") or stripped.endswith("::"):
            example_indent = indent
        if not (
            SECTION_HEADING.fullmatch(stripped)
            or stripped.startswith((">>>", "..."))
        ):
            words.append(stripped)
    return " ".join(words)


def list_python_warnings(source: str, file: str) -> list[str]:
    """Return the warnings Python gives when it compiles ``source``.

    Compiling also finds errors that parsing alone misses, such as
    ``return`` outside a function.

    Raises:
        SourceError: Python cannot compile ``source``.
    """
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            compile(
                source,
                file,
                "exec",
                dont_inherit=True,
            )
        except (SyntaxError, ValueError) as error:
            raise SourceError(describe_python_error(error)) from error
    return [str(warning.message) for warning in caught]


def finish_python_rewrite(old: str, new: str, file: str) -> str:
    """Return ``new`` once it compiles with no new warning or escape.

    Warnings are compared by message, not by line, since the write moves
    lines, so a warning already in the code, such as an invalid escape in
    a regular expression, is left alone. ``compile`` ignores a coding
    line, so the encoding Python reads the file in is compared too.

    Raises:
        SourceError: ``new`` does not compile, gives a warning ``old``
            does not, such as an invalid escape, has a docstring escape
            ``old`` lacks, such as ``\\n``, or changes the encoding.
    """
    try:
        warned = Counter(list_python_warnings(new, file))
    except SourceError as error:
        cause = error.__cause__
        reason = str(error).replace("cannot be read", "would not compile")
        detail = f": {cause.msg}" if isinstance(cause, SyntaxError) else ""
        raise SourceError(f"{file}: the new text {reason}{detail}") from error
    warned -= Counter(list_python_warnings(old, file))
    escapes = count_docstring_escapes(new, file)
    escapes -= count_docstring_escapes(old, file)
    if read_encoding(new) != read_encoding(old):
        raise SourceError(
            f"{file}: the write changes the encoding Python reads it in, "
            f"from {read_encoding(old)} to {read_encoding(new)}; keep a "
            "comment naming an encoding off lines 1 and 2"
        )
    if warned:
        raise SourceError(
            f'{file}: the write adds Python\'s warning "{sorted(warned)[0]}"; '
            "double the backslash to keep it"
        )
    if escapes:
        name, _, escape = sorted(escapes)[0]
        # A backslash at a line's end joins the next line.
        shown = "a backslash at a line end" if escape == "\\\n" else escape
        raise SourceError(
            f"{file}: the write adds {shown} to the docstring of {name}, "
            "which Python reads as another character; double the backslash "
            "to keep it"
        )
    return new


def read_encoding(source: str) -> str:
    """Return the encoding Python reads ``source`` in, from its coding line.

    An encoding Python does not know is returned as the error saying so.
    """
    try:
        lines = io.BytesIO(source.encode()).readline
        return tokenize.detect_encoding(lines)[0]
    except SyntaxError as error:
        return f"none ({error})"


def count_docstring_escapes(
    source: str, file: str
) -> Counter[tuple[str, int, str]]:
    """Count each escape that changes a docstring's value, by docstring.

    A docstring is its name and which of the docstrings with that name
    it is, such as the second ``f`` when an ``if`` and its ``else`` both
    define one, so an escape cannot pass by moving from one docstring to
    another. Raw docstrings have none, and ``#`` comments are not strings.
    """
    escapes: Counter[tuple[str, int, str]] = Counter()
    seen: Counter[str] = Counter()
    for comment in parse_python_comments(source, file):
        docstring = WHOLE_DOCSTRING.fullmatch(comment.text)
        seen[comment.name] += 1
        if docstring and docstring["prefix"] not in ("r", "R"):
            escapes.update(
                (comment.name, seen[comment.name], pair[0])
                for pair in DOCSTRING_ESCAPE.finditer(docstring["body"])
                if pair[1] is not None
            )
    return escapes
