"""Check the owner's answers, then write and approve them.

Example:
    {"interface": 1, "renames": ["Total=Sum"], "answers": [
     {"id": 1, "file": "shop/cart.go", "name": "Sum",
      "old": "// Sums.\\n", "current": "// Adds.\\n", "text": "// Sums.\\n"},
     {"id": 2, "file": "shop/price.go", "name": "Price",
      "old": "// In cents.\\n", "code_removed": true, "cut": true}]}

``id`` is the review's item number, which problems name. ``current``
is the comment as the review saw it in the file, and an answer without
a ``name`` is found by that text. An answer gives the final ``text`` or
``"cut": true``. ``old`` is the text the owner was shown as the
comment's, the approved text for an approved comment, and
``code_removed`` marks one whose code is gone. Renames move approvals
first, as ``--approve-comment`` moves them.
"""

from __future__ import annotations

import json
import os
import re
import signal
import stat
import tempfile
from dataclasses import dataclass, field, replace
from pathlib import Path, PurePosixPath
from typing import TextIO

from . import INTERFACE
from .approvals import (
    SETTINGS,
    apply_renames,
    load_settings,
    replace_settings_file,
    unname_unapprovable_comments,
)
from .languages import LANGUAGES, Language
from .records import Comment, SourceError


@dataclass(frozen=True)
class Answer:
    """One answer of an ``apply`` request; ``text`` is ``""`` for a cut."""

    id: int
    file: str
    name: str
    old: str
    current: str
    text: str
    code_removed: bool


@dataclass(frozen=True)
class Place:
    """Where an answer's ``current`` text sits in its file.

    ``comment`` is the whole comment holding it, and ``name`` the name
    an approval takes, ``""`` when the comment has none.
    """

    start: int
    comment: Comment
    name: str


@dataclass(frozen=True)
class Write:
    """What ``apply`` writes once every answer passes.

    ``originals`` holds the bytes the check read of each file to write,
    ``.code-comments.json`` included, ``None`` for one that was absent.
    ``first_ids`` gives a refusal its item, and ``entries`` is ``None``
    when no answer names a comment, leaving the approvals file alone.
    """

    changed: dict[str, str] = field(default_factory=dict)
    entries: list[dict[str, str]] | None = None
    originals: dict[str, bytes | None] = field(default_factory=dict)
    first_ids: dict[str, int] = field(default_factory=dict)


def read_apply_request(
    stream: TextIO,
) -> tuple[dict[str, str], list[Answer]]:
    """Return the renames and answers of the request ``apply`` reads.

    Raises:
        ValueError: The request is not of the shape the module docstring
            shows.
    """
    request = json.load(stream)
    if not (
        isinstance(request, dict)
        and {"interface", "answers"} <= set(request)
        and set(request) <= {"interface", "renames", "answers"}
        and request["interface"] == INTERFACE
        and isinstance(request.get("renames", []), list)
        and isinstance(request["answers"], list)
    ):
        raise ValueError(
            f'apply reads {{"interface": {INTERFACE}, "renames": [...], '
            '"answers": [...]}'
        )
    renames = {}
    for pair in request.get("renames", []):
        old, _, new = str(pair).partition("=")
        if not (isinstance(pair, str) and old and new):
            raise ValueError(f"rename {pair!r} must be OLD=NEW")
        renames[old] = new
    return renames, [read_answer(entry) for entry in request["answers"]]


def read_answer(entry: object) -> Answer:
    """Return one answer of an ``apply`` request.

    Raises:
        ValueError: The answer is not of the shape the module docstring
            shows, or its file is not a plain path inside the repository.
    """
    fields = entry if isinstance(entry, dict) else {}
    strings = ("file", "name", "old", "current", "text")
    removed = "code_removed" in fields
    # Removed code has no current text, so it needs a name and an old.
    needed = ("cut", "name", "old") if removed else ("current",)
    is_valid = (
        bool(fields)
        and set(fields) <= {"id", *strings, "cut", "code_removed"}
        and {"id", "file", "old"} <= set(fields)
        and type(fields["id"]) is int
        and all(isinstance(fields.get(key, ""), str) for key in strings)
        and fields.get("code_removed", True) is True
        and ("text" in fields) != (fields.get("cut") is True)
        and all(fields.get(key) for key in needed)
    )
    # "./a.go" and "a.go" would be written separately, losing an edit.
    path = PurePosixPath(fields["file"] if is_valid else ".")
    if not (
        is_valid
        and path.as_posix() == fields["file"]
        and not path.is_absolute()
        and ".." not in path.parts
    ):
        raise ValueError(
            "each answer needs an id, a plain relative file, old, current "
            "unless code_removed, and text or cut true; code_removed needs "
            f"a name, cut and an old: {json.dumps(entry)}"
        )
    return Answer(
        id=fields["id"],
        file=fields["file"],
        name=fields.get("name", ""),
        old=fields["old"],
        current=fields.get("current", ""),
        text=fields.get("text", ""),
        code_removed=removed,
    )


