"""Report Python code that breaks one of the skill's Python rules.

Example:
    python3 check_python_rules.py --repo . shop/cart.py
    python3 check_python_rules.py --repo . --base main shop/*.py

Prints one line per finding, then quotes each broken rule's sentence
from the skill. The rules are:

- ``verb-first``: a function or method name does not start with a
  verb the checker knows. Tests, fixtures and properties are left out.
  This is only reported, since the verb list cannot hold every verb.
- ``seven-words``: a function or method name has more than seven words.
- ``docstring``: a function, method or class has no docstring.
- ``summary-line``: a docstring's summary does not fit on its first
  line.
- ``test-why``: a test's docstring has no paragraph after the summary.
- ``starts-with-name``: a docstring starts with the name it documents.
- ``section-heading``: a docstring has a heading in place of a plain
  paragraph, such as ``Why it matters:``.
- ``comment-length``: a ``#`` comment has more than two sentences.
- ``comment-colon``: a ``#`` comment has a colon outside code, a URL
  or a time such as ``4:00``.

Names a language or framework requires, such as ``__init__``,
``setUp``, ``do_GET`` or an ``@override`` method, are left out, and
dunders other than ``__init__`` need no docstring.

With ``--base``, a name is checked only when it is new or renamed since
that commit, a docstring only when a line of its function or class was
added or changed, and a comment only when one of its lines was. Lines
of nested functions and classes count for those, not the outer one. A
file renamed since that commit is compared with its old path, and a
path deleted since then is skipped.

Exit codes:
    0: Nothing found, or only ``verb-first`` findings.
    1: At least one finding other than ``verb-first``.
    2: A path is a folder, unreadable or not valid Python, no Python
        file was given, ``--base`` is not a commit, or ``git`` is
        missing.
"""

from __future__ import annotations

import argparse
import ast
import io
import re
import subprocess
import sys
import tokenize
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

# Each rule and the code-documentation sentence it checks, word for word.
RULES = {
    "verb-first": (
        "Start every function and method name with a verb, except a "
        "test, a fixture or a property."
    ),
    "seven-words": "Every function name must contain at most seven words.",
    "docstring": (
        "Give every function, method and class a docstring, tests and "
        "fixtures included."
    ),
    "summary-line": (
        "Start each docstring with a one-line summary that fits on its "
        "first line."
    ),
    "test-why": (
        "Follow a test's summary with a paragraph on why the test matters."
    ),
    "starts-with-name": (
        "Do not start a doc comment with the name it documents"
    ),
    "section-heading": (
        "Never invent a docstring section, such as `Why it matters:`; "
        "write a plain paragraph instead."
    ),
    "comment-length": "Keep each `#` comment to at most two sentences.",
    "comment-colon": (
        "Use no colon in a `#` comment, except inside code or a URL."
    ),
}
# Headings that stand in for the plain paragraph a docstring should have.
INVENTED_HEADINGS = frozenset(
    {
        "Background",
        "Context",
        "Purpose",
        "Rationale",
        "Why",
        "Why it matters",
        "Why this matters",
    }
)
# Verbs that start function names. A word with "re" or "un" in front of
# one of these counts too, such as "rebuild".
VERBS = frozenset(
    """
    accept add allow answer append apply approve archive arm ask assert
    assign attach audit build bump calculate call cancel capture change
    check choose clean clear clone close collect combine commit compare
    compile complete compute configure confirm connect consume convert copy
    count create cut decide decode delete deliver describe detect disable
    discard dispatch download drop dump edit emit enable encode ensure
    enter escape evaluate exit expand expect explain export extract fail
    fetch fill filter find finish fix flush format freeze generate get give
    group guard handle hash help hide import include increment index init
    insert inspect install is iter join keep kill label launch list load lock
    log look looks make map mark match measure merge migrate mount move name
    normalize notify open order pack pad park parse pass patch pause pick pin
    plan poll pop post prepare preview print process produce prompt prune
    publish pull push put queue quote raise read receive record reduce
    refresh refund refuse register reject release reload remove rename
    render reorder repeat replace reply report request require reserve
    reset resolve restore resume retry return reverse review rewrite round
    run sanitize save scan schedule score scout search seed select send serve
    set setup show shrink skip sleep sort split start stash stop store strip
    submit subscribe summarize swap switch sync take teardown tell test
    trace track transform transition translate trim truncate try undo
    unlock unpack update upgrade upload use validate verify view visit wait
    walk warn watch wrap write yield
    """.split()
)
# Words a name starts with to ask a question or convert, such as
# "is_empty" or "to_json".
QUESTION_OR_CONVERSION_WORDS = frozenset(
    {"as", "can", "from", "has", "into", "should", "to", "was"}
)
# Names that Python, unittest, http.server and pytest require, and main.
FRAMEWORK_NAME = re.compile(
    r"__\w+__|main|setUp\w*|tearDown\w*|asyncSetUp|asyncTearDown"
    r"|do_[A-Z]+|pytest_\w+"
)
PROPERTY_DECORATORS = frozenset(
    {"cached_property", "deleter", "getter", "property", "setter"}
)
# Comments for tools, which are not prose for people.
TOOL_DIRECTIVE = re.compile(
    r"#\s*(?:!|noqa\b|-\*-|(?:type|pragma|fmt|ruff|pylint|mypy|pyright"
    r"|isort|coding):)"
)
# A tool comment such as noqa that follows prose in the same comment.
TRAILING_DIRECTIVE = re.compile(r"\s*#\s*(?:noqa\b|type:|pragma:).*")
CODE_OR_URL = re.compile(r"`[^`]*`|\w+://\S+")
# Abbreviations whose period does not end a sentence.
ABBREVIATIONS = re.compile(r"\b(?:e\.g|i\.e|etc)\.")
WORD = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+")
DEFINITIONS = (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)
Definition = ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef


