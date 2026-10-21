#!/usr/bin/env python3
"""Report Go comments that break a code-documentation rule.

Example:
    python3 check_comments.py --repo . internal/shop/*.go
    python3 check_comments.py --repo . --base main internal/shop/*.go
    python3 check_comments.py --repo . --base main --approve shop/*.go
    python3 check_comments.py --repo . --base main --approve \
        --rename Total=Sum --rename cart.go=basket.go shop/*.go
    python3 check_comments.py --repo . --approve-comment shop/cart.go Total
    python3 check_comments.py --repo . --merge-approvals ../wt 1a2b3c4

Prints one line per finding, then quotes each broken rule's sentence
from the skill. ``.code-comments.json`` in the ``--repo`` folder holds
the owner's approved text, ``""`` for a cut comment, and words to
avoid. ``--approve`` rewrites the approved text from the checked files,
after ``--rename OLD=NEW`` moves the entries of each renamed comment or
``.go`` file, so a rename is not recorded as a cut::

    {"approved": [{"file": "store/cart.go", "name": "Cart.Total",
                   "text": "// Total is in cents.\n"}],
     "avoid": {"client": "say customer"}}

``--approve-comment FILE NAME`` records only that comment's text, or
``""`` when it is gone, and leaves every other entry as it was.

``--merge-approvals FOLDER COMMIT`` takes ``FOLDER``'s approvals, such
as a worktree's before removal, for the Go files ``COMMIT`` changed,
when their text is the comment now. If the two files approve a comment
differently and neither is the comment now, nothing is written.

The rules are:

- ``approved``: approved text changed, a cut comment came back, or an
  approved comment or its file is gone. Without ``--approve``, every
  file the settings name is checked, even when it is not given.
- ``filler``: a sentence about the code uses one of the skill's vague
  filler phrases, such as "facilitates".
- ``avoided-word``: a sentence uses a word from the avoid list.
- ``restated-name``: a comment's first sentence repeats the name of the
  code below it. This never fails the run.

With ``--base``, sentences present at that commit never fail the run,
and ``restated-name`` findings on them are left out. Filler and
avoided words in approved text, or in a license header above
``package``, are only reported. Generated files are skipped.

Exit codes:
    0: Nothing failed.
    1: An ``approved`` finding, or, with ``--base``, filler or an
        avoided word in a sentence not present at that commit, or
        ``--merge-approvals`` found two approvals of a comment, neither
        of them the comment now.
    2: A path is a folder or unreadable, no Go file was given,
        ``--base`` is not a commit, the settings file is not JSON of
        the shape above, ``--approve-comment`` names a FILE that is
        not Go, an empty NAME, or a NAME that two comments in FILE
        share, or ``--merge-approvals`` names a FOLDER without
        ``.code-comments.json``, or a COMMIT that ``--repo``'s HEAD
        lacks or whose changes Git cannot list.
"""

from __future__ import annotations

import argparse
import enum
import json
import os
import re
import shlex
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import NamedTuple, TypeGuard

