"""Check a run of rename_stages.py against its backup.

It fails when a card still holds an old word in its ``stage:`` line
or a move line of its Log, or when a backed-up card differs from its copy in
any other way than one renamed word on such a line. It reads the
cards without rename_stages.py or card.py, but matches move lines
with the same pattern as rename_stages.py, so a wrong pattern would
fool both.

Usage::

    python3 check_stage_rename.py BACKUP PROJECTS --pair OLD=NEW ...

It prints the counts and the first few problems, then ``done``.
"""

import argparse
import re
import sys
from pathlib import Path

# The empty third group lines it up with LOG_MOVE_RE.
STAGE_RE = re.compile(r"^(stage: )(\S+)()$")
LOG_MOVE_RE = re.compile(r"^(- \S+ )([\w-]+)(: .*)$")
HEADING_RE = re.compile(r"^#{1,2}\s")
SHOWN_PROBLEMS = 5


def main(argv: list[str]) -> int:
    """Print the problems found, and exit 1 when there are any.

    Args:
        argv: The arguments after the program name.

    Returns:
        The exit code.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("backup", type=Path)
    parser.add_argument("projects", type=Path)
    parser.add_argument("--pair", action="append", required=True)
    args = parser.parse_args(argv)
    pairs = dict(value.split("=", 1) for value in args.pair)
    cards = sorted(args.projects.glob("*/*-KB-*.md"))
    copies = sorted(args.backup.glob("*/*-KB-*.md"))
    problems = [
        f"{path.name}: {line}"
        for path in cards
        for line in list_old_words(path.read_text(), pairs)
    ]
    for copy in copies:
        current = args.projects / copy.relative_to(args.backup)
        if not current.exists():
            problems.append(f"{copy.name}: gone")
            continue
        problems += [
            f"{copy.name}: {change}"
            for change in list_other_changes(
                copy.read_text(), current.read_text(), pairs
            )
        ]
    print(f"{len(cards)} cards, {len(copies)} backed up")
    print(f"{len(problems)} problems")
    for problem in problems[:SHOWN_PROBLEMS]:
        print(problem)
    print("done")
    return 1 if problems else 0


def list_old_words(text: str, pairs: dict[str, str]) -> list[str]:
    """List the stage and Log move lines that still hold an old word.

    Args:
        text: The card text.
        pairs: Each old word's new word.

    Returns:
        The lines.
    """
    lines = text.split("\n")
    return [
        line
        for line, in_log in zip(lines, mark_log_lines(lines))
        if (match := match_stage_word(line, in_log)) and match[2] in pairs
    ]


def list_other_changes(old: str, new: str, pairs: dict[str, str]) -> list[str]:
    """List the changes from old to new text that are not a renamed word.

    Args:
        old: The backed-up card text.
        new: The card text now.
        pairs: Each old word's new word.

    Returns:
        One short text per change.
    """
    old_lines, new_lines = old.split("\n"), new.split("\n")
    if len(old_lines) != len(new_lines):
        return [f"{len(old_lines)} lines became {len(new_lines)}"]
    changes = []
    in_log = mark_log_lines(old_lines)
    for index, (before, after) in enumerate(zip(old_lines, new_lines)):
        if before == after:
            continue
        match = match_stage_word(before, in_log[index])
        renamed = (
            match is not None
            and match[2] in pairs
            and after == f"{match[1]}{pairs[match[2]]}{match[3]}"
        )
        if not renamed:
            changes.append(f"{before!r} became {after!r}")
    return changes


def mark_log_lines(lines: list[str]) -> list[bool]:
    """Mark each line that sits under the ``## Log`` heading.

    Args:
        lines: The card text split into lines.

    Returns:
        True for each line in the Log section.
    """
    marks, in_log = [], False
    for line in lines:
        if HEADING_RE.match(line):
            in_log = line.strip() == "## Log"
        marks.append(in_log)
    return marks


def match_stage_word(line: str, in_log: bool) -> re.Match[str] | None:
    """Match a ``stage:`` line, or a move line when it is in the Log.

    Args:
        line: One line of a card.
        in_log: True when the line is in the Log section.

    Returns:
        The match, whose second group is the stage word, or None.
    """
    if in_log:
        return LOG_MOVE_RE.match(line)
    return STAGE_RE.match(line)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
