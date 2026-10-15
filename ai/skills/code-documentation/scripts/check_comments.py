#!/usr/bin/env python3
"""Report Go comments that break a code-documentation rule.

Example:
    python3 check_comments.py --repo . internal/shop/*.go
    python3 check_comments.py --repo . --base main internal/shop/*.go

Prints one line per finding, then quotes each broken rule's sentence
from the skill. The rules are:

- ``filler``: a sentence about the code uses one of the skill's vague
  filler phrases, such as "facilitates".
- ``restated-name``: a comment's first sentence repeats the name of the
  code below it. This never fails the run.

With ``--base``, sentences present at that commit never fail the run,
and ``restated-name`` findings on them are left out. Filler in a
license header above ``package`` is only reported. Generated files are
skipped.

Exit codes:
    0: Nothing failed.
    1: With ``--base``, a sentence not present at that commit has filler.
    2: A path is a folder or unreadable, no Go file was given, or
        ``--base`` is not a commit.
"""

from __future__ import annotations

import argparse
import enum
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import NamedTuple

# Each rule and the code-documentation sentence it checks, word for word.
RULES = {
    "restated-name": (
        "Prose derived directly from a name, signature, single statement, "
        "or a test's name, inputs, and assertions is a restatement and "
        "must not be added."
    ),
    "filler": "Avoid vague filler such as:",
}
# The skill's filler examples and their other word forms. They count only
# when the code is the sentence's subject, since "The caller is
# responsible for closing it" is a fact the skill asks for.
FILLER = (
    "facilitate",
    "facilitated",
    "facilitates",
    "facilitating",
    "is responsible for",
    "are responsible for",
    "provides a mechanism for",
    "provides a robust mechanism",
    "seamless",
    "seamlessly",
)
# "func Send", "type Cart", "const Red", or a bare name such as a field.
DECLARED_NAME = re.compile(
    r"(?:func\s+|type\s+|const\s+|var\s+|package\s+)?(\w+)"
)
# "func (c *Cart) Total": group 1 is the receiver type, 2 the method.
METHOD_DECLARATION = re.compile(
    r"func\s+\(\s*(?:\w+\s+)?\*?\s*(\w+)[^)]*\)\s*(\w+)"
)
CONST_OR_VAR_BLOCK = re.compile(r"(const|var)\s*\($")
GENERATED_FILE_MARKER = re.compile(
    r"^// Code generated .* DO NOT EDIT\.$", re.M
)


@dataclass(frozen=True)
class Comment:
    """One comment and the name of the code it sits on.

    Names look like ``Send``, ``Cart.Total`` for a method, ``Cart.Items``
    for a struct field or map entry, and ``const Red`` for a comment
    above a ``const (`` block. A comment inside a function body, with a
    blank line after it, or above an empty block has the name ``""``.
    ``header`` is true for a comment above the ``package`` line with a
    blank line after it, where license text usually sits. A package doc
    comment touches the ``package`` line, so it is named, not a header.
    """

    file: str
    line: int
    name: str
    prose: str
    header: bool = False


@dataclass(frozen=True)
class Finding:
    """A problem in one comment, under one of the ``RULES``.

    ``note`` says why the finding is only reported, or is empty. Only a
    ``filler`` finding with an empty note, in a run with ``--base``,
    fails the run.
    """

    comment: Comment
    rule: str
    message: str
    note: str = ""