SETTINGS = ".code-comments.json"
# Each rule and the code-documentation sentence it checks, word for word.
RULES = {
    "approved": (
        "On a later pass over unchanged behavior, edit existing prose only "
        "to fix a specific false or unsupported claim, an omission that "
        "makes it misleading for safe use, or an applicable convention "
        "violation; otherwise leave it byte-for-byte unchanged."
    ),
    "restated-name": (
        "Prose derived directly from a name, signature, single statement, "
        "or a test's name, inputs, and assertions is a restatement and "
        "must not be added."
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
# Go comments and string literals, matched from left to right, so a
# backtick or "/*" inside one starts nothing.
GO_COMMENT_OR_STRING = re.compile(
    r"//[^\n]*|/\*.*?\*/|`[^`]*`"
    r"""|"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'""",
    re.S,
)


@dataclass(frozen=True)
class Comment:
    """One comment and the name of the code it sits on.

    Names look like ``Send``, ``Cart.Total`` for a method, ``Cart.Items``
    for a struct field or map entry, and ``const Red`` for a comment above
    a ``const (`` block. A comment has the name ``""`` inside a function
    body, with a blank line after it, above an empty block, or, after
    ``unname_shared_names``, when another comment in the file has the same
    name. Such comments cannot be approved.

    ``text`` is the comment exactly as written, which approved text is
    compared with. ``header`` is true for a comment above ``package`` with
    a blank line after it, where license text sits; a package doc comment
    touches ``package``, so it is named instead.
    """

    file: str
    line: int
    name: str
    text: str
    prose: str
    header: bool = False


@dataclass(frozen=True)
class Finding:
    """A problem in one comment, under one of the ``RULES``.

    ``note`` says why the finding is only reported, or is empty. A finding
    with an empty note fails the run when its rule is ``approved``, or,
    with ``--base``, ``filler`` or ``avoided-word``.
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
                    prose=join_comment_prose(comment_lines),
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


def find_rename_candidates(
    name: str,
    text: str,
    file: str,
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
) -> list[Comment]:
    """Find comments that may be a gone approved comment after a rename.

    A comment matches when its first sentence has the same words as the
    approved text's first sentence, except where both words look like
    Go identifiers (``Total``, ``io.Reader``, ``max_size``, ``V2``) or
    are the two comments' names. A comment with approved text of its
    own is the owner's other comment, not this one, so it never matches.

    Args:
        text: The gone comment's approved text.

    Returns:
        The matching named comments in ``file``, in file order.
    """
    approved_words = split_first_sentence(
        join_comment_prose(text.splitlines(keepends=True))
    )
    candidates = []
    for comment in comments:
        if comment.file != file or not comment.name or comment.name == name:
            continue
        if approved.get((comment.file, comment.name)):
            continue
        words = split_first_sentence(comment.prose)
        bare_names = {re.split(r"[. ]", n)[-1] for n in (name, comment.name)}
        if (
            approved_words
            and len(words) == len(approved_words)
            and all(
                a == b
                or (
                    looks_like_identifier(a, bare_names)
                    and looks_like_identifier(b, bare_names)
                )
                for a, b in zip(approved_words, words)
            )
        ):
            candidates.append(comment)
    return candidates


def split_first_sentence(prose: str) -> list[str]:
    """Return the words of the first sentence, with their punctuation."""
    first = split_sentences(prose)[:1]
    return first[0].split() if first else []


def looks_like_identifier(word: str, names: set[str]) -> bool:
    """Return whether a prose word looks like a Go identifier.

    Args:
        word: One word of a sentence, with any punctuation around it.
        names: Names that count as identifiers here.

    Returns:
        True for one of ``names``, or a word with a capital letter, a
        digit, ``_`` or an inner ``.``.
    """
    core = word.strip("`'\",;:()[]!?").rstrip(".")
    return core in names or bool(re.search(r"[A-Z]|[0-9_]|\w\.\w", core))


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


def find_avoided_words(
    sentences: list[str], avoid: dict[str, str]
) -> list[RuleMatch]:
    """Match each avoided word, as a whole word in any sentence."""
    matches = []
    for word, instead in avoid.items():
        whole_word = rf"\b{re.escape(word.lower())}\b"
        found = [s for s in sentences if re.search(whole_word, s.lower())]
        if found:
            matches.append(
                RuleMatch("avoided-word", f'"{word}": {instead}', found)
            )
    return matches


def find_gone_approvals(
    files: set[str],
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
) -> list[Finding]:
    """Find approved comments, or their files, that are gone.

    A cut comment that stays gone is what the owner asked for, so only
    entries with text count.
    """
    current = {(c.file, c.name) for c in comments}
    findings = []
    for (file, name), text in sorted(approved.items()):
        if not text:
            continue
        gone = Comment(file=file, line=0, name=name, text="", prose="")
        if file not in files:
            findings.append(Finding(gone, "approved", "approved file is gone"))
        elif (file, name) not in current:
            message = "approved comment is gone"
            candidates = find_rename_candidates(
                name, text, file, comments, approved
            )
            for candidate in candidates:
                pair = shlex.quote(f"{name}={candidate.name}")
                message += (
                    f"; possibly now {candidate.name} at {candidate.line}"
                )
                message += f" (--approve --rename {pair})"
            findings.append(Finding(gone, "approved", message))
    return findings


def find_changed_approval(
    comment: Comment, approved: dict[tuple[str, str], str]
) -> Finding | None:
    """Find a comment whose text differs from what the owner approved."""
    approved_text = approved[(comment.file, comment.name)]
    if approved_text == comment.text:
        return None
    if not approved_text:
        return Finding(comment, "approved", "the owner cut this comment")
    return Finding(comment, "approved", "approved text changed; restore it")


def find_rule_violations(
    files: set[str],
    comments: list[Comment],
    approved: dict[tuple[str, str], str],
    avoid: dict[str, str],
    base_sentences: set[str] | None,
) -> list[Finding]:
    """Check every comment against the ``RULES``.

    Args:
        files: Paths of the checked files, relative to the repository.
        approved: Approved text by ``(file, name)``; ``""`` means the
            owner cut the comment.
        avoid: Words not to use, each with what to say instead.
        base_sentences: The sentences of every Go comment under the
            repository at ``--base``, or ``None`` without it.

    Returns:
        Gone approved comments, then findings in file order.
    """
    findings = find_gone_approvals(files, comments, approved)
    for comment in comments:
        key = (comment.file, comment.name)
        is_approved = bool(comment.name) and key in approved
        if is_approved:
            changed = find_changed_approval(comment, approved)
            # The owner's text must come back as it was, so other rules
            # would only ask the agent to edit it.
            if changed:
                findings.append(changed)
                continue
        sentences = split_sentences(comment.prose)
        # Prose names code without its qualifier: "Cart.Total" is written
        # "Total", and "const Red" is written "Red".
        name = re.split(r"[. ]", comment.name)[-1]
        matches = [] if is_approved else find_restated_name(sentences, name)
        matches += find_avoided_words(sentences, avoid)
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
            if is_approved:
                note = "approved: report only"
            elif comment.header:
                note = "header: report only"
            elif present_at_base:
                note = "existing: report only"
            findings.append(Finding(comment, match.rule, match.message, note))
    return findings


def load_settings(
    repo: Path,
) -> tuple[dict[tuple[str, str], str], dict[str, str]]:
    """Read approved text and avoided words from ``.code-comments.json``.

    Returns:
        Approved text by ``(file, name)`` and the avoid list; both are
        empty when no file exists.

    Raises:
        ValueError: The file is not JSON of the shape in the module
            docstring. The owner edits the avoid list by hand.
    """
    path = repo / SETTINGS
    if not path.exists():
        return {}, {}
    try:
        settings = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"{path}: {error}") from error
    is_object = isinstance(settings, dict)
    entries = settings.get("approved", []) if is_object else None
    avoid = settings.get("avoid", {}) if is_object else None
    if not (
        isinstance(entries, list)
        and all(
            is_string_map(entry) and {"file", "name", "text"} <= set(entry)
            for entry in entries
        )
        and is_string_map(avoid)
    ):
        raise ValueError(
            f'{path}: expected {{"approved": [{{"file", "name", "text"}}], '
            '"avoid": {word: advice}} of strings; see check_comments.py'
        )
    approved = {(e["file"], e["name"]): e["text"] for e in entries}
    return approved, avoid


def is_string_map(value: object) -> TypeGuard[dict[str, str]]:
    """Return whether ``value`` is a dict of strings to strings."""
    return isinstance(value, dict) and all(
        isinstance(k, str) and isinstance(v, str) for k, v in value.items()
    )


def read_comments_at_commit(repo: Path, rev: str) -> list[Comment]:
    """Read every Go comment under ``repo`` at ``rev``.

    Every Go file counts, not only the checked ones, so a person's
    sentence in a moved or renamed file is still theirs.

    Args:
        repo: The folder given as ``--repo``, inside a Git repository.

    Returns:
        The comments in every Go file at ``rev``, with shared names
        removed.

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
    return unname_shared_names(found)[0]


def unname_shared_names(
    comments: list[Comment],
) -> tuple[list[Comment], dict[tuple[str, str], list[int]]]:
    """Take the name off comments whose name another comment shares.

    Two ``func init()`` or two ``var _`` lines in one file are ordinary
    Go, but approved text could not tell their comments apart.

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


def build_approved_entries(
    repo: Path,
    files: set[str],
    comments: list[Comment],
    base_comments: list[Comment],
    approved: dict[tuple[str, str], str],
) -> tuple[list[dict[str, str]], int]:
    """Build the settings entries that ``--approve`` writes.

    Running ``--approve`` is the owner's approval of what changed: a
    comment gone from a checked file is recorded as cut, and the
    entries of a deleted file are dropped.

    Args:
        files: Paths of the checked files.
        base_comments: Every comment at ``--base``, or none without it.
        approved: Entries already in ``.code-comments.json``.

    Returns:
        The entries to write, and how many were dropped with deleted
        files. Checked files get one entry per named comment, plus a cut
        entry for each comment gone since ``--base`` or since its entry.
        Other files keep their entries. Entries are sorted by file.
    """
    entries = [
        {"file": c.file, "name": c.name, "text": c.text}
        for c in comments
        if c.name
    ]
    # Record gone comments as cut, so they cannot come back unnoticed.
    current = {(c.file, c.name) for c in comments}
    earlier = [(c.file, c.name) for c in base_comments if c.name]
    earlier += [key for key in sorted(approved) if key[0] in files]
    for file, name in dict.fromkeys(earlier):  # In order, without repeats.
        if file in files and (file, name) not in current:
            entries.append({"file": file, "name": name, "text": ""})
    # Keep other files' entries, unless the file no longer exists.
    removed = 0
    for (file, name), text in sorted(approved.items()):
        if file in files:
            continue
        if not (repo / file).exists():
            removed += 1
            continue
        entries.append({"file": file, "name": name, "text": text})
    # Sort by file, keeping comment order within each, so the diff of an
    # approval shows only real changes.
    entries.sort(key=lambda entry: entry["file"])
    return entries, removed


@dataclass
class RenameRules:
    """The ``--rename`` pairs.

    ``used`` records each old name or path that renamed something.
    """

    file_moves: dict[str, str]
    name_changes: dict[str, str]
    checked_files: set[str]
    current_keys: set[tuple[str, str]]
    used: set[str] = field(default_factory=set)

    def move_key(self, file: str, name: str) -> tuple[str, str]:
        """Return ``(file, name)`` after the renames.

        Also records each old name or path it applied in ``used``.
        """
        if file in self.file_moves:
            self.used.add(file)
            file = self.file_moves[file]
        # Another file may still use the old name, so rename only in a
        # checked file where the old name is no longer a comment.
        if (
            file in self.checked_files
            and name in self.name_changes
            and (file, name) not in self.current_keys
        ):
            self.used.add(name)
            name = self.name_changes[name]
        return file, name


def apply_renames(
    renames: dict[str, str],
    files: set[str],
    current_keys: set[tuple[str, str]],
    approved: dict[tuple[str, str], str],
    base_comments: list[Comment],
) -> tuple[dict[tuple[str, str], str], list[Comment], list[str]]:
    """Move entries and ``--base`` comments to their new names.

    A pair of ``.go`` paths, relative to ``--repo``, moves a file; no Go
    name ends in ``.go``, since ``go`` is a keyword. Other pairs rename a
    comment. Files move first, and an entry already under the new name
    wins.

    Args:
        renames: New names or paths by old ones.
        current_keys: ``(file, name)`` of every comment in ``files``.

    Returns:
        The renamed entries and ``--base`` comments, and each old name
        or path that renamed nothing.
    """
    rename_rules = RenameRules(
        file_moves={o: n for o, n in renames.items() if o.endswith(".go")},
        name_changes={
            o: n for o, n in renames.items() if not o.endswith(".go")
        },
        checked_files=files,
        current_keys=current_keys,
    )
    new_keys = {key: rename_rules.move_key(*key) for key in sorted(approved)}
    # Unrenamed entries first, so one already under the new name wins.
    entries = {
        key: approved[key] for key, new in new_keys.items() if new == key
    }
    for key, new in new_keys.items():
        entries.setdefault(new, approved[key])
    moved = []
    for comment in base_comments:
        file, name = rename_rules.move_key(comment.file, comment.name)
        moved.append(replace(comment, file=file, name=name))
    unused = [old for old in renames if old not in rename_rules.used]
    return entries, moved, unused


def parse_rename_pair(value: str) -> tuple[str, str]:
    """Split one ``--rename OLD=NEW`` value into the old and new names."""
    old, _, new = value.partition("=")
    if not old or not new:
        raise argparse.ArgumentTypeError(f"{value!r} is not OLD=NEW")
    return old, new


def replace_settings_file(
    repo: Path, entries: list[dict[str, str]], avoid: dict[str, str]
) -> None:
    """Write ``.code-comments.json`` to a temporary file, then rename it over.

    An interrupted run leaves the old file whole. The old file's mode is
    kept.
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


def summarize_approval(
    entries: list[dict[str, str]],
    approved: dict[tuple[str, str], str],
    removed: int,
) -> str:
    """Return the line ``--approve`` prints.

    For example, ``1 added or changed, 0 kept, 1 newly cut``. An entry
    whose text matches its old entry is kept, ``""`` included, so only
    a comment cut by this approval counts as newly cut.

    Args:
        approved: The entries the file held before.
        removed: Entries dropped with deleted files.
    """
    old_texts = [approved.get((e["file"], e["name"])) for e in entries]
    kept = sum(1 for e, old in zip(entries, old_texts) if old == e["text"])
    newly_cut = sum(
        1 for e, old in zip(entries, old_texts) if not e["text"] and old != ""
    )
    added = len(entries) - kept - newly_cut
    summary = (
        f"{SETTINGS}: {added} added or changed, {kept} kept, "
        f"{newly_cut} newly cut"
    )
    if removed:
        summary += f", {removed} removed with deleted files"
    return summary


def approve_one_comment(repo: Path, file: str, name: str) -> int:
    """Record one comment's text as approved, leaving every other entry.

    A comment review approves each answer as it is given; ``--approve``
    would also approve the file's later comments before the owner
    reached them.

    Args:
        repo: The folder holding ``.code-comments.json``.
        file: The Go file, relative to ``repo``.
        name: The comment's name; a comment gone from ``file`` is
            recorded as cut.

    Returns:
        The exit code described in the module docstring.
    """
    try:
        file = (repo / file).resolve().relative_to(repo).as_posix()
        approved, avoid = load_settings(repo)
        source = (repo / file).read_text(encoding="utf-8")
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2
    if not file.endswith(".go") or not name:
        print(
            "--approve-comment needs a Go file and a comment name, "
            f"not {file!r} {name!r}",
            file=sys.stderr,
        )
        return 2
    comments, shared = unname_shared_names(parse_go_comments(source, file))
    if (file, name) in shared:
        print(
            f"{file} {name}: cannot be approved; "
            f"{len(shared[file, name])} comments in this file have this name",
            file=sys.stderr,
        )
        return 2
    text = next((c.text for c in comments if c.name == name), "")
    entry = {"file": file, "name": name, "text": text}
    entries = [
        {"file": f, "name": n, "text": t}
        for (f, n), t in {**approved, (file, name): text}.items()
    ]
    entries.sort(key=lambda e: e["file"])
    replace_settings_file(repo, entries, avoid)
    print(summarize_approval([entry], approved, 0))
    return 0


def list_changed_go_files(repo: Path, commit: str) -> set[str]:
    """Return the Go files ``commit`` changed, deleted ones included.

    Raises:
        subprocess.CalledProcessError: Git cannot compare ``commit``
            with its first parent.
    """
    diff = ["diff", "--name-only", "--no-renames", "--relative"]
    changed = subprocess.run(
        ["git", "-C", str(repo), *diff, f"{commit}^", commit],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.splitlines()
    return {file for file in changed if file.endswith(".go")}


def read_named_comments(
    repo: Path, files: set[str]
) -> dict[tuple[str, str], str]:
    """Return each named comment's text in ``files`` by ``(file, name)``,
    leaving out names two comments share.
    """
    comments: list[Comment] = []
    for file in sorted(files):
        source = (repo / file).read_text(encoding="utf-8")
        comments += parse_go_comments(source, file)
    return {
        (c.file, c.name): c.text
        for c in unname_shared_names(comments)[0]
        if c.name
    }


def merge_approvals(repo: Path, folder: Path, commit: str) -> int:
    """Take ``folder``'s approvals for the Go files ``commit`` changed.

    An entry is taken when its text is the comment now, since another
    worktree may have approved a later change. In those files, entries
    of deleted files, and approved text of gone comments ``folder``
    lacks, such as an old ``--rename`` name, are dropped.

    Args:
        repo: The main checkout; its HEAD must contain ``commit``.
        folder: The worktree whose ``.code-comments.json`` is read.
        commit: The merged pull request's commit.

    Returns:
        The exit code described in the module docstring.
    """
    git = ["git", "-C", str(repo)]
    ancestor_check = subprocess.run(
        [*git, "merge-base", "--is-ancestor", commit, "HEAD"],
        capture_output=True,
    )
    if ancestor_check.returncode != 0:
        print(
            f"{repo}: HEAD does not contain {commit}; update it first",
            file=sys.stderr,
        )
        return 2
    if not (folder / SETTINGS).exists():
        print(f"{folder / SETTINGS}: no such file", file=sys.stderr)
        return 2
    try:
        approved, avoid = load_settings(repo)
        incoming, _ = load_settings(folder)
        changed_files = list_changed_go_files(repo, commit)
    except (ValueError, subprocess.CalledProcessError) as error:
        print(error, file=sys.stderr)
        return 2
    present_files = {f for f in changed_files if (repo / f).exists()}
    comments_now = read_named_comments(repo, present_files)
    merged = {
        (file, name): text
        for (file, name), text in approved.items()
        if file not in changed_files or file in present_files
    }
    removed = len(approved) - len(merged)
    dropped = [
        (file, name)
        for (file, name), text in merged.items()
        if file in present_files
        and (file, name) not in incoming
        and (file, name) not in comments_now
        and text
    ]
    for key in dropped:
        del merged[key]
    taken = 0
    skipped: list[tuple[str, str]] = []
    conflicts: list[tuple[str, str]] = []
    for key, text in sorted(incoming.items()):
        file, _ = key
        if file not in present_files or approved.get(key) == text:
            continue
        text_now = comments_now.get(key, "")
        if text == text_now:
            merged[key] = text
            taken += 1
        elif key not in approved:
            # Taking it would fail the checker and have agents restore
            # older text.
            skipped.append(key)
        elif approved[key] != text_now:
            conflicts.append(key)
    for file, name in conflicts:
        print(
            f"{file} {name}: approved differently in {folder}, and neither "
            "text is the comment now; nothing written",
            file=sys.stderr,
        )
    if conflicts:
        return 1
    for file, name in skipped:
        print(
            f"{file} {name}: skipped; the approved text is not the comment now"
        )
    for file, name in dropped:
        print(f"{file} {name}: dropped; no longer a comment")
    if taken or removed or dropped:
        entries = [
            {"file": f, "name": n, "text": t} for (f, n), t in merged.items()
        ]
        entries.sort(key=lambda e: e["file"])
        replace_settings_file(repo, entries, avoid)
    print(
        f"{SETTINGS}: {taken} taken from {folder}, {len(skipped)} skipped, "
        f"{len(dropped)} dropped, {removed} removed with deleted files"
    )
    return 0


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
    parser.add_argument(
        "--base", help="commit to compare with; its sentences never fail"
    )
    parser.add_argument(
        "--approve",
        action="store_true",
        help=f"rewrite {SETTINGS}; only the owner, or at their request",
    )
    parser.add_argument(
        "--rename",
        action="append",
        default=[],
        type=parse_rename_pair,
        metavar="OLD=NEW",
        help="with --approve: a renamed comment or .go file; repeat for each",
    )
    parser.add_argument(
        "--approve-comment",
        nargs=2,
        metavar=("FILE", "NAME"),
        help="approve one comment; only the owner, or at their request",
    )
    parser.add_argument(
        "--merge-approvals",
        nargs=2,
        metavar=("FOLDER", "COMMIT"),
        help="take FOLDER's approvals for the Go files COMMIT changed; run "
        "as the cleanup-plan skill says",
    )
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args(argv)
    if args.rename and not args.approve:
        parser.error("--rename needs --approve")
    repo = args.repo.resolve()
    if args.merge_approvals:
        if args.approve or args.approve_comment or args.base or args.paths:
            parser.error("--merge-approvals takes no other option or paths")
        folder, commit = args.merge_approvals
        return merge_approvals(repo, Path(folder).resolve(), commit)
    if args.approve_comment:
        if args.approve or args.base or args.paths:
            parser.error(
                "--approve-comment takes no --approve, --base or paths"
            )
        return approve_one_comment(repo, *args.approve_comment)
    if not args.paths:
        parser.error("give the Go files to check")
    try:
        approved, avoid = load_settings(repo)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    paths = list(args.paths)
    if not args.approve:
        # An unlisted file keeps its approved text checked; a missing one
        # is reported as gone.
        approved_paths = sorted({repo / file for file, _ in approved})
        paths += [path for path in approved_paths if path.exists()]
    checked_files: set[str] = set()
    comments: list[Comment] = []
    for path in paths:
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
    comments, shared = unname_shared_names(comments)
    base_comments: list[Comment] = []
    if args.base:
        try:
            base_comments = read_comments_at_commit(repo, args.base)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
    for (file, name), found in shared.items():
        for line in found:
            print(
                f"{file}:{line} {name}: cannot be approved; "
                f"{len(found)} comments in this file have this name"
            )
    if args.approve:
        # Rename first, so a renamed comment's entry moves instead of
        # being cut as gone or dropped with a deleted file.
        current_keys = {(c.file, c.name) for c in comments}
        approved, base_comments, unused = apply_renames(
            dict(args.rename),
            checked_files,
            current_keys,
            approved,
            base_comments,
        )
        for old in unused:
            print(
                f"--rename {old}: matches no entry or --base comment in "
                "the checked files",
                file=sys.stderr,
            )
        entries, removed = build_approved_entries(
            repo, checked_files, comments, base_comments, approved
        )
        replace_settings_file(repo, entries, avoid)
        print(summarize_approval(entries, approved, removed))
        return 0

    base_sentences = None
    if args.base:
        base_sentences = {
            sentence
            for comment in base_comments
            for sentence in split_sentences(comment.prose)
        }

    findings = find_rule_violations(
        checked_files, comments, approved, avoid, base_sentences
    )
    broken_rules: list[str] = []
    for finding in findings:
        comment = finding.comment
        # A gone comment has no line to point at.
        at_line = f":{comment.line}" if comment.line else ""
        place = f"{comment.file}{at_line} {comment.name or '-'}"
        note = f" ({finding.note})" if finding.note else ""
        print(f"{place}: {finding.rule}: {finding.message}{note}")
        if finding.rule not in broken_rules:
            broken_rules.append(finding.rule)
    for rule in broken_rules:
        print(f'{rule} rule: "{RULES[rule]}"')
    failing_rules = {"approved"}
    if base_sentences is not None:
        failing_rules |= {"avoided-word", "filler"}
    failed = any(
        finding.rule in failing_rules and not finding.note
        for finding in findings
    )
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
