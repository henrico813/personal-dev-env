"""Read comments and their names out of a Python file."""

from __future__ import annotations

import ast
import enum
import io
import re
import sys
import tokenize
import warnings
from collections.abc import Iterator
from typing import NamedTuple

from ..records import Comment, SourceError
from .go import join_comment_prose
from .line_comments import (
    LINE_WIDTH,
    show_line_comment,
    wrap_words,
    write_line_comment,
)

PYTHON_DEFINITIONS = (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)

DocstringOwner = (
    ast.Module | ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef
)

PYTHON_GENERATED_MARKER = re.compile(
    r"^# (?:Code generated .* DO NOT EDIT\.$|@generated\b)", re.M
)

# A run of only tool directives, such as noqa, gets no name.
PYTHON_DIRECTIVE = re.compile(
    r"#\s*(?:noqa\b|(?:type|pragma|fmt|ruff|pylint|mypy|pyright|isort):)"
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
    part of the file.
    """
    is_hash_run = comment.text.lstrip().startswith("#")
    has_directive = is_hash_run and PYTHON_DIRECTIVE.search(comment.text)
    # Python reads a shebang or coding line only on line 1 or 2.
    file_lines = comment.text.splitlines()[: max(0, 3 - comment.line)]
    has_file_line = is_hash_run and any(
        PYTHON_SHEBANG_OR_ENCODING.match(line) for line in file_lines
    )
    return not (comment.unapprovable_reason or has_directive or has_file_line)


class PartKind(enum.Enum):
    """What one line of a docstring's plain text is."""

    PARAGRAPH = enum.auto()
    # A section heading, such as Args.
    HEADING = enum.auto()
    # One line of a section, such as an argument.
    ENTRY = enum.auto()
    # A line kept as written, such as an example.
    KEPT = enum.auto()
    BLANK = enum.auto()


class Part(NamedTuple):
    """One line of a docstring's plain text and what kind of line it is."""

    kind: PartKind
    text: str


def show_python_comment(text: str) -> tuple[str, list[dict[str, str]]]:
    """Return a comment's plain text and screen rows.

    A ``#`` comment is shown as a Go comment is, with ``#`` marks. A
    docstring's first row starts with its opening quotes, a section
    entry's later rows are indented two spaces, and the closing quotes
    are a row of their own.

    Raises:
        SourceError: ``text`` is neither ``#`` lines nor a whole docstring.
    """
    if text.lstrip().startswith("#"):
        return show_line_comment(text, "#")
    docstring, parts = read_docstring_parts(text)
    rows: list[dict[str, str]] = []
    for part in parts:
        if part.kind in (PartKind.PARAGRAPH, PartKind.ENTRY):
            rest = "  " if part.kind is PartKind.ENTRY else ""
            rows.append({"first": "", "rest": rest, "words": part.text})
        else:
            rows.append({"fixed": part.text})
    opening = docstring["prefix"] + docstring["quotes"]
    if rows and rows[0].get("first") == "":
        rows[0]["first"] = opening
    else:
        rows.insert(0, {"fixed": opening})
    rows.append({"fixed": docstring["quotes"]})
    return write_plain_parts(parts), rows


def write_python_comment(plain: str, old: str) -> str:
    """Return ``plain`` laid out as ``old``, a ``#`` comment or docstring.

    A docstring keeps the indentation, prefix and quote character of
    ``old``, and puts its summary on the opening quotes' line. Plain text
    with no words returns ``""``, so writing it deletes the comment.

    Raises:
        SourceError: ``plain`` holds the docstring's closing quotes, a
            carriage return or a NUL.
    """
    # Python reads a carriage return as a line end and refuses a NUL.
    if "\r" in plain or "\0" in plain:
        raise SourceError("the text holds a carriage return or a NUL")
    if old.lstrip().startswith("#"):
        return write_line_comment(plain, old, "#")
    docstring = read_docstring_parts(old)[0]
    return write_docstring(read_plain_parts(plain), docstring)


def read_docstring_parts(text: str) -> tuple[re.Match[str], list[Part]]:
    """Return a docstring's quotes match and the parts of its plain text.

    Raises:
        SourceError: ``text`` is not a whole docstring.
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


def read_plain_parts(plain: str) -> list[Part]:
    """Return the parts of plain text an owner or agent edited.

    Lines that start with a tab or two spaces, doctest lines and lines
    ending in a backslash are kept. After a heading such as ``Args:``,
    each line up to a blank line is one entry.
    """
    parts: list[Part] = []
    in_section = False
    for line in plain.split("\n"):
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


def write_plain_parts(parts: list[Part]) -> str:
    """Return the plain text of a docstring's parts, one line each."""
    return "".join(part.text + "\n" for part in parts) or "\n"


def write_docstring(parts: list[Part], docstring: re.Match[str]) -> str:
    """Return ``parts`` as a docstring with the quotes of ``docstring``.

    Paragraphs wrap at ``LINE_WIDTH``, and section entries are indented
    four spaces, with their later lines four more. A docstring with one
    short paragraph stays on one line. A docstring in single quotes
    becomes triple quoted when it needs a second line or holds its quote
    character.

    Raises:
        SourceError: The text holds the triple quotes it would be in.
    """
    if not parts:
        return ""
    indent, prefix = docstring["indent"], docstring["prefix"]
    quote = docstring["quotes"][0]
    text = write_plain_parts(parts)
    width = LINE_WIDTH - len(indent.replace("\t", "    "))
    summary = parts[0].text
    # The closing quotes would join a last quote or be escaped by a "\".
    if (
        len(parts) == 1
        and parts[0].kind is PartKind.PARAGRAPH
        and not summary.endswith((quote, "\\"))
    ):
        quotes = docstring["quotes"]
        if quote in summary:
            quotes = quote * 3
        line = f"{prefix}{quotes}{summary}{quotes}"
        if len(line) <= width and quote * 3 not in summary:
            return f"{indent}{line}\n"
    quotes = quote * 3
    if quotes in text:
        raise SourceError(f"the text holds {quotes}, which ends the docstring")
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
    """Wrap ``words`` with the first line at most ``first`` wide.

    A word ending in a backslash stays on a line with the word after it,
    since Python joins a docstring line that ends in a backslash to the
    next one, which would change the docstring's text. A named escape
    such as ``\\N{EM DASH}`` stays whole, as a split one does not compile.
    """
    # Plain text with a NUL is refused, so one stands for a space that
    # must not break.
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