@dataclass(frozen=True)
class Finding:
    """One rule break, at the line it starts on."""

    file: str
    line: int
    name: str
    rule: str
    message: str


def split_name_words(name: str) -> list[str]:
    """Split ``parse_HTTPConfig`` into ``["parse", "HTTP", "Config"]``."""
    return [w for part in name.split("_") for w in WORD.findall(part)]


def is_verb(word: str) -> bool:
    """Return whether a name's first word is a known verb or question word."""
    word = word.lower()
    if word in VERBS or word in QUESTION_OR_CONVERSION_WORDS:
        return True
    return word[:2] in {"re", "un"} and word[2:] in VERBS


def list_decorator_names(
    node: ast.FunctionDef | ast.AsyncFunctionDef,
) -> set[str]:
    """Return the last part of each decorator, such as ``fixture``."""
    names = set()
    for decorator in node.decorator_list:
        if isinstance(decorator, ast.Call):
            decorator = decorator.func
        if isinstance(decorator, ast.Attribute):
            names.add(decorator.attr)
        elif isinstance(decorator, ast.Name):
            names.add(decorator.id)
    return names


def check_function_name(
    node: ast.FunctionDef | ast.AsyncFunctionDef,
) -> list[tuple[str, str]]:
    """Return the ``(rule, message)`` pairs a function's name breaks."""
    if FRAMEWORK_NAME.fullmatch(node.name):
        return []
    decorators = list_decorator_names(node)
    if "override" in decorators:
        return []
    words = split_name_words(node.name)
    found = []
    if len(words) > 7:
        found.append(("seven-words", f"{len(words)} words"))
    is_test = node.name.startswith("test")
    exempt = is_test or decorators & ({"fixture"} | PROPERTY_DECORATORS)
    if words and not exempt and not is_verb(words[0]):
        found.append(
            (
                "verb-first",
                f'"{words[0]}" is not in the checker\'s verbs (report only)',
            )
        )
    return found


def check_docstring(node: Definition) -> list[tuple[str, str]]:
    """Return the ``(rule, message)`` pairs a docstring breaks."""
    docstring = ast.get_docstring(node, clean=False)
    is_dunder = node.name.startswith("__") and node.name.endswith("__")
    if docstring is None and is_dunder and node.name != "__init__":
        return []
    if docstring is None:
        return [("docstring", "no docstring")]
    lines = docstring.splitlines()
    found = []
    if not lines or not lines[0].strip():
        found.append(("summary-line", "first line is empty"))
    elif len(lines) > 1 and lines[1].strip():
        found.append(("summary-line", "summary runs onto a second line"))
    paragraphs = re.split(r"\n\s*\n", docstring.strip())
    is_test = not isinstance(node, ast.ClassDef) and node.name.startswith(
        "test"
    )
    if is_test and len(paragraphs) < 2:
        found.append(("test-why", "no paragraph after the summary"))
    first_word = docstring.split(maxsplit=1)[:1]
    if first_word and first_word[0].strip("`'\"(),.:") in {
        node.name,
        f"{node.name}()",
    }:
        found.append(("starts-with-name", f'starts with "{node.name}"'))
    for line in lines[1:]:
        heading = line.strip().partition(":")
        if heading[1] and heading[0] in INVENTED_HEADINGS:
            found.append(("section-heading", f'"{heading[0]}:"'))
    return found


