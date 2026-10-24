"""Read comments and their names out of a Go file."""

from __future__ import annotations

import enum
import re

from ..records import Comment, Function

# "func Send", "type Cart", "const Red", or a bare name such as a field.
DECLARED_NAME = re.compile(
    r"(?:func\s+|type\s+|const\s+|var\s+|package\s+)?(\w+)"
)

# "func (c *Cart) Total": group 1 is the receiver type, 2 the method.
METHOD_DECLARATION = re.compile(
    r"func\s+\(\s*(?:\w+\s+)?\*?\s*(\w+)[^)]*\)\s*(\w+)"
)

CONST_OR_VAR_BLOCK = re.compile(r"(const|var)\s*\($")

# Functions "go test" runs, such as "TestTotal" and "BenchmarkSort".
GO_TEST_NAME = re.compile(r"(?:Test|Benchmark|Fuzz|Example)(?![a-z])")

# A line for the Go tools, such as `//go:embed`, which is not prose.
GO_DIRECTIVE = re.compile(r"//(?:line |extern |export |[a-z0-9]+:[a-z0-9])")

GO_GENERATED_MARKER = re.compile(r"^// Code generated .* DO NOT EDIT\.$", re.M)

# Go comments and string literals, matched from left to right, so a
# backtick or "/*" inside one starts nothing.
GO_COMMENT_OR_STRING = re.compile(
    r"//[^\n]*|/\*.*?\*/|`[^`]*`"
    r"""|"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'""",
    re.S,
)


class Scope(enum.Enum):
    """Where a line sits in a Go file, which decides a comment's name."""

    # Behaves like DECLARATION with no outer name.
    TOP_LEVEL = enum.auto()
    # Inside a function body, where comments are unnamed.
    FUNCTION = enum.auto()
    # Inside "const (" or "var (", where names stand alone: "Red".
    BLOCK = enum.auto()
    # Inside a type, a var with a literal, or another declaration, where
    # names include the declaration's name: "Cart.Items".
    DECLARATION = enum.auto()


def parse_go_comments(source: str, file: str) -> list[Comment]:
    """Return each run of whole-line ``//`` comments in a Go file.

    Args:
        file: Its path relative to the repository, used in findings.

    Returns:
        The comments in file order, each with the name of the code
        below it.
    """
    lines = source.splitlines(keepends=True)
    inside_text = find_lines_inside_comment_or_string(source)
    comments: list[Comment] = []
    outer_name, scope, seen_package = "", Scope.TOP_LEVEL, False
    start: int | None = None

    # A blank line past the end closes a comment on the file's last lines.
    for number, line in enumerate([*lines, "\n"], 1):
        # A line inside a raw string or "/* */" may look like a comment
        # or declaration, such as a shell script's "esac" in a test.
        if number in inside_text:
            continue
        text = line.strip()
        if text.startswith("//"):
            start = number if start is None else start
            continue
        if start is not None:
            # The blank line past the end has no code to name.
            name = ""
            if number <= len(lines):
                name = name_code_below_comment(
                    lines, number, outer_name, scope
                )
            comment_lines = lines[start - 1 : number - 1]
            comments.append(
                Comment(
                    file=file,
                    line=start,
                    name=name,
                    text="".join(comment_lines),
                    prose=join_comment_prose("".join(comment_lines)),
                    header=not seen_package and not name,
                )
            )
            start = None
        # Top-level declarations start at column 0, so a letter there
        # opens one and a "}" there closes it.
        starts_declaration = line[:1].isalpha()
        if starts_declaration:
            keyword = text.split()[0]
            outer_name = parse_declared_name(text)
            seen_package = seen_package or keyword == "package"
            if CONST_OR_VAR_BLOCK.match(text):
                scope = Scope.BLOCK
            elif keyword == "func":
                scope = Scope.FUNCTION
            else:
                scope = Scope.DECLARATION
        # Only a block ends at a ")" in column 0; a multi-line signature
        # also ends there, but its body follows.
        elif line[:1] == "}" or (line[:1] == ")" and scope is Scope.BLOCK):
            outer_name, scope = "", Scope.TOP_LEVEL
    return comments


def list_go_functions(source: str, file: str) -> list[Function]:
    """Return each function and method declared in a Go file."""
    inside_text = find_lines_inside_comment_or_string(source)
    functions = []
    for number, line in enumerate(source.splitlines(), 1):
        if number in inside_text or not line.startswith("func "):
            continue
        text = line.strip()
        name = parse_declared_name(text)
        method = METHOD_DECLARATION.match(text)
        functions.append(
            Function(
                file=file,
                line=number,
                name=name,
                is_test=file.endswith("_test.go")
                and bool(GO_TEST_NAME.match(name.split(".")[-1])),
                is_getter=method is not None
                and text[method.end() :].startswith("()"),
            )
        )
    return functions


def find_lines_inside_comment_or_string(source: str) -> set[int]:
    """Return the 1-based lines starting inside a raw string or ``/* */``."""
    inside: set[int] = set()
    for match in GO_COMMENT_OR_STRING.finditer(source):
        newlines = match[0].count("\n")
        if newlines:
            first = source.count("\n", 0, match.start()) + 2
            inside.update(range(first, first + newlines))
    return inside


def parse_declared_name(text: str) -> str:
    """Name a declaration that starts at column 0.

    Args:
        text: The declaration line, stripped.

    Returns:
        ``Recv.Method`` for a method, otherwise the declared name.
    """
    method = METHOD_DECLARATION.match(text)
    if method:
        return f"{method[1]}.{method[2]}"
    declared = DECLARED_NAME.match(text)
    return declared[1] if declared else ""


def name_code_below_comment(
    lines: list[str], number: int, outer_name: str, scope: Scope
) -> str:
    """Name the code on 1-based line ``number``, just below a comment.

    Args:
        outer_name: Name of the declaration the line sits in, if any.

    Returns:
        The comment's name, or ``""`` when it has none.
    """
    line = lines[number - 1]
    text = line.strip()
    if not text:
        return ""
    if not line[0].isspace():
        if CONST_OR_VAR_BLOCK.match(text) and number < len(lines):
            # A block takes the name of its first entry, such as
            # "const Red", skipping blank and comment lines before it.
            keyword = text.split()[0]
            inside = (later.strip() for later in lines[number:])
            first_line = next(
                (t for t in inside if t and not t.startswith("//")), ""
            )
            first_entry = DECLARED_NAME.match(first_line)
            if not first_entry:
                return ""
            return f"{keyword} {first_entry[1]}"
        return parse_declared_name(text)
    declared = DECLARED_NAME.match(text)
    if scope is Scope.FUNCTION or not declared:
        return ""
    if scope is Scope.BLOCK:
        return declared[1]
    return f"{outer_name}.{declared[1]}"


def join_comment_prose(comment_text: str, marker: str = "//") -> str:
    """Join comment lines into one line of prose, leaving out code and lists.

    Go directive lines, such as ``//go:embed``, are left out too.

    Args:
        comment_text: The comment's lines, each starting with ``marker``.
    """
    words = []
    for line in comment_text.splitlines():
        if GO_DIRECTIVE.match(line.strip()):
            continue
        text = line.strip()[len(marker) :]
        # Go doc comments indent code blocks and lists.
        if text.strip() and not text.startswith(("\t", "  ")):
            words.append(text.strip())
    return " ".join(words)
