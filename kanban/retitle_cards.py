"""Give cards the work types and titles of an approved list.

Each card in the list is a line such as
``- ABC-KB-001 (draft) feat: Old title``, then an optional line
``  -> New title`` right below it. A card with no arrow line keeps
its title and gets only the work type. The whole list is refused on a
``- `` line that is not a card line, an arrow line not right below its
card line, or a key listed twice. Other lines are skipped.

Without ``--apply`` it prints the changes and writes nothing. Before
any write it checks that each key finds one card and each new title
passes check_title with its type, and refuses the whole list on one
failure. With ``--apply`` it copies each card it changes to a new
folder under ~/.local/state/card, writes an ``undo.sh`` there that
puts them back, and prints the command to run it, also when a change
fails partway. A card already at its type and title is skipped, so
running it again changes nothing.

Usage::

    python3 retitle_cards.py LIST [--vault NAME | --projects DIR] [--apply]
"""

import argparse
import re
import shlex
import shutil
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

import card

CARD_LINE_RE = re.compile(r"- (\S+) \([a-z-]+\) ([a-z]+): .*")
REFUSED = "list refused, no card changed:"


@dataclass
class Change:
    """One card's work type and new title, None when it keeps its own."""

    key: str
    work_type: str
    title: str | None = None


def main(argv: list[str]) -> int:
    """Check the whole list, then print its changes or make them.

    Args:
        argv: The arguments after the program name.

    Returns:
        The exit code, 1 when the list is refused or a change fails.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("list", type=Path, help="the approved list file")
    parser.add_argument("--vault", help="vault name for nobs vault path")
    parser.add_argument("--projects", type=Path, help="a Projects folder")
    parser.add_argument("--apply", action="store_true", help="make them")
    args = parser.parse_args(argv)
    try:
        changes = read_changes(args.list.read_text(encoding="utf-8"))
        projects = args.projects or card.find_projects_folder(args.vault)
        cards = check_changes(changes, projects)
    except (OSError, card.CardError) as error:
        print(f"retitle_cards: {error}", file=sys.stderr)
        return 1
    pending = [
        change for change in changes if needs_change(change, cards[change.key])
    ]
    if not args.apply or not pending:
        for change in pending:
            now = cards[change.key]
            if now.work_type != change.work_type:
                print(f"{change.key} type {change.work_type}")
            if change.title is not None and now.title != change.title:
                print(f"{change.key} title {change.title}")
        print(f"{len(pending)} cards to change")
        return 0
    try:
        undo = back_up_cards([cards[change.key] for change in pending])
    except OSError as error:
        print(f"retitle_cards: {error}, no card changed", file=sys.stderr)
        return 1
    for change in pending:
        # The cards before this one changed, so a stop must still print
        # the undo command, as after a dropped network drive.
        try:
            changed = apply_change(change, cards[change.key], projects)
        except (OSError, card.CardError) as error:
            print(f"retitle_cards: {error}", file=sys.stderr)
            changed = False
        if not changed:
            print(f"retitle_cards: stopped at {change.key}", file=sys.stderr)
            print(f"backup: {undo.parent}")
            print(f"undo: sh {shlex.quote(str(undo))}")
            return 1
    print(f"changed {len(pending)} cards")
    print(f"undo: sh {shlex.quote(str(undo))}")
    return 0


def read_changes(text: str) -> list[Change]:
    """Read each card line of the list and the arrow line right below it.

    Args:
        text: The list file's text.

    Returns:
        The changes, in list order.

    Raises:
        card.CardError: A line is out of place, with every such line
            listed by its number.
    """
    changes: list[Change] = []
    bad_lines = []
    keys: set[str] = set()
    above: Change | None = None
    for number, line in enumerate(text.splitlines(), start=1):
        if line.lstrip().startswith("->"):
            if above is None:
                bad_lines.append(f"line {number}: no card line right above")
            elif above.title is not None:
                bad_lines.append(f"line {number}: a second -> for {above.key}")
            else:
                above.title = line.strip().removeprefix("->").strip()
            continue
        above = None
        if match := CARD_LINE_RE.fullmatch(line):
            if match[1].upper() in keys:
                bad_lines.append(f"line {number}: {match[1]} is listed twice")
            keys.add(match[1].upper())
            above = Change(match[1], match[2])
            changes.append(above)
        elif line.startswith("- "):
            bad_lines.append(f"line {number} is not a card line: {line}")
    if bad_lines:
        raise card.CardError("\n".join([REFUSED, *bad_lines]))
    return changes


def check_changes(
    changes: list[Change], projects: Path
) -> dict[str, card.Card]:
    """Find each change's card and check its type and new title.

    Args:
        changes: The changes from the list.
        projects: The vault's Projects folder.

    Returns:
        Each key's card.

    Raises:
        card.CardError: Any key or title fails, with every failure listed.
    """
    problems = []
    cards = {}
    for change in changes:
        try:
            cards[change.key] = card.find_card(projects, change.key)
        except card.CardError as error:
            problems.append(str(error))
        if change.title is None:
            if change.work_type not in card.WORK_TYPES:
                problems.append(
                    f'{change.key} "{change.work_type}" is not a work type'
                )
            continue
        findings = card.check_title(change.title, change.work_type, False)
        problems += [
            f"{change.key} {finding}" for finding in findings if finding.fails
        ]
    if problems:
        raise card.CardError("\n".join([REFUSED, *problems]))
    return cards


def needs_change(change: Change, now: card.Card) -> bool:
    """Return True when the card's type or title differs from the list."""
    if now.work_type != change.work_type:
        return True
    return change.title is not None and now.title != change.title


def back_up_cards(cards: list[card.Card]) -> Path:
    """Copy each card to a new folder and write an undo.sh that restores it.

    undo.sh first removes the card's note under any title, since
    ``card set KEY title`` renames the file.

    Args:
        cards: The cards about to change.

    Returns:
        The path of undo.sh.
    """
    card.STATE_DIR.mkdir(parents=True, exist_ok=True)
    backup = Path(tempfile.mkdtemp(prefix="retitle-", dir=card.STATE_DIR))
    lines = ["#!/bin/sh", "set -e"]
    for note in cards:
        copy = backup / note.path.name
        shutil.copy2(note.path, copy)
        renamed = shlex.quote(str(note.path.parent / f"{note.key} "))
        lines.append(f"rm -f -- {renamed}*.md")
        lines.append(
            f"cp -p -- {shlex.quote(str(copy))} {shlex.quote(str(note.path))}"
        )
    undo = backup / "undo.sh"
    undo.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return undo


def apply_change(change: Change, now: card.Card, projects: Path) -> bool:
    """Set the work type, then the title, with card's own set command.

    The type goes first, since ``card set KEY title`` refuses a card whose
    work type fails T3.

    Args:
        change: The card's change.
        now: The card before the change.
        projects: The vault's Projects folder.

    Returns:
        False when card refused a step; card printed why.
    """
    if now.work_type != change.work_type:
        argv = ["set", change.key, "type", change.work_type]
        if card.main(argv, projects) != 0:
            return False
    if change.title is not None and now.title != change.title:
        argv = ["set", change.key, "title", change.title]
        if card.main(argv, projects) != 0:
            return False
    return True


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