def iter_definitions(
    node: ast.AST, prefix: str = ""
) -> Iterator[tuple[Definition, str]]:
    """Yield each definition inside ``node``, named like ``Cart.total``."""
    for child in ast.iter_child_nodes(node):
        if isinstance(child, DEFINITIONS):
            name = f"{prefix}{child.name}"
            yield child, name
            yield from iter_definitions(child, f"{name}.")
        else:
            yield from iter_definitions(child, prefix)


def list_own_lines(node: Definition) -> set[int]:
    """Return a definition's lines, leaving out nested definitions.

    Decorators count, so adding one marks the definition changed.
    """
    lines = set(range(find_first_line(node), (node.end_lineno or 0) + 1))
    for child, _ in iter_definitions(node):
        lines -= set(
            range(find_first_line(child), (child.end_lineno or 0) + 1)
        )
    return lines


def find_first_line(node: Definition) -> int:
    """Return the line of a definition's first decorator, or its ``def``."""
    return min([node.lineno, *(d.lineno for d in node.decorator_list)])


def check_definitions(
    tree: ast.Module,
    file: str,
    changed: set[int] | None,
    old_names: set[str],
) -> list[Finding]:
    """Check each new function name and each changed definition.

    Args:
        changed: Lines added or changed since ``--base``; ``None``
            means every line.
        old_names: Names, such as ``Cart.total``, the file had at
            ``--base``, whose name rules are not checked.
    """
    findings = []
    for node, name in iter_definitions(tree):
        broken = []
        if not isinstance(node, ast.ClassDef) and (
            changed is None or name not in old_names
        ):
            broken += check_function_name(node)
        if changed is None or list_own_lines(node) & changed:
            broken += check_docstring(node)
        findings += [
            Finding(file, node.lineno, name, rule, message)
            for rule, message in broken
        ]
    return findings


def check_comments(
    source: str, file: str, changed: set[int] | None
) -> list[Finding]:
    """Check each ``#`` comment with a line that changed.

    Whole-line comments on consecutive lines are one comment.

    Args:
        changed: Lines added or changed since ``--base``; ``None``
            means every line.
    """
    comments: list[tuple[list[int], list[str]]] = []
    tokens = tokenize.generate_tokens(io.StringIO(source).readline)
    previous_line = 0
    for token in tokens:
        if token.type != tokenize.COMMENT:
            continue
        line = token.start[0]
        if TOOL_DIRECTIVE.match(token.string):
            continue
        whole_line = not token.line[: token.start[1]].strip()
        text = TRAILING_DIRECTIVE.sub("", token.string[1:]).strip()
        if whole_line and comments and previous_line == line - 1:
            comments[-1][0].append(line)
            comments[-1][1].append(text)
        else:
            comments.append(([line], [text]))
        previous_line = line if whole_line else 0
    findings = []
    for lines, texts in comments:
        if changed is not None and not set(lines) & changed:
            continue
        prose = CODE_OR_URL.sub("code", " ".join(texts))
        prose = ABBREVIATIONS.sub("abbreviation", prose)
        sentences = [s for s in re.split(r"(?<=[.!?])\s+", prose) if s]
        if len(sentences) > 2:
            findings.append(
                Finding(
                    file,
                    lines[0],
                    "-",
                    "comment-length",
                    f"{len(sentences)} sentences",
                )
            )
        if re.search(r":(?!\d)", prose):
            findings.append(
                Finding(file, lines[0], "-", "comment-colon", "has a colon")
            )
    return findings


def is_commit(repo: Path, base: str) -> bool:
    """Return whether ``base`` names a commit in ``repo``."""
    known = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "rev-parse",
            "--verify",
            "--quiet",
            f"{base}^{{commit}}",
        ],
        capture_output=True,
    )
    return known.returncode == 0


def is_in_commit(repo: Path, base: str, file: str) -> bool:
    """Return whether ``file``, relative to ``repo``, exists at ``base``."""
    # "./" makes Git read the path from --repo, not the Git root.
    at_base = subprocess.run(
        ["git", "-C", str(repo), "cat-file", "-e", f"{base}:./{file}"],
        capture_output=True,
    )
    return at_base.returncode == 0


