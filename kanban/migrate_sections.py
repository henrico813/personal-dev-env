#!/usr/bin/env python3
"""Move cards from the old sections to the ones card writes now.

Old cards have Why, Outcome and "Open questions", some also a Context
section, and hint lines between ``%%`` marks. New cards have Context,
Acceptance Criteria and "Open Questions", with no hints, and a
``sessions`` field after ``prs``. Each Outcome sentence or bullet
becomes one ``- [ ]`` item, in its own words.

A card already in the new layout is left as it is, so running this
again changes nothing.

Usage::

    migrate_sections.py [--vault NAME] [--dry-run] [KEY ...]

With no KEY it migrates every card in the vault.
"""

import argparse
import re
import sys

import card

# A whole-line hint and the blank line after it.
HINT_LINE_RE = re.compile(r"^[ \t]*%%[^\n]*?%%[ \t]*\n(?:[ \t]*\n)?", re.M)
SENTENCE_END_RE = re.compile(r"(?<=[.!?])\s+(?=\S)")
CHECKBOX_LINE_RE = re.compile(r"^- \[[ xX]\] ")


def main(argv: list[str]) -> int:
    """Migrate the named cards, or all, or list the ones that would change.

    Args:
        argv: The arguments after the program name.

    Returns:
        The exit code.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--vault", help="vault name for nobs vault path")
    parser.add_argument("keys", nargs="*", help="cards to migrate")
    parser.add_argument(
        "--dry-run", action="store_true", help="list cards, change none"
    )
    args = parser.parse_args(argv)
    try:
        projects = card.find_projects_folder(args.vault)
        changed = 0
        cards = (
            [card.find_card(projects, key) for key in args.keys]
            if args.keys
            else card.load_all_cards(projects)
        )
        for note in cards:
            if migrate_text(note.text) == note.text:
                continue
            changed += 1
            if not args.dry_run:
                card.update_card(note, migrate_text)
            print(
                f"{'would migrate' if args.dry_run else 'migrated'} {note.key}"
            )
    except card.CardError as error:
        print(f"migrate_sections: {error}", file=sys.stderr)
        return 1
    print(f"{changed} of {len(cards)} cards changed")
    return 0


def migrate_text(text: str) -> str:
    """Return a card's text in the new section layout.

    Args:
        text: The card text.

    Returns:
        The new text, the same text for a card already migrated.
    """
    lines = text.split("\n")
    end = card.find_frontmatter_end(lines)
    start = end + 1 if end else 0
    head, body = "\n".join(lines[:start]), "\n".join(lines[start:])
    text = (head + "\n" if head else "") + HINT_LINE_RE.sub("", body)
    if card.find_heading(text.split("\n"), "Why") is not None:
        text = merge_section(text, "Context", "Why")
        text = rename_heading(text, "Why", "Context")
    text = convert_outcome(text)
    text = rename_heading(text, "Open questions", "Open Questions")
    if end and "sessions" not in card.read_fields(text):
        text = card.set_field(text, "sessions", [])
    return text


def merge_section(text: str, source: str, target: str) -> str:
    """Move a section's lines to the end of another and drop its heading.

    Args:
        text: The card text.
        source: The section to remove.
        target: The section that gets its lines.

    Returns:
        The new text, unchanged when either section is missing.
    """
    lines = text.split("\n")
    bounds = card.find_section(lines, source)
    if bounds is None or card.find_heading(lines, target) is None:
        return text
    moved = [line for line in lines[bounds[0] : bounds[1]] if line.strip()]
    del lines[card.find_heading(lines, source) : bounds[1]]
    for line in moved:
        lines = card.add_to_section("\n".join(lines), target, line).split("\n")
    return "\n".join(lines)


def convert_outcome(text: str) -> str:
    """Turn the Outcome section into Acceptance Criteria checkboxes.

    Args:
        text: The card text.

    Returns:
        The new text, unchanged when there is no Outcome section.
    """
    lines = text.split("\n")
    bounds = card.find_section(lines, "Outcome")
    if bounds is None:
        return text
    start, stop = bounds
    body = []
    for line in lines[start:stop]:
        # Blank, indented and heading lines, and checkboxes, stay as they are.
        if (
            not line.strip()
            or line[:1].isspace()
            or line.startswith("#")
            or CHECKBOX_LINE_RE.match(line)
        ):
            body.append(line)
            continue
        words = line.removeprefix("- ").strip()
        body += [f"- [ ] {item}" for item in SENTENCE_END_RE.split(words)]
    lines[start:stop] = body
    return rename_heading("\n".join(lines), "Outcome", card.CRITERIA)


def rename_heading(text: str, old: str, new: str) -> str:
    """Rename a ``##`` heading, matched in any case.

    Args:
        text: The card text.
        old: The heading to rename.
        new: The new heading.

    Returns:
        The new text, unchanged when the heading is missing.
    """
    lines = text.split("\n")
    index = card.find_heading(lines, old)
    if index is not None:
        lines[index] = f"## {new}"
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
