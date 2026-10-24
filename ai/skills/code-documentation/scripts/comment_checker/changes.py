"""Find the comments the owner must answer since a base commit."""

from __future__ import annotations

import subprocess
from pathlib import Path

from .approvals import load_settings, unname_unapprovable_comments
from .languages import LANGUAGES, UNSUPPORTED_SUFFIXES
from .records import Comment, SourceError
from .rules import find_rename_candidates


def run_git(repo: Path, *args: str) -> str:
    """Return the stdout of a Git command run in ``repo``.

    Raises:
        ValueError: Git failed, with its stderr.
    """
    run = subprocess.run(["git", "-C", str(repo), *args], capture_output=True)
    if run.returncode != 0:
        error = run.stderr.decode(errors="replace").strip()
        raise ValueError(f"git {' '.join(args)}: {error}")
    return run.stdout.decode(errors="replace")


def list_changed_files(repo: Path, base: str) -> dict[str, str]:
    """Return each file changed since ``base``, leaving out deleted ones.

    Untracked files count, and Git's rename detection follows moves.

    Returns:
        Each file's path at ``base`` by its path now, or ``""`` for a new
        file, both relative to ``repo``.
    """
    diff = run_git(
        repo,
        "diff",
        "-z",
        "--name-status",
        "-M",
        "--relative",
        base,
        "--",
    )
    untracked = run_git(
        repo,
        "ls-files",
        "-z",
        "--others",
        "--exclude-standard",
    )
    changed = {}
    # Each change is a status and a path; a move has the old path first.
    fields = diff.split("\0")[:-1]
    while fields:
        status, path = fields.pop(0), fields.pop(0)
        old_path = path
        if status.startswith("R"):
            path = fields.pop(0)
        if status != "D":
            changed[path] = "" if status == "A" else old_path
    for path in untracked.split("\0")[:-1]:
        changed[path] = ""
    return changed


def read_source_comments(
    sources: dict[str, str],
) -> tuple[list[Comment], list[Comment], set[tuple[str, str]]]:
    """Read the comments in ``sources``, skipping generated files.

    Returns:
        The named comments in file order, every comment, and each
        ``(file, name)`` that cannot be approved, such as a shared name.

    Raises:
        SourceError: A file cannot be parsed; the message names it.
    """
    found: list[Comment] = []
    for file, source in sorted(sources.items()):
        language = LANGUAGES[Path(file).suffix]
        if language.generated_marker.search(source):
            continue
        try:
            found += language.read_comments(source, file)
        except SourceError as error:
            raise SourceError(f"{file}: {error}") from error
    comments, unapprovable = unname_unapprovable_comments(found)
    named = [c for c in comments if c.name]
    return named, comments, set(unapprovable)


def read_moved_approvals(
    repo: Path, changed: dict[str, str], suffixes: set[str]
) -> tuple[dict[tuple[str, str], str], list[str]]:
    """Return the approvals in ``suffixes`` files, moved files followed.

    Returns:
        Approved text by ``(file, name)`` under each file's path now, and
        an ``OLD=NEW`` pair for each moved file that has approvals.

    Raises:
        ValueError: ``.code-comments.json`` is not of its shape.
    """
    moved_to = {
        old: new for new, old in changed.items() if old not in ("", new)
    }
    approvals = {}
    renames: list[str] = []
    for (file, name), text in load_settings(repo)[0].items():
        if Path(file).suffix not in suffixes:
            continue
        if file in moved_to:
            pair = f"{file}={moved_to[file]}"
            renames += [] if pair in renames else [pair]
            file = moved_to[file]
        approvals[file, name] = text
    return approvals, renames


def move_renamed_approvals(
    approvals: dict[tuple[str, str], str],
    named: list[Comment],
    comments: list[Comment],
) -> tuple[dict[tuple[str, str], str], dict[tuple[str, str], list[str]]]:
    """Move each gone approval to its one likely new name, in place.

    A name that has an approval of its own keeps it.

    Returns:
        Each moved approval's old name by its new ``(file, name)``, and
        the likely new names of every gone approval.
    """
    current = {(c.file, c.name) for c in named}
    gone = sorted(k for k, t in approvals.items() if t and k not in current)
    # Every likely name is found before any approval moves.
    likely_names = {
        (file, name): [
            c.name
            for c in find_rename_candidates(
                name,
                approvals[file, name],
                file,
                comments,
                approvals,
            )
        ]
        for file, name in gone
    }
    renamed = {}
    for file, name in gone:
        if len(likely_names[file, name]) == 1:
            new_key = (file, likely_names[file, name][0])
            if new_key not in approvals:
                approvals[new_key] = approvals.pop((file, name))
                renamed[new_key] = name
    return renamed, likely_names


