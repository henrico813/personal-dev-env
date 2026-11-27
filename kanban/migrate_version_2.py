#!/usr/bin/env python3
"""Move every card to the version 2 layout.

A card gains each layout section it lacks, such as Notes and Appendix,
and ``template_version: 2``. Its text stays as it was, apart from blank
lines at its end. A second run changes nothing. A card whose layout
sections are out of order or repeated is skipped, to be fixed by hand.

Run it after migrate_sections.py, which moves older cards to version 1.
A card that still has the old Why or Outcome section is skipped.

Usage::

    migrate_version_2.py [--vault NAME] [--apply --user]

Without --apply it only lists the cards it would change or skip.
--apply backs up each card before writing it and writes an undo.sh
beside the copies.
"""

import argparse
import shlex
import shutil
import sys
import tempfile
from pathlib import Path

import card

VERSION = "2"


def main(argv: list[str], projects: Path | None = None) -> int:
    """List the cards to migrate and to skip, and migrate them with --apply.

    Args:
        argv: The arguments after the program name.
        projects: The vault's Projects folder. When None, it asks
            ``nobs vault path`` for the vault.

    Returns:
        The exit code, 1 when a card was skipped or anything failed.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--vault", help="vault name for nobs vault path")
    parser.add_argument(
        "--apply",
        action="store_true",
        help="write the cards; without it, only list them",
    )
    parser.add_argument(
        "--user",
        action="store_true",
        help="the user asked for this; needed with --apply",
    )
    args = parser.parse_args(argv)
    try:
        if args.apply and not args.user:
            raise card.CardError(
                "--apply needs --user; the user runs it after review",
            )
        if projects is None:
            projects = card.find_projects_folder(args.vault)
        cards = card.load_all_cards(projects)
        changes, skipped = list_changes(cards)
        print(
            f"{len(changes)} of {len(cards)} cards to migrate,"
            f" {skipped} skipped",
        )
        finished = True
        if args.apply and changes:
            finished = apply_changes(projects, changes)
    except card.CardError as error:
        print(f"migrate_version_2: {error}", file=sys.stderr)
        return 1
    return 0 if finished and not skipped else 1


def list_changes(cards: list[card.Card]) -> tuple[list[card.NoteChange], int]:
    """Print each card to migrate or skip, and return the changes.

    Args:
        cards: Every card in the vault.

    Returns:
        The change for each card that differs, and the number skipped.
    """
    changes: list[card.NoteChange] = []
    skipped = 0
    for note in cards:
        try:
            new_text = migrate_text(note.text)
        except card.CardError as error:
            print(f"skip {note.key}: {error}")
            skipped += 1
            continue
        if new_text != note.text:
            print(f"migrate {note.key}")
            changes.append(
                card.NoteChange(note.path, note.path, new_text, note.mtime_ns),
            )
    return changes, skipped


def apply_changes(projects: Path, changes: list[card.NoteChange]) -> bool:
    """Back up and write each card, and write undo.sh for the ones written.

    It stops at the first card it cannot write, such as one the user
    saved during the run. A second run finishes the rest.

    Args:
        projects: The vault's Projects folder.
        changes: The cards to write.

    Returns:
        True when every card was written.

    Raises:
        card.CardError: The backup folder cannot be made.
    """
    try:
        card.STATE_DIR.mkdir(parents=True, exist_ok=True)
        backup = Path(tempfile.mkdtemp(prefix="migrate-", dir=card.STATE_DIR))
    except OSError as error:
        raise card.CardError(f"{error}, no card changed") from error
    print(f"backup: {backup}")
    written: list[card.NoteChange] = []
    try:
        for change in changes:
            try:
                copy = backup / change.path.relative_to(projects)
                copy.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(change.path, copy)
                wrote = card.write_safely(
                    change.path,
                    change.path.stem,
                    change.new_text,
                    change.mtime_ns,
                )
            except OSError as error:
                print(f"migrate_version_2: {error}", file=sys.stderr)
                wrote = False
            if not wrote:
                print(
                    f"migrate_version_2: stopped at {change.path.name};"
                    " run again to finish, or run the undo line below",
                    file=sys.stderr,
                )
                return False
            written.append(change)
        print(f"migrated {len(written)} cards")
        return True
    finally:
        # A stop or Ctrl-C leaves later cards alone, so undo puts back
        # only the written ones and keeps any edit to the rest.
        undo = card.write_undo_script(projects, backup, written)
        print(
            "undo.sh puts back the backups and drops edits made to those"
            " cards after this run",
        )
        print(f"undo: sh {shlex.quote(str(undo))}")


def migrate_text(text: str) -> str:
    """Return a card's text in the version 2 layout.

    Args:
        text: The card text.

    Returns:
        The new text, the same text for a complete version 2 card.

    Raises:
        card.CardError: The card still has the old Why or Outcome
            section, its layout sections are out of order or repeated,
            or it has no frontmatter.
    """
    lines = text.split("\n")
    if any(
        card.find_heading(lines, name) is not None
        for name in ("Why", "Outcome")
    ):
        raise card.CardError(
            "old Why or Outcome section; run migrate_sections.py first",
        )
    known = [name.lower() for name in card.SECTIONS]
    start = card.find_frontmatter_end(lines) + 1
    order = [
        known.index(heading[2].lower())
        for line in lines[start:]
        if (heading := card.HEADING_RE.match(line))
        and heading[1] == "##"
        and heading[2].lower() in known
    ]
    if order != sorted(set(order)):
        raise card.CardError("sections out of order or repeated; fix by hand")
    missing = [
        name
        for name in card.SECTIONS
        if card.find_heading(lines, name) is None
    ]
    if missing:
        lines = text.rstrip("\n").split("\n")
        for name in missing:
            card.insert_section(lines, name)
        text = "\n".join(lines) + "\n"
    return card.set_field(text, "template_version", VERSION)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
