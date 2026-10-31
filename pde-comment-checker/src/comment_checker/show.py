"""Turn comments into the words the owner edits and the rows shown."""

from __future__ import annotations

import json
from pathlib import Path
from typing import TextIO

from .approvals import unname_unapprovable_comments
from .languages.rules import LANGUAGES, LanguageRules
from .records import Comment, SourceError


def show_comments(
    repo: Path, entries: list[dict[str, str]]
) -> tuple[dict[str, object], int]:
    """Return each entry's comment text, plain text and screen rows.

    Args:
        repo: The repository that holds each comment's file.
        entries: The comments to show, as the review tool sends them.

    Returns:
        The reply without its interface number, and the exit code.

    Raises:
        ValueError: A file is not of a language ``show`` takes, or
            cannot be read as UTF-8 text.
    """
    shown = []
    problems = []
    for entry in entries:
        file, name = entry["file"], entry.get("name", "")
        language = LANGUAGES.get(Path(file).suffix)
        if not (
            language and language.prepare_for_screen and language.build_comment
        ):
            raise ValueError(f"{file}: show does not take this language")
        text = entry.get("text", "")
        try:
            if "plain" in entry:
                old = find_layout_comment(repo, entry, language)
                # The new wording copies the old comment's layout, so
                # this call raises SourceError when the old text is not
                # a comment of this language.
                language.prepare_for_screen(old.text)
                text = language.build_comment(entry["plain"], old.text)
            # Plain text with no words gives no text, which cuts the comment.
            plain, screen = ("", [])
            if text:
                plain, screen = language.prepare_for_screen(text)
        except (OSError, UnicodeDecodeError) as error:
            raise ValueError(f"{file}: {error}") from error
        except SourceError as error:
            problems.append(f"{file} {name}".strip() + f": {error}")
            continue
        shown.append(
            {
                "file": file,
                "name": name,
                "text": text,
                "plain": plain,
                "screen": screen,
            }
        )
    if problems:
        return {"shown": [], "problems": problems}, 1
    return {"shown": shown}, 0


def find_layout_comment(
    repo: Path, entry: dict[str, str], language: LanguageRules
) -> Comment:
    """Return the comment whose layout an entry's ``plain`` takes.

    Raises:
        SourceError: There is no such comment, or a rewrite of it could
            change how the code builds or is checked.
    """
    file = entry["file"]
    if "text" in entry:
        comment = Comment(
            file=file, line=1, name="", text=entry["text"], prose=""
        )
    else:
        source = (repo / file).read_bytes().decode("utf-8")
        if "\r" in source:
            raise SourceError("cannot be read (it has a carriage return)")
        comments = unname_unapprovable_comments(
            language.read_comments(source, file)
        )[0]
        named = [c for c in comments if c.name == entry["name"]]
        if not named:
            raise SourceError("no comment has this name in the file")
        comment = named[0]
    if not language.is_writable(comment):
        raise SourceError(
            "a rewrite could change how the code builds or is checked"
        )
    return comment


def read_show_entries(stream: TextIO) -> list[dict[str, str]]:
    """Return the entries ``show`` reads, a JSON list.

    Raises:
        ValueError: An entry lacks a ``file``, or both a ``text`` and a
            ``name`` with ``plain``, or has a value that is not a string.
    """
    entries = json.load(stream)
    if not isinstance(entries, list):
        raise ValueError("show reads a JSON list of entries")
    for entry in entries:
        if not (
            isinstance(entry, dict)
            and set(entry) <= {"file", "name", "text", "plain"}
            and all(isinstance(value, str) for value in entry.values())
            and "file" in entry
            and ("text" in entry or (entry.get("name") and "plain" in entry))
        ):
            raise ValueError(
                "each show entry needs a file and a text, or a name and "
                f"plain: {json.dumps(entry)}"
            )
    return entries
