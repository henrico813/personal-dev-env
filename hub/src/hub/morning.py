"""Run ``hub morning``, which prints everything that waits on the user.

``decisions.md`` in the run folder holds one entry per question for
the user, so a message can show each question in full instead of
naming it by number. An entry looks like this::

    ## New model overnight

    Deploy the new model tonight so you can test it in the morning?

    - A. Deploy from the branch, with the undo path ready.
    - B. Wait until you return.

    Recommended: A, since you want to test it in the morning.
    If no reply: A.
    Depends on: your rule of 2026-11-09 to test it in the morning.

``Depends on:`` is optional. A line that starts with a space carries
on the line above it.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

FIELDS = ("Recommended:", "If no reply:", "Depends on:")


def run_morning(args: argparse.Namespace) -> int:
    """Print reviews.md, then every entry of decisions.md in full.

    Args:
        args: The parsed ``morning`` command line.

    Returns:
        0 after printing, or 2, printing nothing on standard output,
        when an entry lacks a part the user needs to decide.
    """
    entries = read_decisions(args.run / "decisions.md")
    problems = [
        f"hub: decisions.md entry {title!r} lacks {', '.join(missing)}"
        for title, lines in entries
        if (missing := find_missing_parts(lines))
    ]
    if problems:
        print("\n".join(problems), file=sys.stderr)
        return 2

    reviews = args.run / "reviews.md"
    pull_requests = reviews.read_text().strip() if reviews.exists() else ""
    sections = [pull_requests] if pull_requests else []
    for title, lines in entries:
        sections.append("\n".join([f"Decision: {title}", *lines]))
    print("\n\n".join(sections) or "Nothing waits on the user.")
    return 0


def read_decisions(path: Path) -> list[tuple[str, list[str]]]:
    """Read each ``## <title>`` entry and its lines, blank lines left out.

    Args:
        path: The run folder's ``decisions.md``.

    Returns:
        Each entry's title and lines, in file order, or no entries when
        the file is missing.
    """
    if not path.exists():
        return []
    entries: list[tuple[str, list[str]]] = []
    for line in path.read_text().splitlines():
        if line.startswith("## "):
            entries.append((line.removeprefix("## ").strip(), []))
        elif entries and line.strip():
            entries[-1][1].append(line.rstrip())
    return entries


def find_missing_parts(lines: list[str]) -> list[str]:
    """List the parts a decision entry lacks.

    Args:
        lines: The entry's lines, without its title.

    Returns:
        A phrase for each missing part, such as ``the Recommended: line``,
        or an empty list when the entry is whole.
    """
    starts = [line for line in lines if not line.startswith(" ")]
    choices = [line for line in starts if line.startswith("- ")]
    question = [
        line
        for line in starts
        if not line.startswith("- ") and not line.startswith(FIELDS)
    ]
    missing = []
    if not question:
        missing.append("the question")
    if len(choices) < 2:
        missing.append("two choices")
    for field in FIELDS[:2]:
        if not any(line.startswith(field) for line in starts):
            missing.append(f"the {field} line")
    return missing
