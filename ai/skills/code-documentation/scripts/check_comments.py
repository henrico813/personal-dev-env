#!/usr/bin/env python3
"""Flag Go comments that break a code-documentation rule.

Example:
    python3 check_comments.py --repo . internal/shop/*.go
    python3 check_comments.py --repo . --base main internal/shop/*.go
    python3 check_comments.py --repo . --base main --approve shop/*.go
    python3 check_comments.py --repo . --base main --approve \
        --rename Total=Sum shop/*.go

Reads whole-line Go ``//`` comments. ``.code-comments.json`` at the
repository root holds the owner's approved text, ``""`` for a cut
comment, and words to avoid. ``--approve`` rewrites that file::

    {"approved": [{"file": "store/cart.go", "name": "Cart.Total",
                   "text": "// Total is in cents.\n"}],
     "avoid": {"client": "say customer"}}

``--rename OLD=NEW`` moves the entries for a renamed comment in the
checked files, and its comment at ``--base``, to the new name first.
Without it, a renamed comment is recorded as cut under its old name.

Exits 1 when approved text changed or is gone, in any file it names.
With ``--base``, approved text found unchanged under a name that had no
comment at that commit and no entry is reported as moved, not failed.
With ``--base``, it also exits 1 when a sentence no Go file had at that
commit has an avoided word or filler. Exits 2 on unreadable input,
including a settings file of another shape.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass, replace
from pathlib import Path
from typing import TypeGuard

SETTINGS = ".code-comments.json"
# Each rule and the code-documentation sentence it checks, word for word.
RULES = {
    "approved": (
        "Never change that text or that file, even when the text seems "
        "false; report it instead."
    ),
    "restated-name": (
        "Prose derived directly from a name, signature, single statement, "
        "or a test's name, inputs, and assertions is a restatement and "
        "must not be added."
    ),
    "repeated-fact": (
        "Add only useful missing context and remove new filler or narration."
    ),
    "avoided-word": (
        "Explain project-specific terms when a new contributor would not "
        "know them."
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
# Two sentences repeat a fact when they share at least three content
# words, covering half of the shorter one. On a Go package the owner
# reviewed, this flagged 10 pairs in 25 comments, including 5 of the 6
# repeats the owner removed.
SHARED_WORDS = 3
SHARED_PART = 0.5
STOPWORDS = frozenset(
    "the and for that this with from its are was were has have had not "
    "but can may must any all one when then than into onto only also "
    "each which what who how why where does did been being their there "
    "they them our out you your".split()
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
    above a ``const (`` block. A comment inside a function body, with a
    blank line after it, or whose name another comment in the file
    shares, has the name ``""`` and cannot be approved. ``header`` is
    true for a comment above the ``package`` line with a blank line
    after it, where license text usually sits. A package doc comment
    touches the ``package`` line, so it is named, not a header.
    """

    file: str
    line: int
    name: str
    old: str
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
                    "".join(run),
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


def content_words(sentence: str) -> set[str]:
    """Return a sentence's content words, used to compare facts.

    Args:
        sentence: One sentence.

    Returns:
        Lowercased words of three or more letters, without common words
        and without a plural ``s``.
    """
    words = set()
    for word in re.findall(r"[a-z][a-z0-9'-]+", sentence.lower()):
        if len(word) > 2 and word not in STOPWORDS:
            words.add(
                word[:-1] if word.endswith("s") and len(word) > 4 else word
            )
    return words


def renamed_from(
    name: str, text: str, file: str, comments: list[Comment]
) -> list[Comment]:
    """Find comments that may be a gone approved comment after a rename.

    A comment matches when its first sentence has the same words as the
    approved text's first sentence, except for words that look like Go
    identifiers (``MarkUnclear``, ``io.Reader``, ``max_size``, ``V2``)
    or are either comment's own name.

    Args:
        name: The gone comment's name.
        text: Its approved text.
        file: The file it was approved in.
        comments: Every checked comment.

    Returns:
        The matching named comments in ``file``, in file order.
    """
    first = sentences(go_prose(text.splitlines(keepends=True)))[:1]
    words = first[0].split() if first else []
    likely = []
    for c in comments:
        if c.file != file or not c.name or c.name == name:
            continue
        own = sentences(c.prose)[:1]
        theirs = own[0].split() if own else []
        names = {re.split(r"[. ]", n)[-1] for n in (name, c.name)}
        if (
            words
            and len(theirs) == len(words)
            and all(
                a == b or identifier(a, names) or identifier(b, names)
                for a, b in zip(words, theirs)
            )
        ):
            likely.append(c)
    return likely


