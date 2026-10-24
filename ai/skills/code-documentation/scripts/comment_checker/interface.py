"""Answer pde-comment-review's commands with one JSON object each.

Example:
    pde-comment-checker version
    pde-comment-checker list --interface 1 --repo . shop/cart.go
    pde-comment-checker changes --interface 1 --repo . --base 1a2b3c4

Every reply carries ``"interface": 1``. The exit code is 0 for a reply,
1 for a refusal whose reasons are in the reply's ``problems``, and 2 for
a usage or setup error, printed on stderr with no reply.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from . import INTERFACE, VERSION
from .approvals import unname_unapprovable_comments
from .changes import find_owner_changes
from .languages import LANGUAGES, UNSUPPORTED_SUFFIXES
from .records import Comment, SourceError

COMMANDS = ("version", "list", "changes")


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
    try:
        reply, code = find_owner_changes(repo, args.base)
    except (OSError, ValueError, SourceError) as error:
        print(error, file=sys.stderr)
        return 2
    print_reply(reply)
    return code
