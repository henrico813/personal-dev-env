"""Read, write and merge .code-comments.json."""

from __future__ import annotations

import argparse
import json
import os
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import TypeGuard

from .languages import LANGUAGES
from .records import Comment, SourceError

SETTINGS = ".code-comments.json"


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


def read_files_at_commit(
    repo: Path, rev: str, suffixes: set[str]
) -> dict[str, str]:
    """Read every file at ``rev`` whose name ends in one of ``suffixes``.

    Every such file under ``repo`` counts, not only the checked ones, so
    a person's sentence in a moved or renamed file is still theirs.

    Args:
        repo: The folder given as ``--repo``, inside a Git repository.
        suffixes: The checked files' suffixes, such as ``.go``.

    Returns:
        Each file's source by its path relative to ``repo``.

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
    sources = {}
    for file in listed:
        if Path(file).suffix not in suffixes:
            continue
        # "./" makes Git read the path from --repo, not the Git root.
        shown = subprocess.run(
            [*git, "show", f"{rev}:./{file}"], capture_output=True
        ).stdout
        # An old file need not be UTF-8; only its comments matter.
        sources[file] = shown.decode("utf-8", errors="replace")
    return sources


def read_comments_in_files(sources: dict[str, str]) -> list[Comment]:
    """Read the comments in files read by ``read_files_at_commit``.

    Returns:
        The comments, with shared names removed. A file its parser
        cannot read is left out, since its sentences only decide which
        wording is old.
    """
    found = []
    for file, source in sources.items():
        try:
            found += LANGUAGES[Path(file).suffix].read_comments(source, file)
        except SourceError:
            continue
    return unname_unapprovable_comments(found)[0]


def list_function_names(sources: dict[str, str]) -> set[str]:
    """Return the name of each function in files whose names are checked."""
    names = set()
    for file, source in sources.items():
        list_functions = LANGUAGES[Path(file).suffix].list_functions
        if list_functions:
            names |= {f.name for f in list_functions(source, file)}
    return names


def unname_unapprovable_comments(
    comments: list[Comment],
) -> tuple[list[Comment], dict[tuple[str, str], tuple[str, list[int]]]]:
    """Take the name off comments that cannot keep one.

    Two ``func init()`` or two ``var _`` lines in one file are ordinary
    Go, but approved text could not tell their comments apart. A
    comment with an ``unapprovable_reason`` loses its name too.

    Returns:
        The comments, with those names set to ``""``, and each such
        name's reason and lines by ``(file, name)``.
    """
    by_key: dict[tuple[str, str], list[Comment]] = {}
    for comment in comments:
        if comment.name:
            by_key.setdefault((comment.file, comment.name), []).append(comment)
    unapprovable = {}
    for key, same_name_comments in by_key.items():
        lines = [c.line for c in same_name_comments]
        reason = same_name_comments[0].unapprovable_reason
        if len(lines) > 1:
            reason = f"{len(lines)} comments in this file have this name"
        if reason:
            unapprovable[key] = (reason, lines)
    unnamed = [
        replace(c, name="") if (c.file, c.name) in unapprovable else c
        for c in comments
    ]
    return unnamed, unapprovable


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

    A pair whose old side is a file in the entries, in ``files`` or at
    ``--base`` moves that file; other pairs rename a comment. A suffix
    cannot tell a file from a name, since a Python name such as
    ``Runner.go`` looks like a path. Files move first, and an entry
    already under the new name wins.

    Args:
        renames: New names or paths by old ones.
        current_keys: ``(file, name)`` of every comment in ``files``.

    Returns:
        The renamed entries and ``--base`` comments, and each old name
        or path that renamed nothing.
    """
    known_files = files | {file for file, _ in approved}
    known_files |= {comment.file for comment in base_comments}
    rename_rules = RenameRules(
        file_moves={o: n for o, n in renames.items() if o in known_files},
        name_changes={
            o: n for o, n in renames.items() if o not in known_files
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


def approve_one_comment(
    repo: Path, file: str, name: str, renames: dict[str, str]
) -> int:
    """Record one comment's text as approved, after the renames.

    ``--approve`` would approve every comment in the file, so a comment
    review approves each answer with this instead.

    Args:
        repo: The folder holding ``.code-comments.json``.
        file: The Go or Python file, relative to ``repo``.
        name: The comment's name; a comment gone from ``file`` is
            recorded as cut.
        renames: New names or paths by old ones, applied first. A name
            pair renames an entry only in ``file``.

    Returns:
        The exit code described in the module docstring.
    """
    try:
        file = (repo / file).resolve().relative_to(repo).as_posix()
        approved, avoid = load_settings(repo)
        path = repo / file
        source = path.read_text(encoding="utf-8") if path.exists() else ""
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 2
    language = LANGUAGES.get(Path(file).suffix)
    if language is None or not name:
        suffixes = " or ".join(LANGUAGES)
        print(
            f"--approve-comment needs a {suffixes} file and a comment name, "
            f"not {file!r} {name!r}",
            file=sys.stderr,
        )
        return 2
    try:
        found = language.read_comments(source, file)
    except SourceError as error:
        print(f"{file}: {error}", file=sys.stderr)
        return 2
    comments, unapprovable = unname_unapprovable_comments(found)
    if (file, name) in unapprovable:
        reason, _ = unapprovable[file, name]
        print(f"{file} {name}: cannot be approved; {reason}", file=sys.stderr)
        return 2
    approved, _, _ = apply_renames(
        renames,
        {file},
        {(c.file, c.name) for c in comments},
        approved,
        [],
    )
    # A mistyped or moved FILE must not record a cut.
    if not path.exists() and (file, name) not in approved:
        print(
            f"{file}: no such file, and no approval of {name} in it",
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


def list_changed_files(repo: Path, commit: str) -> set[str]:
    """Return the Go and Python files ``commit`` changed, deleted ones too.

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
    return {file for file in changed if Path(file).suffix in LANGUAGES}


def read_named_comments(
    repo: Path, files: set[str]
) -> dict[tuple[str, str], str]:
    """Return the text of each approvable comment in ``files``.

    Raises:
        SourceError: Python cannot read one of ``files``.
    """
    comments: list[Comment] = []
    for file in sorted(files):
        source = (repo / file).read_text(encoding="utf-8")
        language = LANGUAGES[Path(file).suffix]
        try:
            comments += language.read_comments(source, file)
        except SourceError as error:
            raise SourceError(f"{file}: {error}") from error
    return {
        (c.file, c.name): c.text
        for c in unname_unapprovable_comments(comments)[0]
        if c.name
    }


def merge_approvals(repo: Path, folder: Path, commit: str) -> int:
    """Take ``folder``'s approvals for the files ``commit`` changed.

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
        changed_files = list_changed_files(repo, commit)
        present_files = {f for f in changed_files if (repo / f).exists()}
        # Read only approved files, so another file cannot stop cleanup.
        approved_files = {file for file, _ in [*approved, *incoming]}
        comments_now = read_named_comments(
            repo, present_files & approved_files
        )
    except (ValueError, SourceError, subprocess.CalledProcessError) as error:
        print(error, file=sys.stderr)
        return 2
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