def list_base_names(repo: Path, base: str, old_file: str) -> set[str]:
    """Return the definition names ``old_file`` had at ``base``.

    Returns:
        Names such as ``Cart.total``, or an empty set when the file
        is missing at ``base`` or is not valid Python there.
    """
    shown = subprocess.run(
        ["git", "-C", str(repo), "show", f"{base}:./{old_file}"],
        capture_output=True,
    )
    if shown.returncode != 0:
        return set()
    try:
        # Bytes make ast read the encoding the way Python does.
        tree = ast.parse(shown.stdout)
    except (SyntaxError, ValueError):
        return set()
    return {name for _, name in iter_definitions(tree)}


def find_renamed_files(repo: Path, base: str) -> dict[str, str]:
    """Return the old path of each file renamed since ``base``.

    Returns:
        Old paths by new path, both relative to ``repo``.
    """
    status = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "diff",
            "--name-status",
            "-M",
            "--relative",
            base,
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    renamed: dict[str, str] = {}
    for line in status.splitlines():
        fields = line.split("\t")
        if fields[0].startswith("R") and len(fields) == 3:
            renamed[fields[2]] = fields[1]
    return renamed


def find_changed_lines(
    repo: Path, base: str, file: str, old_file: str
) -> set[int] | None:
    """Return the lines of ``file`` added or changed since ``base``.

    Args:
        old_file: The path ``file`` had at ``base``, which differs
            when it was renamed.

    Returns:
        The 1-based line numbers, or ``None`` when ``file`` is new
        since ``base``, so every line counts.
    """
    if not is_in_commit(repo, base, old_file):
        return None
    diff = subprocess.run(
        [
            "git",
            "-C",
            str(repo),
            "diff",
            "-U0",
            "-M",
            "--no-color",
            "--no-ext-diff",
            base,
            "--",
            old_file,
            file,
        ],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    changed: set[int] = set()
    for start, count in re.findall(
        r"^@@ \S+ \+(\d+)(?:,(\d+))? @@", diff, re.M
    ):
        first = int(start)
        changed.update(range(first, first + int(count or 1)))
    return changed


def main(argv: list[str] | None = None) -> int:
    """Check the given Python files and print one line per finding.

    Args:
        argv: Command-line arguments; ``sys.argv`` when ``None``.

    Returns:
        The exit code described in the module docstring.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument(
        "--base", help="commit to compare with; older code is not checked"
    )
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args(argv)
    repo = args.repo.resolve()
    renamed: dict[str, str] = {}
    if args.base:
        try:
            known = is_commit(repo, args.base)
        except FileNotFoundError:
            print("--base needs git, which was not found", file=sys.stderr)
            return 2
        if not known:
            print(
                f"--base {args.base}: not a commit in {repo}", file=sys.stderr
            )
            return 2
        renamed = find_renamed_files(repo, args.base)
    findings: list[Finding] = []
    checked = 0
    for path in args.paths:
        if path.is_dir():
            print(f"{path}: a folder; pass its Python files", file=sys.stderr)
            return 2
        if path.suffix != ".py":
            print(f"{path}: skipped; not a Python file", file=sys.stderr)
            continue
        try:
            file = path.resolve().relative_to(repo).as_posix()
        except ValueError as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        if (
            args.base
            and not path.exists()
            and is_in_commit(repo, args.base, file)
        ):
            print(f"{path}: skipped; deleted since --base", file=sys.stderr)
            continue
        try:
            # tokenize.open reads a BOM or coding line as Python does.
            with tokenize.open(path) as stream:
                source = stream.read()
            tree = ast.parse(source, filename=str(path))
        except (OSError, ValueError, SyntaxError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        changed = None
        old_names: set[str] = set()
        if args.base:
            old_file = renamed.get(file, file)
            changed = find_changed_lines(repo, args.base, file, old_file)
            old_names = list_base_names(repo, args.base, old_file)
        findings += check_definitions(tree, file, changed, old_names)
        findings += check_comments(source, file, changed)
        checked += 1
    if not checked:
        print("no Python file to check", file=sys.stderr)
        return 2
    broken_rules: list[str] = []
    for finding in sorted(findings, key=lambda f: (f.file, f.line)):
        print(
            f"{finding.file}:{finding.line} {finding.name}: "
            f"{finding.rule}: {finding.message}"
        )
        if finding.rule not in broken_rules:
            broken_rules.append(finding.rule)
    for rule in broken_rules:
        print(f'{rule} rule: "{RULES[rule]}"')
    return 1 if any(f.rule != "verb-first" for f in findings) else 0