class RuleMatch(NamedTuple):
    """A rule a comment breaks, before the ``--base`` policy applies."""

    rule: str
    message: str
    sentences: list[str]


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
    comments: list[Comment] = []
    outer_name, scope, seen_package = "", Scope.TOP_LEVEL, False
    start: int | None = None

    # A blank line past the end closes a comment on the file's last lines.
    for number, line in enumerate([*lines, "\n"], 1):
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
            comments.append(
                Comment(
                    file=file,
                    line=start,
                    name=name,
                    prose=join_comment_prose(lines[start - 1 : number - 1]),
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
            # A block takes the name of its first entry: "const Red".
            keyword = text.split()[0]
            next_line = lines[number]
            first_entry = DECLARED_NAME.match(next_line.strip())
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


def join_comment_prose(comment_lines: list[str]) -> str:
    """Join comment lines into one line of prose, leaving out code and lists.

    Args:
        comment_lines: The comment's lines, with their ``//`` markers.
    """
    words = []
    for line in comment_lines:
        text = line.strip()[2:]
        # Go doc comments indent code blocks and lists.
        if text.strip() and not text.startswith(("\t", "  ")):
            words.append(text.strip())
    return " ".join(words)


def split_sentences(prose: str) -> list[str]:
    """Split prose after ``.``, ``!`` or ``?`` followed by a space."""
    return [s for s in re.split(r"(?<=[.!?])\s+", prose) if s]


def find_restated_name(sentences: list[str], name: str) -> list[RuleMatch]:
    """Match a first sentence that contains the name of the code below."""
    if (
        name
        and sentences
        and re.search(rf"\b{re.escape(name)}\b", sentences[0])
    ):
        message = f'first sentence repeats the name "{name}"'
        return [RuleMatch("restated-name", message, sentences[:1])]
    return []


def find_filler(sentences: list[str], name: str) -> list[RuleMatch]:
    """Match each ``FILLER`` phrase in sentences whose subject is the code.

    A sentence is about the code when it starts with "This", the code's
    name, or "Package" and the name.
    """
    subject = r"This\b"
    if name:
        subject = rf"(?:This|{re.escape(name)}|Package {re.escape(name)})\b"
    about_code = [s for s in sentences if re.match(subject, s)]
    matches = []
    for phrase in FILLER:
        whole_words = rf"\b{re.escape(phrase)}\b"
        found = [s for s in about_code if re.search(whole_words, s.lower())]
        if found:
            matches.append(RuleMatch("filler", f'"{phrase}"', found))
    return matches


def find_rule_violations(
    comments: list[Comment], base_sentences: set[str] | None
) -> list[Finding]:
    """Check every comment against the ``RULES``.

    Args:
        comments: Comments from the checked files.
        base_sentences: The sentences of every Go comment under the
            repository at ``--base``, or ``None`` without it.

    Returns:
        Findings in file order.
    """
    findings = []
    for comment in comments:
        sentences = split_sentences(comment.prose)
        # Prose names code without its qualifier: "Cart.Total" is written
        # "Total", and "const Red" is written "Red".
        name = re.split(r"[. ]", comment.name)[-1]
        matches = find_restated_name(sentences, name)
        matches += find_filler(sentences, name)
        for match in matches:
            # A sentence present at --base may be a person's words, so it
            # never fails the run, and the agent is not asked to cut it.
            present_at_base = base_sentences is not None and all(
                s in base_sentences for s in match.sentences
            )
            if present_at_base and match.rule == "restated-name":
                continue
            note = ""
            if comment.header:
                note = "header: report only"
            elif present_at_base:
                note = "existing: report only"
            findings.append(Finding(comment, match.rule, match.message, note))
    return findings


def read_comments_at_commit(repo: Path, rev: str) -> list[Comment]:
    """Read every Go comment under ``repo`` at ``rev``.

    Every Go file counts, not only the checked ones, so a person's
    sentence in a moved or renamed file is still theirs.

    Args:
        repo: The folder given as ``--repo``, inside a Git repository.

    Raises:
        ValueError: ``rev`` is not a commit in ``repo``.
    """
    git = ["git", "-C", str(repo)]
    known = subprocess.run(
        [*git, "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"],
        capture_output=True,
    )
    if known.returncode != 0:
        raise ValueError(f"--base {rev}: not a commit in {repo}")
    listed = subprocess.run(
        [*git, "ls-tree", "-r", "--name-only", rev, "--", "."],
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    found = []
    for file in listed:
        if not file.endswith(".go"):
            continue
        # "./" makes Git read the path from --repo, not the Git root.
        shown = subprocess.run(
            [*git, "show", f"{rev}:./{file}"], capture_output=True
        ).stdout
        # An old file need not be UTF-8; only its comments matter.
        source = shown.decode("utf-8", errors="replace")
        found += parse_go_comments(source, file)
    return found


def main(argv: list[str] | None = None) -> int:
    """Check the given Go files and print one line per finding.

    Args:
        argv: Command-line arguments; ``sys.argv`` when ``None``.

    Returns:
        The exit code described in the module docstring.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument(
        "--base", help="commit to compare with; its sentences never fail"
    )
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args(argv)
    repo = args.repo.resolve()
    checked_files: set[str] = set()
    comments: list[Comment] = []
    for path in args.paths:
        if path.is_dir():
            print(f"{path}: a folder; pass its Go files", file=sys.stderr)
            return 2
        if path.suffix != ".go":
            print(f"{path}: skipped; not a Go file", file=sys.stderr)
            continue
        try:
            file = path.resolve().relative_to(repo).as_posix()
            # Skip a file given twice, such as "a.go" and "./a.go".
            if file in checked_files:
                continue
            source = path.read_text(encoding="utf-8")
        except (OSError, ValueError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        if GENERATED_FILE_MARKER.search(source):
            print(f"{path}: skipped; generated", file=sys.stderr)
            continue
        comments += parse_go_comments(source, file)
        checked_files.add(file)
    if not checked_files:
        print("no Go file to check", file=sys.stderr)
        return 2

    base_sentences = None
    if args.base:
        try:
            base_comments = read_comments_at_commit(repo, args.base)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        base_sentences = {
            sentence
            for comment in base_comments
            for sentence in split_sentences(comment.prose)
        }

    findings = find_rule_violations(comments, base_sentences)
    broken_rules: list[str] = []
    for finding in findings:
        comment = finding.comment
        place = f"{comment.file}:{comment.line} {comment.name or '-'}"
        note = f" ({finding.note})" if finding.note else ""
        print(f"{place}: {finding.rule}: {finding.message}{note}")
        if finding.rule not in broken_rules:
            broken_rules.append(finding.rule)
    for rule in broken_rules:
        print(f'{rule} rule: "{RULES[rule]}"')
    failed = base_sentences is not None and any(
        finding.rule == "filler" and not finding.note for finding in findings
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
