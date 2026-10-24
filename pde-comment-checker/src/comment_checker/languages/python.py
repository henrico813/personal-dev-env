"""Read comments and their names out of a Python file."""

from __future__ import annotations

import ast
import io
import re
import sys
import tokenize
import warnings
from collections.abc import Iterator

from ..records import Comment, SourceError
from .go import join_comment_prose
from .line_comments import show_line_comment, write_line_comment

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


def show_python_comment(text: str) -> tuple[str, list[dict[str, str]]]:
    """Return a ``#`` comment's plain text and screen rows, as Go's are.

    Raises:
        SourceError: ``text`` is not whole ``#`` lines, such as a docstring.
    """
    return show_line_comment(text, "#")


def write_python_comment(plain: str, old: str) -> str:
    """Return ``plain`` as ``#`` lines, laid out as Go comments are."""
    return write_line_comment(plain, old, "#")


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