def list_comment_items(
    named: list[Comment],
    approvals: dict[tuple[str, str], str],
    renamed: dict[tuple[str, str], str],
    text_at_base: dict[tuple[str, str], str],
    changed: dict[str, str],
) -> tuple[list[dict[str, object]], list[str]]:
    """Return an item for each comment now that the owner must answer.

    A comment the checker must not write over is left out, and a comment
    the owner cut is a problem.

    Returns:
        The items, and the problems to fix first.
    """
    items: list[dict[str, object]] = []
    problems = []
    for c in named:
        key = (c.file, c.name)
        if not LANGUAGES[Path(c.file).suffix].is_writable(c):
            continue
        if approvals.get(key) == "":
            problems.append(
                f"{c.file} {c.name}: the owner cut this comment. Delete it "
                "from the code."
            )
            continue
        if key in renamed:
            reason = "approved_renamed"
        elif key in approvals:
            if approvals[key] == c.text:
                continue
            reason = "approved_edited"
        elif c.file in changed and text_at_base.get(key) != c.text:
            reason = "changed" if key in text_at_base else "new"
        else:
            continue
        items.append(
            {
                "file": c.file,
                "name": c.name,
                "reason": reason,
                "old": approvals.get(key, c.text),
                "current": c.text,
                "can_cut": not c.only_body,
            }
        )
    return items, problems


def list_gone_approval_items(
    approvals: dict[tuple[str, str], str],
    named: list[Comment],
    unapprovable: set[tuple[str, str]],
    likely_names: dict[tuple[str, str], list[str]],
) -> tuple[list[dict[str, object]], list[str]]:
    """Return a ``code_removed`` item for each approved comment now gone.

    A gone comment whose name is now shared, or that may have been
    renamed, is a problem instead, since the owner's text has no sure
    place.

    Returns:
        The items, and the problems to fix first.
    """
    current = {(c.file, c.name) for c in named}
    items: list[dict[str, object]] = []
    problems = []
    for key, text in sorted(approvals.items()):
        if not text or key in current:
            continue
        place = f"{key[0]} {key[1]}"
        if key in unapprovable:
            problems.append(
                f"{place}: the owner approved this comment, and now more "
                "than one comment in the file has its name, so none of them "
                "can be told apart. Keep a comment on only one of them."
            )
        elif likely_names[key]:
            problems.append(
                f"{place}: the owner approved this comment, and it is gone. "
                f"It may now be {' or '.join(likely_names[key])}, but that "
                "is not certain, so none is guessed. Reword the first "
                "sentence of each comment that is not the renamed one."
            )
        else:
            items.append(
                {
                    "file": key[0],
                    "name": key[1],
                    "reason": "code_removed",
                    "old": text,
                }
            )
    return items, problems


def find_owner_changes(repo: Path, base: str) -> tuple[dict[str, object], int]:
    """Return what the owner must answer for the changes since ``base``.

    An item's ``reason`` is ``new`` or ``changed`` for a comment new or
    changed since ``base``, ``approved_edited`` or ``approved_renamed``
    for an approved comment edited or under a new name, or
    ``code_removed`` for an approved comment whose code is gone. Only
    languages with owner reviews on are read.

    Returns:
        The reply without its interface number, and the exit code.

    Raises:
        OSError: A code file or ``.code-comments.json`` cannot be opened.
        ValueError: Git fails, ``.code-comments.json`` is not of its
            shape, or a code file is not UTF-8.
        SourceError: A file cannot be parsed.
    """
    all_changed = list_changed_files(repo, base)
    reviewed = {s for s, language in LANGUAGES.items() if language.reviews_on}
    changed = {
        path: old
        for path, old in all_changed.items()
        if Path(path).suffix in reviewed
    }
    approvals, renames = read_moved_approvals(repo, changed, reviewed)
    files = set(changed) | {file for file, _ in approvals}
    sources = {
        file: (repo / file).read_text(encoding="utf-8")
        for file in files
        if (repo / file).exists()
    }
    named, comments, unapprovable = read_source_comments(sources)
    base_sources = {
        file: run_git(repo, "show", f"{base}:./{old}")
        for file, old in changed.items()
        if old
    }
    text_at_base = {
        (c.file, c.name): c.text for c in read_source_comments(base_sources)[0]
    }
    renamed, likely_names = move_renamed_approvals(approvals, named, comments)
    items, problems = list_comment_items(
        named,
        approvals,
        renamed,
        text_at_base,
        changed,
    )
    gone_items, gone_problems = list_gone_approval_items(
        approvals,
        named,
        unapprovable,
        likely_names,
    )
    problems += gone_problems
    renames += [f"{old}={new}" for (_, new), old in renamed.items()]
    unsupported = [
        path
        for path in all_changed
        if Path(path).suffix in UNSUPPORTED_SUFFIXES
    ]
    reply: dict[str, object] = {
        "base": base,
        "items": [] if problems else items + gone_items,
        "renames": [] if problems else sorted(renames),
        "problems": sorted(problems),
        "unsupported": sorted(unsupported),
    }
    return reply, 1 if problems else 0