def check_answers(
    answers: list[Answer], renames: dict[str, str], repo: Path
) -> tuple[Write, list[str]]:
    """Return what writing and approving the answers changes, or problems.

    Nothing is written. Every problem is reported, not only the first.

    Returns:
        What to write, empty when there is a problem, and the problems.

    Raises:
        OSError: A file cannot be read, or ``gofmt`` is not on ``PATH``.
        ValueError: A file is not UTF-8, or ``.code-comments.json`` is
            not of its shape.
    """
    settings = repo / SETTINGS
    # Read before the approvals are, so a change made later is caught.
    settings_bytes = settings.read_bytes() if settings.exists() else None
    problems: list[str] = []
    files: dict[str, tuple[str, list[Comment]] | None] = {}
    places: dict[int, Place] = {}
    named_answers: list[tuple[Answer, str]] = []
    first_ids: dict[tuple[str, object], int] = {}
    for index, answer in enumerate(answers):
        language = LANGUAGES.get(Path(answer.file).suffix)
        if not (language and language.reviews_on and language.finish_rewrite):
            problems.append(
                f"item {answer.id}: {answer.file}: owner reviews are off for "
                "this language, so the checker writes no answer in it"
            )
            continue
        if answer.file not in files:
            try:
                files[answer.file] = read_file_comments(repo, answer.file)
            except SourceError as error:
                problems.append(f"{answer.file}: {error}")
                files[answer.file] = None
        read = files[answer.file]
        if read is None:
            continue
        try:
            place = find_answer_place(answer, *read, language)
        except SourceError as error:
            problems.append(str(error))
            continue
        name = place.name if place else answer.name
        if place:
            places[index] = place
        if name:
            named_answers.append((answer, name))
        at = (answer.file, name or (place.start if place else ""))
        if at in first_ids:
            problems.append(
                f"items {first_ids[at]} and {answer.id} are the same comment "
                f"in {answer.file}; keep only one of them"
            )
        first_ids.setdefault(at, answer.id)
    if problems:
        return Write(), problems
    comments = {file: read[1] for file, read in files.items() if read}
    problems = find_approval_problems(repo, renames, named_answers, comments)
    if problems:
        return Write(), problems
    changed, problems = build_new_sources(answers, places, files)
    if problems:
        return Write(), problems
    sources = {file: read[0] for file, read in files.items() if read}
    originals: dict[str, bytes | None] = {
        file: sources[file].encode() for file in changed
    }
    file_ids: dict[str, int] = {}
    for answer in answers:
        file_ids.setdefault(answer.file, answer.id)
    entries = None
    if named_answers:
        entries = approve_answers(
            repo, renames, named_answers, {**sources, **changed}
        )
        originals[SETTINGS] = settings_bytes
    return Write(changed, entries, originals, file_ids), []


def read_file_comments(repo: Path, file: str) -> tuple[str, list[Comment]]:
    """Return a file's source and every comment in it, ``""`` when gone.

    Raises:
        SourceError: The file has a carriage return or does not parse.
        OSError: The file cannot be read.
        ValueError: The file is not UTF-8.
    """
    path = repo / file
    if not path.exists():
        return "", []
    # read_text would turn a carriage return into a line feed.
    source = path.read_bytes().decode("utf-8")
    if "\r" in source:
        raise SourceError("cannot be read (it has a carriage return)")
    return source, LANGUAGES[path.suffix].read_comments(source, file)