def identifier(word: str, names: set[str]) -> bool:
    """Return whether a prose word looks like a Go identifier.

    Args:
        word: One word of a sentence, with any punctuation around it.
        names: Names that count as identifiers here.

    Returns:
        True for one of ``names``, or a word with a capital letter after
        its first, a digit, ``_`` or an inner ``.``.
    """
    core = word.strip("`'\",;:()[]!?").rstrip(".")
    return core in names or bool(re.search(r".[A-Z]|[0-9_]|\w\.\w", core))


def check(
    files: set[str],
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
    avoid: dict[str, str],
    base: set[str] | None,
    base_names: set[tuple[str, str]],
) -> list[Finding]:
    """Check every comment against the ``RULES``.

    Args:
        files: Paths of the checked files, relative to the repository.
        comments: Comments from those files.
        approved: Approved text by ``(file, name)``; ``""`` means the
            owner cut the comment.
        avoid: Words not to use, each with what to say instead.
        base: The sentences of every Go comment under the repository at
            ``--base``, or ``None`` without it.
        base_names: ``(file, name)`` of every named comment at
            ``--base``.

    Returns:
        Findings in file order. A finding with a ``note`` is only
        reported: it is in approved text, in a header, or in a sentence
        the task did not add or change, or it is approved text that
        moved to a new name unchanged.
    """
    findings = []
    found = {(c.file, c.name) for c in comments}
    # Comments holding approved text under a new name stay approved.
    moved: set[Comment] = set()
    for (file, name), text in sorted(approved.items()):
        gone = Comment(file, 0, name, "", "")
        if text and file not in files:
            findings.append(Finding(gone, "approved", "approved file is gone"))
        elif text and (file, name) not in found:
            # The text counts as moved only to a name that is new since
            # --base and has no entry, so a copy elsewhere cannot hide a
            # deletion. Only a named comment can be recorded.
            same = [
                c
                for c in comments
                if base is not None
                and c.file == file
                and c.name
                and c.old == text
                and (c.file, c.name) not in approved
                and (c.file, c.name) not in base_names
                and c not in moved
            ]
            if same:
                moved.add(same[0])
                message = (
                    f"approved text moved to {file}:{same[0].line} "
                    f"{same[0].name}; record it with --approve --rename"
                )
                note = "renamed: report only"
                findings.append(Finding(gone, "approved", message, note))
                continue
            message = "approved comment is gone"
            likely = renamed_from(name, text, file, comments)
            if likely:
                places = ", ".join(f"{c.name} at {c.line}" for c in likely)
                message += f"; possibly now {places}"
            findings.append(Finding(gone, "approved", message))
    split = [
        (c, s, content_words(s)) for c in comments for s in sentences(c.prose)
    ]
    for comment in comments:
        key = (comment.file, comment.name)
        has_entry = bool(comment.name) and key in approved
        is_approved = has_entry or comment in moved
        if has_entry and approved[key] != comment.old:
            message = (
                "the owner cut this comment"
                if not approved[key]
                else "approved text changed; restore it"
            )
            findings.append(Finding(comment, "approved", message))
            continue
        note = "approved: report only" if is_approved else ""
        if not note and comment.header:
            note = "header: report only"

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
                if rule in ("restated-name", "repeated-fact"):
                    return
                why = why or "existing: report only"
            findings.append(Finding(comment, rule, message, why))

        own = sentences(comment.prose)
        name = re.split(r"[. ]", comment.name)[-1]
        if not is_approved:
            if name and own and re.search(rf"\b{re.escape(name)}\b", own[0]):
                message = f'first sentence repeats the name "{name}"'
                add("restated-name", message, own[:1])
            for other, sentence, words in split:
                if other is comment:
                    continue
                for mine in own:
                    mine_words = content_words(mine)
                    common = len(mine_words & words)
                    smaller = min(len(mine_words), len(words))
                    if (
                        common >= SHARED_WORDS
                        and common >= SHARED_PART * smaller
                    ):
                        place = f"{other.file}:{other.line}"
                        message = f"repeats {place}: {sentence!r}"
                        add("repeated-fact", message, [mine, sentence])
                        break

        def having(pattern: str) -> list[str]:
            """Return the sentences with ``pattern`` as whole words."""
            whole = rf"\b{re.escape(pattern.lower())}\b"
            return [s for s in own if re.search(whole, s.lower())]

        for word, instead in avoid.items():
            if having(word):
                add("avoided-word", f'"{word}": {instead}', having(word))
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


