"""Behavior tests for the hub's check that a summary covers its note.

Each test writes a research note and a checked quote table to a
temporary folder and runs check_sections.py there.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/herdr-hub/scripts/check_sections.py"
NOTE = """\
# Queue backends compared

## Summary

Use Redis streams: they keep
order and need no new server.

## Findings

### Kafka

Kafka needs three brokers.

```sh
# not a heading
kafka-topics --list
```

### Redis

Redis streams keep order.

## Costs

Kafka costs twice as much to run.

## Sources

"""


def check(tmp_path: Path, quotes: list[str]) -> subprocess.CompletedProcess:
    """Write note.md and a table of quotes, then run the check on them.

    Args:
        tmp_path: Empty folder for the run.
        quotes: The quotes check_quotes.py found.

    Returns:
        The finished script.
    """
    (tmp_path / "note.md").write_text(NOTE)
    (tmp_path / "table.jsonl").write_text(
        "".join(
            json.dumps({"file": "note.md", "quote": q}) + "\n" for q in quotes
        )
    )
    return subprocess.run(
        [sys.executable, str(SCRIPT), "note.md", "table.jsonl"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
    )


def test_check_sections_lists_unquoted_section(tmp_path: Path) -> None:
    """A section with no quote is named with its lines; quoted ones are not.

    The hub reads only the checked quotes, so a section the summary
    skipped never reaches the user unless this check names it. A quote
    across wrapped lines, or from a ``###`` part, covers its section.
    A heading inside a code block and an empty section are not sections.
    """
    quotes = ["they keep order", "Redis streams keep order."]

    result = check(tmp_path, quotes)

    assert result.returncode == 1
    assert result.stdout == (
        "no quote: lines 23-26 ## Costs\nquoted: 2 of 3 sections\ndone\n"
    )


def test_check_sections_passes_when_all_quoted(tmp_path: Path) -> None:
    """Exit status 0 tells the hub the summary covers every section.

    A false failure would send the hub to reread parts of the note it
    already has, which costs the tokens this check is meant to save.
    """
    quotes = ["Use Redis streams", "Redis streams keep order.", "twice as"]

    result = check(tmp_path, quotes)

    assert result.returncode == 0
    assert result.stdout == "quoted: 3 of 3 sections\ndone\n"