def find_answer_place(
    answer: Answer, source: str, comments: list[Comment], language: Language
) -> Place | None:
    """Return where an answer's comment is, or ``None`` for removed code.

    Raises:
        SourceError: The comment is not as the review saw it, or the
            answer could not be written safely.
    """
    starts = [0] + [line_end.end() for line_end in re.finditer("\n", source)]
    named = unname_unapprovable_comments(comments)[0]
    by_name = {c.name: c for c in named if c.name}
    if answer.code_removed:
        if answer.name in by_name:
            raise SourceError(
                f"item {answer.id}: {answer.file} {answer.name}: its removed "
                "code is back. Send the item again without code_removed."
            )
        return None
    changed = SourceError(
        f"{answer.file}: item {answer.id} changed in the file after it was "
        "added; ask the AI to send it again"
    )
    if answer.name:
        comment = by_name.get(answer.name)
        if comment is None or comment.text != answer.current:
            raise changed
        start = starts[comment.line - 1]
        place = Place(start, comment, answer.name)
    elif answer.current not in source:
        raise changed
    else:
        try:
            place = find_text_once(
                source, answer.current, list(zip(comments, named)), starts
            )
        except SourceError as error:
            raise SourceError(f"item {answer.id}: {answer.file}: {error}")
        rest = place.comment.text.replace(answer.current, "", 1)
        # A directive line, such as `//go:noinline`, is not part of the prose.
        if any(
            language.is_writable(replace(place.comment, text=line))
            for line in rest.splitlines(keepends=True)
        ):
            raise SourceError(
                f"item {answer.id}: {answer.file}: is only part of a "
                "comment: a comment line is just above or below it"
            )
    where = f"item {answer.id}: {answer.file} {place.name}".rstrip()
    if not language.is_writable(replace(place.comment, text=answer.current)):
        raise SourceError(
            f"{where}: a rewrite could change how the code builds or is "
            "checked"
        )
    # A new directive, such as `//go:noinline`, would change the build too.
    if answer.text and not language.is_writable(
        replace(place.comment, text=answer.text)
    ):
        raise SourceError(
            f"{where}: the new text could change how the code builds or is "
            "checked"
        )
    if not answer.text and place.comment.only_body:
        raise SourceError(
            f"{where}: the docstring is its body's only statement, so "
            "cutting it would break the code"
        )
    return place


def find_text_once(
    source: str,
    text: str,
    comments: list[tuple[Comment, Comment]],
    starts: list[int],
) -> Place:
    """Return where ``text`` is, found by the text alone.

    ``text`` must be found once, start a line and lie inside one comment.
    It takes that comment's name only when it is the whole comment.

    Args:
        comments: Each comment, and the same comment as approvals name it.
        starts: Where each line of ``source`` starts.

    Raises:
        SourceError: ``text`` is not such a comment.
    """
    count = source.count(text)
    if count > 1:
        lines = [
            str(source.count("\n", 0, match.start()) + 1)
            for match in re.finditer(re.escape(text), source)
        ]
        raise SourceError(
            f"comment found {count} times, at lines {', '.join(lines)}; the "
            "checker cannot tell the copies apart"
        )
    start = source.index(text)
    if start and source[start - 1] != "\n":
        raise SourceError("comment does not start a line")
    if not text.endswith("\n"):
        raise SourceError("must be whole comment lines ending in a newline")
    for comment, approvable in comments:
        comment_start = starts[comment.line - 1]
        if comment_start <= start < comment_start + len(comment.text):
            break
    else:
        raise SourceError(
            "is not a comment when the checker reads the file, such as text "
            "inside a string"
        )
    if start + len(text) > comment_start + len(comment.text):
        raise SourceError("holds lines that are not part of the comment")
    name = approvable.name if text == comment.text else ""
    return Place(start, comment, name)


