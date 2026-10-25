"""Check the owner's answers before they are written into the code.

Example:
    {"interface": 1, "answers": [
     {"id": 1, "file": "shop/cart.go", "name": "Total",
      "current": "// the agent's edit\\n", "text": "// Sums.\\n"},
     {"id": 2, "file": "shop/cart.go", "current": "// x\\n", "cut": true}]}

``id`` is the review's item number, which problems name. ``current``
is the comment as the review saw it in the file, and an answer without
a ``name`` is found by that text. An answer gives the final ``text`` or
``"cut": true``.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, replace
from pathlib import Path, PurePosixPath
from typing import TextIO

from . import INTERFACE
from .approvals import unname_unapprovable_comments
from .languages import LANGUAGES, Language
from .records import Comment, SourceError


@dataclass(frozen=True)
class Answer:
    """One answer of an ``apply`` request; ``text`` is ``""`` for a cut."""

    id: int
    file: str
    name: str
    current: str
    text: str


@dataclass(frozen=True)
class Place:
    """Where an answer's ``current`` text sits in its file.

    ``comment`` is the whole comment holding it, and ``name`` the name
    an approval takes, ``""`` when the comment has none.
    """

    start: int
    comment: Comment
    name: str


def read_apply_request(stream: TextIO) -> list[Answer]:
    """Return the answers of the request ``apply`` reads.

    Raises:
        ValueError: The request is not of the shape the module docstring
            shows.
    """
    request = json.load(stream)
    if not (
        isinstance(request, dict)
        and set(request) == {"interface", "answers"}
        and request["interface"] == INTERFACE
        and isinstance(request["answers"], list)
    ):
        raise ValueError(
            f'apply reads {{"interface": {INTERFACE}, "answers": [...]}}'
        )
    return [read_answer(entry) for entry in request["answers"]]


def read_answer(entry: object) -> Answer:
    """Return one answer of an ``apply`` request.

    Raises:
        ValueError: The answer is not of the shape the module docstring
            shows, or its file is not a plain path inside the repository.
    """
    fields = entry if isinstance(entry, dict) else {}
    strings = ("file", "name", "current", "text")
    is_valid = (
        bool(fields)
        and set(fields) <= {"id", *strings, "cut"}
        and {"id", "file", "current"} <= set(fields)
        and type(fields["id"]) is int
        and all(isinstance(fields.get(key, ""), str) for key in strings)
        and fields["current"] != ""
        and ("text" in fields) != (fields.get("cut") is True)
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
            "each answer needs an id, a plain relative file, current, and "
            f"text or cut true: {json.dumps(entry)}"
        )
    return Answer(
        id=fields["id"],
        file=fields["file"],
        name=fields.get("name", ""),
        current=fields["current"],
        text=fields.get("text", ""),
    )


def check_answers(
    answers: list[Answer], repo: Path
) -> tuple[list[str], list[str]]:
    """Return each file the answers change, or the problems.

    Nothing is written. Every problem is reported, not only the first.

    Raises:
        OSError: A file cannot be read, or ``gofmt`` is not on ``PATH``.
        ValueError: A file is not UTF-8.
    """
    problems: list[str] = []
    files: dict[str, tuple[str, list[Comment]] | None] = {}
    places: dict[int, Place] = {}
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
            place = places[index] = find_answer_place(answer, *read, language)
        except SourceError as error:
            problems.append(str(error))
            continue
        at = (answer.file, place.name or place.start)
        if at in first_ids:
            problems.append(
                f"items {first_ids[at]} and {answer.id} are the same comment "
                f"in {answer.file}; keep only one of them"
            )
        first_ids.setdefault(at, answer.id)
    if problems:
        return [], problems
    changed, problems = build_new_sources(answers, places, files)
    return sorted(changed), problems


def read_file_comments(repo: Path, file: str) -> tuple[str, list[Comment]]:
    """Return a file's source and every comment in it.

    Raises:
        SourceError: The file has a carriage return or does not parse.
        OSError: The file cannot be read.
        ValueError: The file is not UTF-8.
    """
    path = repo / file
    # read_text would turn a carriage return into a line feed.
    source = path.read_bytes().decode("utf-8")
    if "\r" in source:
        raise SourceError("cannot be read (it has a carriage return)")
    return source, LANGUAGES[path.suffix].read_comments(source, file)


def find_answer_place(
    answer: Answer, source: str, comments: list[Comment], language: Language
) -> Place:
    """Return where an answer's comment is in ``source``.

    Raises:
        SourceError: The comment is not as the review saw it, or the
            answer could not be written safely.
    """
    starts = [0] + [line_end.end() for line_end in re.finditer("\n", source)]
    named = unname_unapprovable_comments(comments)[0]
    by_name = {c.name: c for c in named if c.name}
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