def load_settings(
    repo: Path,
) -> tuple[dict[tuple[str, str], str], dict[str, str]]:
    """Read approved text and avoided words from ``.code-comments.json``.

    Exits 2 when the file is not JSON of the shape in the module
    docstring.

    Args:
        repo: The folder given as ``--repo``.

    Returns:
        Approved text by ``(file, name)`` and the avoid list; both are
        empty when no file exists.
    """
    path = repo / SETTINGS
    if not path.exists():
        return {}, {}
    try:
        settings = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        print(f"{path}: {error}", file=sys.stderr)
        sys.exit(2)
    is_object = isinstance(settings, dict)
    entries = settings.get("approved", []) if is_object else None
    avoid = settings.get("avoid", {}) if is_object else None
    if not (
        isinstance(entries, list)
        and all(
            strings(e) and set(e) >= {"file", "name", "text"} for e in entries
        )
        and strings(avoid)
    ):
        print(
            f'{path}: expected {{"approved": [{{"file", "name", "text"}}], '
            '"avoid": {word: advice}} of strings; see check_comments.py',
            file=sys.stderr,
        )
        sys.exit(2)
    approved = {(e["file"], e["name"]): e["text"] for e in entries}
    return approved, avoid


def strings(value: object) -> TypeGuard[dict[str, str]]:
    """Return whether ``value`` is a JSON object whose values are strings.

    Args:
        value: A value read from the settings file.

    Returns:
        True for a dict with string keys and values.
    """
    return isinstance(value, dict) and all(
        isinstance(k, str) and isinstance(v, str) for k, v in value.items()
    )