def find_approval_problems(
    repo: Path,
    renames: dict[str, str],
    named_answers: list[tuple[Answer, str]],
    comments: dict[str, list[Comment]],
) -> list[str]:
    """Return why approving the answers could approve text the owner never saw.

    Args:
        named_answers: Each answer of a named comment, with that name.
        comments: Every comment of each answered file.

    Raises:
        ValueError: ``.code-comments.json`` is not of its shape.
    """
    approved = load_settings(repo)[0]
    files = {answer.file for answer, _ in named_answers}
    texts = {
        (c.file, c.name): c.text
        for file in files
        for c in unname_unapprovable_comments(comments[file])[0]
        if c.name
    }
    moved, unused = approved, list(renames)
    if files:
        moved, _, unused = apply_renames(
            renames, files, set(texts), approved, []
        )
    problems = []
    for answer, name in named_answers:
        key = (answer.file, name)
        if moved.get(key) == answer.old:
            continue
        where = f"item {answer.id}: {answer.file} {name}"
        if texts.get(key, "") != answer.old:
            problems.append(
                f"{where}: no comment by this name has the item's old. Fix "
                "or remove the item's name."
            )
        elif moved.get(key) == "":
            problems.append(
                f"{where}: the owner cut this comment. Delete it from the "
                "code."
            )
        elif key in moved:
            problems.append(
                f"{where}: differs from the text the owner approved. To "
                "change it, list it as an item with this as old:\n    "
                + indent_text(moved[key])
            )
    removed = {(a.file, name) for a, name in named_answers if a.code_removed}
    for (file, name), text in moved.items():
        key = (file, name)
        if text and file in files and key not in texts and key not in removed:
            problems.append(
                f"{file} {name}: the owner's approved comment is gone. If you "
                "renamed it, add the rename to renames; if you deleted its "
                "code, send it as an item with code_removed and this as "
                "old:\n    " + indent_text(text)
            )
    for old in unused:
        problems.append(
            f"rename {old} would move no approved comment, because its file "
            "has no item. Add an item for that comment in its file, or "
            "remove the rename if the comment was never approved"
        )
    return problems


def indent_text(text: str) -> str:
    """Return ``text`` with its later lines indented under a message."""
    return text.rstrip("\n").replace("\n", "\n    ")


def build_new_sources(
    answers: list[Answer],
    places: dict[int, Place],
    files: dict[str, tuple[str, list[Comment]] | None],
) -> tuple[dict[str, str], list[str]]:
    """Return each changed file's source as the write leaves it, or problems.

    Each file is refused unless its language's write check passes and,
    before that check's own layout, it reads back with each answered
    comment holding its text and every other comment unchanged.

    Args:
        places: Each located answer's place, by its index in ``answers``.

    Raises:
        OSError: ``gofmt`` is not on ``PATH``.
    """
    changed, problems = {}, []
    for file, read in files.items():
        assert read is not None
        source, comments = read
        edits = [
            (places[i], answers[i]) for i in places if answers[i].file == file
        ]
        new = source
        for place, answer in sorted(edits, key=lambda edit: -edit[0].start):
            end = place.start + len(answer.current)
            new = new[: place.start] + answer.text + new[end:]
        if new == source:
            continue
        expected = []
        for comment in comments:
            text = comment.text
            for place, answer in edits:
                if place.comment.line == comment.line:
                    text = text.replace(answer.current, answer.text, 1)
            expected += [(comment.name, text)] if text else []
        language = LANGUAGES[Path(file).suffix]
        assert language.finish_rewrite is not None
        try:
            finished = language.finish_rewrite(source, new, file)
        except SourceError as error:
            problems.append(str(error))
            continue
        written = language.read_comments(new, file)
        if [(c.name, c.text) for c in written] != expected:
            problems.append(
                f"{file}: the answers would not read back as the same "
                "comments with the new text; check that each text is whole "
                "comment lines"
            )
            continue
        changed[file] = finished
    return ({}, problems) if problems else (changed, [])


def approve_answers(
    repo: Path,
    renames: dict[str, str],
    named_answers: list[tuple[Answer, str]],
    sources: dict[str, str],
) -> list[dict[str, str]]:
    """Return the entries ``.code-comments.json`` gets for the answers.

    Each answer is approved in turn after the renames, as by a run of
    ``--approve-comment``, and a comment gone from its file is cut.

    Args:
        sources: Each answered file's source as the write would leave it,
            ``""`` when the file is gone.

    Raises:
        ValueError: ``.code-comments.json`` is not of its shape.
    """
    approved = load_settings(repo)[0]
    for answer, name in named_answers:
        file = answer.file
        language = LANGUAGES[Path(file).suffix]
        comments = unname_unapprovable_comments(
            language.read_comments(sources[file], file)
        )[0]
        keys = {(c.file, c.name) for c in comments}
        approved = apply_renames(renames, {file}, keys, approved, [])[0]
        text = next((c.text for c in comments if c.name == name), "")
        approved[file, name] = text
    entries = [
        {"file": f, "name": n, "text": t} for (f, n), t in approved.items()
    ]
    return sorted(entries, key=lambda entry: entry["file"])


