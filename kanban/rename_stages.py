"""Rename stage words on every card, after a backup.

Each ``--pair OLD=NEW`` renames a stage word in a card's ``stage:``
field and in the Log lines of moves to it, such as
``- 2026-11-15 OLD: by hub``. Other text keeps its words. A card
holding no OLD word is left as it is, so running this again changes
nothing.

Before it writes, it copies each card it will change to
~/.local/state/card/stage-rename-<time>/<Project>/ and prints a
``restore:`` line, a ``cp -r`` of that folder over Projects.
Restoring puts back only those cards and overwrites any edit made to
them after the run. A card edited between the read and the write is
skipped, dropped from the backup and named on stderr; run again.

Usage::

    python3 rename_stages.py [--vault NAME | --projects DIR] --pair OLD=NEW ...
"""

import argparse
import re
import shlex
import sys
from datetime import datetime
from pathlib import Path

import card

# A Log line of a move, whose second group is the stage word.
LOG_MOVE_RE = re.compile(r"^(- \S+ )([\w-]+)(: .*)$")


def main(argv: list[str]) -> int:
    """Back up the cards that hold an old word, then rename it in each.

    Args:
        argv: The arguments after the program name.

    Returns:
        The exit code, 1 when a pair, the vault lookup, the backup or a
        rename fails.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--vault", help="vault name for nobs vault path")
    parser.add_argument("--projects", type=Path, help="a Projects folder")
    parser.add_argument(
        "--pair", action="append", required=True, help="OLD=NEW"
    )
    args = parser.parse_args(argv)
    try:
        pairs = read_pairs(args.pair)
        projects = args.projects or card.find_projects_folder(args.vault)
        cards = [
            note
            for note in card.load_all_cards(projects)
            if rename_words(note.text, pairs) != note.text
        ]
        if not cards:
            print("0 cards hold an old stage word")
            return 0
        backup = card.STATE_DIR / datetime.now().strftime(
            "stage-rename-%Y%m%d-%H%M%S"
        )
        try:
            # A run in the same second must not write over these copies.
            backup.mkdir(parents=True)
        except FileExistsError:
            raise card.CardError(f"{backup} exists, run again") from None
        for note in cards:
            copy = backup / note.project / note.path.name
            copy.parent.mkdir(parents=True, exist_ok=True)
            copy.write_text(note.text, encoding="utf-8")
    except card.CardError as error:
        print(f"rename_stages: {error}", file=sys.stderr)
        return 1
    print(f"backup of {len(cards)} cards: {backup}")
    source, target = shlex.quote(f"{backup}/."), shlex.quote(f"{projects}/")
    print(f"restore: cp -r {source} {target}")
    skipped = []
    for note in cards:
        if not card.write_safely(
            note.path, note.key, rename_words(note.text, pairs), note.mtime_ns
        ):
            # The restore line must not put this copy over the newer edit.
            (backup / note.project / note.path.name).unlink()
            skipped.append(note.key)
    print(f"renamed {len(cards) - len(skipped)} cards")
    if skipped:
        print(
            f"rename_stages: changed while renaming, run again:"
            f" {' '.join(skipped)}",
            file=sys.stderr,
        )
        return 1
    return 0


def read_pairs(values: list[str]) -> dict[str, str]:
    """Read ``OLD=NEW`` values into a map from old word to new word.

    Args:
        values: The ``--pair`` values.

    Returns:
        Each old word's new word.

    Raises:
        CardError: A value is not ``OLD=NEW``, or a new word is also an
            old one, which a second run would rename again.
    """
    pairs = {}
    for value in values:
        old, _, new = value.partition("=")
        if not old or not new:
            raise card.CardError(f"--pair {value!r} is not OLD=NEW")
        pairs[old] = new
    if renamed_again := set(pairs.values()) & set(pairs):
        raise card.CardError(
            f"{', '.join(sorted(renamed_again))} is old and new"
        )
    return pairs


def rename_words(text: str, pairs: dict[str, str]) -> str:
    """Return a card's text with old words renamed in stage and Log lines.

    Args:
        text: The card text.
        pairs: Each old word's new word.

    Returns:
        The new text, the same text when no old word is left.
    """
    stage = card.read_fields(text).get("stage")
    if isinstance(stage, str) and stage in pairs:
        text = card.set_field(text, "stage", pairs[stage])
    lines = text.split("\n")
    bounds = card.find_section(lines, "Log")
    if bounds is not None:
        for index in range(*bounds):
            match = LOG_MOVE_RE.match(lines[index])
            if match and match[2] in pairs:
                lines[index] = f"{match[1]}{pairs[match[2]]}{match[3]}"
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
