#!/usr/bin/env python3
"""Flag Go comments that break a code-documentation rule.

Example:
    python3 check_comments.py --repo . internal/shop/*.go
    python3 check_comments.py --repo . --base main internal/shop/*.go

Reads whole-line Go ``//`` comments and names the code each one sits on.
Each finding names a rule, and the output quotes that rule's sentence
from the skill. With ``--base``, it exits 1 when a sentence no Go file
had at that commit has filler, and leaves out questions about older
sentences. Exits 2 on a folder, an unreadable file, or no Go file to
check.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

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
DECLARATION = re.compile(
    r"(?:func\s+|type\s+|const\s+|var\s+|package\s+)?(\w+)"
)
METHOD = re.compile(r"func\s+\(\s*(?:\w+\s+)?\*?\s*(\w+)[^)]*\)\s*(\w+)")
BLOCK = re.compile(r"(const|var)\s*\($")
GENERATED = re.compile(r"^// Code generated .* DO NOT EDIT\.$", re.M)


@dataclass(frozen=True)
class Comment:
    """One comment and the name of the code it sits on.

    Names look like ``Send``, ``Cart.Total`` for a method, ``Cart.Items``
    for a struct field or map entry, and ``const Red`` for a comment
    above a ``const (`` block. A comment inside a function body, or with
    a blank line after it, has the name ``""``. ``header`` is true for a
    comment above the ``package`` line with a blank line after it, where
    license text usually sits. A package doc comment touches the
    ``package`` line, so it is named, not a header.
    """

    file: str
    line: int
    name: str
    prose: str
    header: bool = False


@dataclass(frozen=True)
class Finding:
    """A problem in one comment, under one of the ``RULES``.

    ``note`` is empty when the finding can fail the run, and otherwise
    says why it is only reported.
    """

    comment: Comment
    rule: str
    message: str
    note: str = ""


def go_comments(source: str, file: str) -> list[Comment]:
    """Return each run of whole-line ``//`` comments in a Go file.

    Args:
        source: The Go file's text.
        file: Its path relative to the repository, used in findings.

    Returns:
        The comments in file order, each with the name of the code
        below it.
    """
    lines = source.splitlines(keepends=True)
    comments: list[Comment] = []
    outer, kind, seen_package = "", "", False
    start: int | None = None

    for number, line in enumerate(lines + ["\n"], 1):
        text = line.strip()
        if text.startswith("//"):
            start = number if start is None else start
            continue
        if start is not None:
            run = lines[start - 1 : number - 1]
            end = number > len(lines)
            name = "" if end else go_name(lines, number, outer, kind)
            comments.append(
                Comment(
                    file,
                    start,
                    name,
                    go_prose(run),
                    not seen_package and not name,
                )
            )
            start = None
        if line[:1].isalpha():
            outer, kind = declaration(text), text.split()[0]
            seen_package = seen_package or kind == "package"
            if BLOCK.match(text):
                kind = "block"
        elif line[:1] == "}" or (line[:1] == ")" and kind == "block"):
            # A ")" at column 0 also closes a multi-line signature, which
            # is still inside the function.
            outer, kind = "", ""
    return comments


def declaration(text: str) -> str:
    """Name a declaration that starts at column 0.

    Args:
        text: The declaration line, stripped.

    Returns:
        ``Recv.Method`` for a method, otherwise the declared name.
    """
    method = METHOD.match(text)
    if method:
        return f"{method[1]}.{method[2]}"
    word = DECLARATION.match(text)
    return word[1] if word else ""


def go_name(lines: list[str], number: int, outer: str, kind: str) -> str:
    """Name the code on 1-based line ``number``, just below a comment.

    Args:
        lines: Every line of the file.
        number: The line below the comment, counted from 1.
        outer: Name of the declaration the line sits in, if any.
        kind: ``block`` inside ``const (`` or ``var (``, ``func`` inside
            a function, otherwise the declaration's keyword.

    Returns:
        The comment's name, or ``""`` when it has none.
    """
    line = lines[number - 1]
    text = line.strip()
    if not text:
        return ""
    if not line[0].isspace():
        if BLOCK.match(text) and number < len(lines):
            first = DECLARATION.match(lines[number].strip())
            return f"{text.split()[0]} {first[1] if first else ''}"
        return declaration(text)
    word = DECLARATION.match(text)
    if kind == "func" or not word:
        return ""
    if kind == "block":
        return word[1]
    return f"{outer}.{word[1]}"


def go_prose(run: list[str]) -> str:
    """Join comment lines into one paragraph, leaving out code and lists.

    Args:
        run: The comment's lines, with their ``//`` markers.

    Returns:
        The comment's prose on one line.
    """
    words = []
    for line in run:
        text = line.strip()[2:]
        if text.strip() and not text.startswith(("\t", "  ")):
            words.append(text.strip())
    return " ".join(words)


def sentences(prose: str) -> list[str]:
    """Split prose after ``.``, ``!`` or ``?`` followed by a space.

    Args:
        prose: Comment prose on one line.

    Returns:
        The sentences, without empty ones.
    """
    return [s for s in re.split(r"(?<=[.!?])\s+", prose) if s]


def check(comments: list[Comment], base: set[str] | None) -> list[Finding]:
    """Check every comment against the ``RULES``.

    Args:
        comments: Comments from the checked files.
        base: The sentences of every Go comment under the repository at
            ``--base``, or ``None`` without it.

    Returns:
        Findings in file order. A finding with a ``note`` is only
        reported: it is in a header, or in a sentence the task did not
        add or change.
    """
    findings = []
    for comment in comments:
        note = "header: report only" if comment.header else ""

        def add(rule: str, message: str, found: list[str]) -> None:
            """Record a finding on the current comment.

            Args:
                rule: The key in ``RULES`` the finding belongs to.
                message: What the checker found.
                found: The sentences involved. When all were there at
                    ``--base``, a question is dropped and a failure is
                    only reported.
            """
            why = note
            if base is not None and all(s in base for s in found):
                if rule == "restated-name":
                    return
                why = why or "existing: report only"
            findings.append(Finding(comment, rule, message, why))

        own = sentences(comment.prose)
        name = re.split(r"[. ]", comment.name)[-1]
        if name and own and re.search(rf"\b{re.escape(name)}\b", own[0]):
            message = f'first sentence repeats the name "{name}"'
            add("restated-name", message, own[:1])

        def having(pattern: str) -> list[str]:
            """Return the sentences with ``pattern`` as whole words."""
            whole = rf"\b{re.escape(pattern.lower())}\b"
            return [s for s in own if re.search(whole, s.lower())]

        # The code is the subject: "This ...", "Name ...", or "Package name".
        subject = rf"(?:This|{re.escape(name)}|Package {re.escape(name)})\b"
        about_code = [
            s for s in own if re.match(subject if name else r"This\b", s)
        ]
        for phrase in FILLER:
            matched = [s for s in having(phrase) if s in about_code]
            if matched:
                add("filler", f'"{phrase}"', matched)
    return findings


def base_comments(repo: Path, rev: str) -> list[Comment]:
    """Read every Go comment under ``repo`` at ``rev``.

    Every Go file counts, not only the checked ones, so a person's
    sentence in a moved or renamed file is still theirs.

    Args:
        repo: The folder given as ``--repo``, inside a Git repository.
        rev: A commit, branch or tag.

    Returns:
        The comments in every Go file at ``rev``.
    """
    git = ["git", "-C", str(repo)]
    known = subprocess.run(
        [*git, "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}"],
        capture_output=True,
    )
    if known.returncode != 0:
        print(f"--base {rev}: not a commit in {repo}", file=sys.stderr)
        sys.exit(2)
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
        found += go_comments(source, file)
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
    parser.add_argument("--base", help="commit the task started from")
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args(argv)
    repo = args.repo.resolve()
    files: set[str] = set()
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
            if file in files:
                continue
            source = path.read_text(encoding="utf-8")
        except (OSError, ValueError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        if GENERATED.search(source):
            print(f"{path}: skipped; generated", file=sys.stderr)
            continue
        comments += go_comments(source, file)
        files.add(file)
    if not files:
        print("no Go file to check", file=sys.stderr)
        return 2
    base = None
    if args.base:
        before = base_comments(repo, args.base)
        base = {s for c in before for s in sentences(c.prose)}
    findings = check(comments, base)
    for f in findings:
        place = f"{f.comment.file}:{f.comment.line} {f.comment.name or '-'}"
        note = f" ({f.note})" if f.note else ""
        print(f"{place}: {f.rule}: {f.message}{note}")
    for rule in dict.fromkeys(f.rule for f in findings):
        print(f'{rule} rule: "{RULES[rule]}"')
    failed = base is not None and any(
        f.rule == "filler" and not f.note for f in findings
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