def write_answers(repo: Path, write: Write) -> list[str]:
    """Write the owner's new comments into the files and approve them.

    If anything goes wrong partway, the checker puts every file back. That
    includes Ctrl-C and other stop signals. Once the last file is saved, the
    change stays even if a signal arrives. A forced kill, or a disk error while
    putting files back, can still leave some files changed.

    Args:
        repo: The repository folder.
        write: The new file contents and approvals, from the earlier check.

    Returns:
        Reasons the files cannot be written, or an empty list on success.

    Raises:
        KeyboardInterrupt: A signal arrived before the last file was saved.
        OSError: A file could not be saved or put back.
    """
    problems = []
    for name, original in write.originals.items():
        path = repo / name
        if (path.read_bytes() if path.exists() else None) == original:
            continue
        if name == SETTINGS:
            problems.append(
                f"{SETTINGS} changed after the answers were checked; run "
                "apply again"
            )
        else:
            problems.append(
                f"{name}: item {write.first_ids[name]} changed in the file "
                "after it was added; ask the AI to send it again"
            )
    if problems:
        return problems
    avoid = load_settings(repo)[1]
    temps: list[Path] = []
    replaced: list[str] = []
    signals = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    handlers = {}
    interrupted = None

    def record_interruption(signum, frame) -> None:
        """Record the first signal so the write can finish or roll back."""
        nonlocal interrupted
        if interrupted is None:
            interrupted = signum

    def stop_if_interrupted() -> None:
        if interrupted is not None:
            raise KeyboardInterrupt(f"signal {interrupted}")

    try:
        for sig in signals:
            handlers[sig] = signal.signal(sig, record_interruption)
        for file, source in write.changed.items():
            temps.append(write_beside(repo / file, source.encode()))
            stop_if_interrupted()
        try:
            for file, temp in zip(write.changed, temps):
                # Listed first, so Ctrl-C just after the rename puts it back.
                replaced.append(file)
                os.replace(temp, repo / file)
                stop_if_interrupted()
            if write.entries is not None:
                replaced.append(SETTINGS)
                replace_settings_file(repo, write.entries, avoid)
            # This check commits the write; later signals cannot abort cleanup.
            stop_if_interrupted()
        except BaseException as error:
            failed = put_back_files(repo, replaced, write.originals)
            if failed:
                raise OSError(
                    f"{error}; putting them back failed, so these may still "
                    f"hold the answers: {', '.join(failed)}"
                ) from error
            raise
    finally:
        try:
            for temp in temps:
                temp.unlink(missing_ok=True)
        finally:
            mask = signal.pthread_sigmask(signal.SIG_BLOCK, signals)
            try:
                for sig, handler in handlers.items():
                    signal.signal(sig, handler)
                # This snapshot hands later signals back to the caller.
                pending = signal.sigpending() & (set(signals) - mask)
                for sig in pending:
                    signal.sigwait({sig})
            finally:
                signal.pthread_sigmask(signal.SIG_SETMASK, mask)
    return []


def put_back_files(
    repo: Path, names: list[str], originals: dict[str, bytes | None]
) -> list[str]:
    """Put each file back as the check read it, deleting one that was absent.

    Every file is tried, even after one fails.

    Returns:
        The files that could not be put back.
    """
    failed = []
    for name in names:
        path, original = repo / name, originals[name]
        try:
            if original is None:
                path.unlink(missing_ok=True)
                continue
            temp = write_beside(path, original)
            try:
                os.replace(temp, path)
            finally:
                temp.unlink(missing_ok=True)
        except OSError:
            failed.append(name)
    return failed


def write_beside(path: Path, data: bytes) -> Path:
    """Write ``data`` to a new file beside ``path``, with ``path``'s mode.

    Returns:
        The new file.

    Raises:
        OSError: The file cannot be written.
    """
    handle, name = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
    temp = Path(name)
    try:
        with os.fdopen(handle, "wb") as out:
            out.write(data)
        os.chmod(temp, stat.S_IMODE(path.stat().st_mode))
    except BaseException:
        temp.unlink(missing_ok=True)
        raise
    return temp
