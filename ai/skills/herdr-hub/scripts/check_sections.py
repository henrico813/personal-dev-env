"""List the sections of a note that no checked quote comes from.

check_quotes.py proves each quote of a summary is in the note, but not
that the summary covers the whole note. This script finds where each
quote of the table sits and prints each ``##`` section that holds none,
with its line range, so the hub can read only that section. A section
runs to the next ``#`` or ``##`` heading, so its ``###`` parts count as
its own. Headings inside code blocks do not count.

Example:
    python3 check_sections.py note.md table.jsonl
    no quote: lines 324-342 ## Command mapping
    quoted: 8 of 9 sections
    done
"""

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

HEADING = re.compile(r"(#{1,6})\s+\S")
FENCE = re.compile(r"\s*(```|~~~)")


@dataclass
class Section:
    """One ``##`` section of the note, by its 1-based line numbers."""

    title: str
    first: int
    last: int


def check_sections(note: Path, table: Path) -> int:
    """Print each section of note that no quote in table comes from.

    Args:
        note: The research note.
        table: The lines check_quotes.py found, one JSON object each.

    Returns:
        0 when every section holds a quote, otherwise 1.
    """
    lines = note.read_text().splitlines()
    sections = find_sections(lines)
    found = (find_quote_line(lines, quote) for quote in read_quotes(table))
    quoted = {line for line in found if line is not None}
    missing = [
        section
        for section in sections
        if not any(section.first <= line <= section.last for line in quoted)
    ]
    for section in missing:
        print(
            f"no quote: lines {section.first}-{section.last} {section.title}"
        )
    print(
        f"quoted: {len(sections) - len(missing)} of {len(sections)} sections"
    )
    print("done")
    return 1 if missing else 0


def find_sections(lines: list[str]) -> list[Section]:
    """Return the note's ``##`` sections that hold any text.

    Args:
        lines: The note's lines.

    Returns:
        The sections in order. Text before the first one is left out.
    """
    sections: list[Section] = []
    current = None
    in_code = False
    for number, line in enumerate(lines, start=1):
        if FENCE.match(line):
            in_code = not in_code
        heading = None if in_code else HEADING.match(line)
        if heading and len(heading.group(1)) <= 2:
            if current:
                current.last = number - 1
                current = None
            if len(heading.group(1)) == 2:
                current = Section(line.strip(), number, len(lines))
                sections.append(current)
    # A heading with no text under it has nothing to quote.
    return [
        section
        for section in sections
        if any(line.strip() for line in lines[section.first : section.last])
    ]


def read_quotes(table: Path) -> list[str]:
    """Return the quote of each line in table.

    Args:
        table: The file check_quotes.py wrote.

    Returns:
        The quotes, in order.
    """
    return [
        json.loads(line)["quote"]
        for line in table.read_text().splitlines()
        if line.strip()
    ]


def find_quote_line(lines: list[str], quote: str) -> int | None:
    """Return the number of the line where quote starts, or None.

    Args:
        lines: The note's lines.
        quote: A quote check_quotes.py found, perhaps across wrapped lines.

    Returns:
        The 1-based line number, or None when the quote is not found.
    """
    # Match on words, as check_quotes.py does, then map the first word
    # back to its line.
    words = [
        (word, number)
        for number, line in enumerate(lines, start=1)
        for word in line.split()
    ]
    text = " ".join(word for word, _ in words)
    start = text.find(" ".join(quote.split()))
    if start < 0:
        return None
    return words[text.count(" ", 0, start)][1]


def main() -> int:
    """Check the note and table named on the command line.

    Returns:
        0 when every section holds a quote, 1 when one does not, and 2
        when a file cannot be read or the table is not JSON lines with
        a quote.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("note", type=Path, help="the research note")
    parser.add_argument("table", type=Path, help="check_quotes.py's table")
    args = parser.parse_args()
    try:
        return check_sections(args.note, args.table)
    except (OSError, json.JSONDecodeError, KeyError, TypeError) as error:
        print(f"error: {error!r}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
