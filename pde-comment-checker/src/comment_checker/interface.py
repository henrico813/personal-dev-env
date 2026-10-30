"""Answer pde-comment-review's commands with one JSON object each.

Example:
    pde-comment-checker version
    pde-comment-checker list --interface 1 --repo . shop/cart.go
    pde-comment-checker changes --interface 1 --repo . --base 1a2b3c4
    echo '[{"file": "a.go", "text": "// Sums.\\n"}]' |
        pde-comment-checker show --interface 1 --repo .
    pde-comment-checker apply [--check] --interface 1 --repo . < answers.json

Every reply carries ``"interface": 1``. The exit code is 0 for a reply,
1 for a refusal whose reasons are in the reply's ``problems``, and 2 for
a usage or setup error, printed on stderr with no reply.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import TextIO

from . import INTERFACE, VERSION
from .apply import (
    check_answers,
    read_apply_request,
    write_reviewed_comments,
)
from .approvals import unname_unapprovable_comments
from .changes import find_owner_changes
from .languages import LANGUAGES, UNSUPPORTED_SUFFIXES, Language
from .records import Comment, SourceError

COMMANDS = ("version", "list", "changes", "show", "apply")


def build_parser() -> argparse.ArgumentParser:
    """Return the parser for the commands, each taking ``--interface``."""
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--repo", type=Path, default=Path("."))
    common.add_argument("--interface", type=int, metavar="NUMBER")
    parser = argparse.ArgumentParser(prog="pde-comment-checker")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version", parents=[common])
    listing = commands.add_parser("list", parents=[common])
    listing.add_argument("files", nargs="+", type=Path)
    changes = commands.add_parser("changes", parents=[common])
    changes.add_argument("--base", required=True, metavar="REF")
    commands.add_parser("show", parents=[common])
    applying = commands.add_parser("apply", parents=[common])
    applying.add_argument("--check", action="store_true")
    return parser


def list_comments_in_files(repo: Path, paths: list[Path]) -> int:
    """Print every comment the checker can locate in ``paths``.

    Each comment has its exact ``text``, whether the checker may write
    over it, and whether cutting it keeps the code valid. A file with a
    known code suffix the checker cannot read is marked ``unsupported``.

    Returns:
        The exit code described in the module docstring.
    """
    listed: list[dict[str, object]] = []
    comments: list[Comment] = []
    problems = []
    for path in paths:
        try:
            file = path.resolve().relative_to(repo).as_posix()
            # read_text would turn a carriage return into a line feed.
            data = path.read_bytes()
            if path.suffix in UNSUPPORTED_SUFFIXES:
                listed.append({"file": file, "unsupported": True})
                continue
            if path.suffix not in LANGUAGES:
                raise ValueError("not a file of a language the checker reads")
            source = data.decode("utf-8")
        except (OSError, ValueError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        try:
            if "\r" in source:
                raise SourceError("cannot be read (it has a carriage return)")
            comments += LANGUAGES[path.suffix].read_comments(source, file)
        except SourceError as error:
            problems.append(f"{file}: {error}")
            continue
        listed.append({"file": file, "language": LANGUAGES[path.suffix].name})
    if problems:
        print_reply({"files": [], "problems": problems})
        return 1
    comments = unname_unapprovable_comments(comments)[0]
    for entry in listed:
        if "language" in entry:
            entry["comments"] = [
                describe_comment(c)
                for c in comments
                if c.file == entry["file"]
            ]
    print_reply({"files": listed})
    return 0


def describe_comment(comment: Comment) -> dict[str, object]:
    """Return how ``list`` shows one comment."""
    language = LANGUAGES[Path(comment.file).suffix]
    return {
        "name": comment.name,
        "line": comment.line,
        "end_line": comment.line + comment.text.rstrip("\n").count("\n"),
        "code_line": comment.code_line,
        "text": comment.text,
        "writable": language.is_writable(comment),
        "can_cut": not comment.only_body,
    }


def show_comments(repo: Path, entries: list[dict[str, str]]) -> int:
    """Print each entry's comment text, plain text and screen rows.

    An entry gives a comment's exact ``text``, or the ``plain`` sentences
    to write. ``plain`` is laid out like the entry's ``text`` when it has
    one, or else like the comment now under ``name`` in ``file``.

    Returns:
        The exit code described in the module docstring.
    """
    shown = []
    problems = []
    for entry in entries:
        file, name = entry["file"], entry.get("name", "")
        language = LANGUAGES.get(Path(file).suffix)
        if not (
            language
            and language.prepare_for_screen
            and language.lay_out_wording
        ):
            print(f"{file}: show does not take this language", file=sys.stderr)
            return 2
        text = entry.get("text", "")
        try:
            if "plain" in entry:
                old = find_layout_comment(repo, entry, language)
                # Text that is not a comment of this language has no layout.
                language.prepare_for_screen(old.text)
                text = language.lay_out_wording(entry["plain"], old.text)
            # Plain text with no words gives no text, which cuts the comment.
            plain, screen = ("", [])
            if text:
                plain, screen = language.prepare_for_screen(text)
        except (OSError, UnicodeDecodeError) as error:
            print(f"{file}: {error}", file=sys.stderr)
            return 2
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
        print_reply({"shown": [], "problems": problems})
        return 1
    print_reply({"shown": shown})
    return 0


def find_layout_comment(
    repo: Path, entry: dict[str, str], language: Language
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


def print_reply(reply: dict[str, object]) -> None:
    """Print ``reply`` as one JSON object with the interface number."""
    print(json.dumps({"interface": INTERFACE, **reply}))


def main(argv: list[str]) -> int:
    """Run one command and print its reply.

    Args:
        argv: The command and its arguments, such as ``["version"]``.

    Returns:
        The exit code described in the module docstring.
    """
    args = build_parser().parse_args(argv)
    if args.interface is None and args.command != "version":
        print(f"{args.command} needs --interface {INTERFACE}", file=sys.stderr)
        return 2
    if args.interface not in (None, INTERFACE):
        print(
            f"pde-comment-checker speaks interface {INTERFACE}, not "
            f"{args.interface}; run pde-installer so pde-comment-review "
            "and the checker come from one install",
            file=sys.stderr,
        )
        return 2
    repo = args.repo.resolve()
    if args.command == "version":
        print_reply({"version": VERSION, "languages": sorted(LANGUAGES)})
        return 0
    if args.command == "list":
        return list_comments_in_files(repo, args.files)
    if args.command == "show":
        try:
            entries = read_show_entries(sys.stdin)
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        return show_comments(repo, entries)
    if args.command == "apply":
        try:
            renames, answers = read_apply_request(sys.stdin)
            write, problems = check_answers(answers, renames, repo)
            if not (problems or args.check):
                problems = write_reviewed_comments(repo, write)
        except (OSError, ValueError) as error:
            print(error, file=sys.stderr)
            return 2
        changed = [] if problems else sorted(write.changed)
        print_reply({"changed": changed, "problems": problems})
        return 1 if problems else 0
    try:
        reply, code = find_owner_changes(repo, args.base)
    except (OSError, ValueError, SourceError) as error:
        print(error, file=sys.stderr)
        return 2
    print_reply(reply)
    return code