def base_comments(repo: Path, rev: str) -> list[Comment]:
    """Read every Go comment under ``repo`` at ``rev``.

    Every Go file counts, not only the checked ones, so a person's
    sentence in a moved or renamed file is still theirs.

    Args:
        repo: The folder given as ``--repo``, inside a Git repository.
        rev: A commit, branch or tag.

    Returns:
        The comments, with shared names removed.
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
        source = subprocess.run(
            [*git, "show", f"{rev}:./{file}"], capture_output=True, text=True
        ).stdout
        found += go_comments(source, file)
    return unname_shared(found)[0]


def unname_shared(
    comments: list[Comment],
) -> tuple[list[Comment], dict[tuple[str, str], list[int]]]:
    """Take the name off comments whose name another comment shares.

    Two ``func init()`` or two ``var _`` lines in one file are ordinary
    Go, but approved text could not tell their comments apart.

    Args:
        comments: Every checked comment.

    Returns:
        The comments, with shared names set to ``""``, and the lines
        of each shared name by ``(file, name)``.
    """
    lines: dict[tuple[str, str], list[int]] = {}
    for comment in comments:
        if comment.name:
            lines.setdefault((comment.file, comment.name), []).append(
                comment.line
            )
    shared = {key: found for key, found in lines.items() if len(found) > 1}
    unnamed = [
        replace(c, name="") if (c.file, c.name) in shared else c
        for c in comments
    ]
    return unnamed, shared


def approve(
    files: set[str],
    comments: list[Comment],
    before: list[Comment],
    approved: dict[tuple[str, str], str],
) -> list[dict[str, str]]:
    """List approved entries for the checked files, for the owner.

    Args:
        files: Paths of the checked files.
        comments: Their current comments.
        before: Every comment at ``--base``, or none without it.
        approved: Entries already in ``.code-comments.json``.

    Returns:
        One entry per named comment, then ``""`` for each named comment
        of these files that was there at ``--base`` and is now gone,
        then the existing entries for other files and for cut comments
        that are still gone, so saving the result loses no approval.
    """
    now = {(c.file, c.name) for c in comments}
    entries = [
        {"file": c.file, "name": c.name, "text": c.old}
        for c in comments
        if c.name
    ]
    for c in before:
        if c.name and c.file in files and (c.file, c.name) not in now:
            entries.append({"file": c.file, "name": c.name, "text": ""})
    listed = {(e["file"], e["name"]) for e in entries}
    for (file, name), text in sorted(approved.items()):
        kept = file not in files or (not text and (file, name) not in now)
        if kept and (file, name) not in listed:
            entries.append({"file": file, "name": name, "text": text})
    return entries


def rename(
    renames: dict[str, str],
    files: set[str],
    approved: dict[tuple[str, str], str],
    before: list[Comment],
) -> tuple[dict[tuple[str, str], str], list[Comment]]:
    """Move entries and ``--base`` comments in ``files`` to new names.

    Renaming the ``--base`` comments keeps ``approve`` from recording a
    renamed comment as cut. An entry already under the new name wins.

    Args:
        renames: New names by old name.
        files: Paths of the checked files; entries elsewhere keep their
            names, since another package may use the same name.
        approved: Entries from ``.code-comments.json``.
        before: Every comment at ``--base``.

    Returns:
        The renamed entries and ``--base`` comments.
    """

    def moves(file: str, name: str) -> bool:
        """Return whether ``--rename`` applies to this comment."""
        return file in files and name in renames

    entries = {key: text for key, text in approved.items() if not moves(*key)}
    for (file, name), text in sorted(approved.items()):
        if moves(file, name):
            entries.setdefault((file, renames[name]), text)
    renamed = [
        replace(c, name=renames[c.name]) if moves(c.file, c.name) else c
        for c in before
    ]
    return entries, renamed


def save_settings(
    repo: Path,
    entries: list[dict[str, str]],
    avoid: dict[str, str],
    approved: dict[tuple[str, str], str],
) -> str:
    """Replace ``.code-comments.json`` in one step and summarize it.

    The new file is written beside the old one and renamed over it, so
    an interrupted run leaves the old file whole. A shell redirect
    would empty the file before the checker could read it.

    Args:
        repo: The folder given as ``--repo``.
        entries: The approved entries to write.
        avoid: The avoid list to keep.
        approved: The entries the file held, after ``--rename``.

    Returns:
        A line counting entries added or changed, kept, and cut.
    """
    path = repo / SETTINGS
    text = json.dumps({"approved": entries, "avoid": avoid}, indent=1)
    handle, temp = tempfile.mkstemp(dir=repo, prefix=f"{SETTINGS}.")
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as out:
            out.write(text + "\n")
        mode = stat.S_IMODE(path.stat().st_mode) if path.exists() else 0o644
        os.chmod(temp, mode)
        os.replace(temp, path)
    except BaseException:
        Path(temp).unlink(missing_ok=True)
        raise
    cut = sum(1 for e in entries if not e["text"])
    kept = sum(
        1
        for e in entries
        if e["text"] and approved.get((e["file"], e["name"])) == e["text"]
    )
    added = len(entries) - cut - kept
    return f"{SETTINGS}: {added} added or changed, {kept} kept, {cut} cut"


def rename_pair(value: str) -> tuple[str, str]:
    """Parse one ``--rename OLD=NEW`` value.

    Args:
        value: The option's text.

    Returns:
        The old and new names.
    """
    old, _, new = value.partition("=")
    if not old or not new:
        raise argparse.ArgumentTypeError(f"{value!r} is not OLD=NEW")
    return old, new


def main(argv: list[str] | None = None) -> int:
    """Check the given Go files and print one line per finding.

    Args:
        argv: Command-line arguments; ``sys.argv`` when ``None``.

    Returns:
        The exit code described in the module docstring.
    """
    # "@renames.txt" reads one argument per line, such as
    # "--rename=const Red=const Crimson".
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0], fromfile_prefix_chars="@"
    )
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--base", help="commit the task started from")
    parser.add_argument(
        "--approve",
        action="store_true",
        help=f"rewrite {SETTINGS}; only the owner, or at their request",
    )
    parser.add_argument(
        "--rename",
        action="append",
        default=[],
        type=rename_pair,
        metavar="OLD=NEW",
        help="with --approve: a renamed comment; repeat for each",
    )
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args(argv)
    if args.rename and not args.approve:
        parser.error("--rename needs --approve")
    repo = args.repo.resolve()
    approved, avoid = load_settings(repo)
    paths = list(args.paths)
    if not args.approve:
        # An unlisted file keeps its approved text checked; a missing one
        # is reported by check() as gone.
        named = sorted({repo / f for f, _ in approved})
        paths += [path for path in named if path.exists()]
    files, comments = set(), []
    for path in paths:
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
    comments, shared = unname_shared(comments)
    before = base_comments(repo, args.base) if args.base else []
    if args.approve:
        known = {n for f, n in approved if f in files}
        known |= {c.name for c in before if c.file in files}
        for old, new in args.rename:
            if old not in known:
                print(
                    f"--rename {old}={new}: no entry or --base comment "
                    f"is named {old!r} in the checked files",
                    file=sys.stderr,
                )
        approved, before = rename(dict(args.rename), files, approved, before)
        entries = approve(files, comments, before, approved)
        print(save_settings(repo, entries, avoid, approved))
        return 0
    for (file, name), found in shared.items():
        for line in found:
            print(
                f"{file}:{line} {name}: cannot be approved; "
                f"{len(found)} comments in this file have this name"
            )
    base = None
    if args.base:
        base = {s for c in before for s in sentences(c.prose)}
    base_names = {(c.file, c.name) for c in before if c.name}
    findings = check(files, comments, approved, avoid, base, base_names)
    for f in findings:
        # A gone comment has no line to point at.
        at = f":{f.comment.line}" if f.comment.line else ""
        place = f"{f.comment.file}{at} {f.comment.name or '-'}"
        note = f" ({f.note})" if f.note else ""
        print(f"{place}: {f.rule}: {f.message}{note}")
    for rule in dict.fromkeys(f.rule for f in findings):
        print(f'{rule} rule: "{RULES[rule]}"')
    failed = any(
        (
            f.rule == "approved"
            or (base is not None and f.rule in ("avoided-word", "filler"))
        )
        and not f.note
        for f in findings
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
