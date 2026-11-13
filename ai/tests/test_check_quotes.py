"""Behavior tests for the quote check that local reading jobs pass.

Each test writes a card, a web page and a model's answer file to a
temporary folder and runs check_quotes.py there.
"""

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/model-routing/scripts/check_quotes.py"
CARD = """\
# Fix the board
---

- 2026-11-10 review: the board shows each card's
  next step on its details page
"""
PAGE = """\
<html><body><p>Pass <code>--ff-only</code> to refuse
&ldquo;merges&rdquo; that are not fast-forward.</p></body></html>
"""


def check(tmp_path: Path, answers: list[str]) -> subprocess.CompletedProcess:
    """Write card.md and answers.jsonl, then run the check on them.

    Args:
        tmp_path: Empty folder for the run.
        answers: The answer file's lines.

    Returns:
        The finished script.
    """
    (tmp_path / "card.md").write_text(CARD)
    (tmp_path / "page.html").write_text(PAGE)
    (tmp_path / "answers.jsonl").write_text("".join(f"{a}\n" for a in answers))
    return subprocess.run(
        [sys.executable, str(SCRIPT), "answers.jsonl", "table.jsonl"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )


def answer(quote: str, file: str = "card.md") -> str:
    """Return one answer line for file with quote.

    Args:
        quote: The quote the model gave.
        file: The card file it names.

    Returns:
        The JSON line.
    """
    return json.dumps({"file": file, "quote": quote})


def test_check_quotes_keeps_found_quotes(tmp_path: Path) -> None:
    """Every found quote reaches the table, even one across a wrapped line.

    The stronger model sorts only the table, so a dropped true line
    loses a card from the pick list. Cards wrap long lines, and the
    model's quote joins them, so an exact match would reject most true
    quotes.
    """
    lines = [answer("the board shows each card's next step on its")]

    result = check(tmp_path, lines)

    assert result.returncode == 0
    assert result.stdout == "found: 1 of 1\ndone\n"
    assert (tmp_path / "table.jsonl").read_text() == f"{lines[0]}\n"


def test_check_quotes_drops_untrusted_lines(tmp_path: Path) -> None:
    """A made-up quote, a missing card or a broken line stays out.

    A local model's own report cannot be trusted, and four cloud
    subagents once read over six million tokens to check one card
    pile. The script is the only check before the stronger model
    reads the table, so each bad line must stay out and be named.
    """
    good = answer("next step on its details page")
    lines = [
        answer("the board is done"),
        good,
        answer("next step", file="gone.md"),
        answer("   "),
        "not json",
    ]

    result = check(tmp_path, lines)

    assert result.returncode == 1
    assert result.stdout == (
        "line 1: quote not in card.md\n"
        "line 3: cannot read gone.md\n"
        "line 4: empty quote\n"
        "line 5: not a JSON object with quote and file or url\n"
        "found: 1 of 5\n"
        "done\n"
    )
    assert (tmp_path / "table.jsonl").read_text() == f"{good}\n"


def test_check_quotes_reads_web_pages(tmp_path: Path) -> None:
    """A quote from a web page is checked against the page's text.

    Web lookups that only quote pages run on the local model, so the
    script is the only check that a quote is real. Pages wrap words in
    tags such as <code> and write quotes as entities, and the model
    quotes the text a reader sees, so a check of the raw HTML would
    reject true quotes.
    """
    url = (tmp_path / "page.html").as_uri()
    good = json.dumps({"url": url, "quote": "Pass --ff-only to refuse"})
    lines = [
        good,
        json.dumps({"url": url, "quote": "\u201cmerges\u201d that are"}),
        json.dumps({"url": url, "quote": "Pass --rebase to refuse"}),
    ]

    result = check(tmp_path, lines)

    assert result.returncode == 1
    assert result.stdout == (
        f"line 3: quote not in {url}\n"
        "found: 2 of 3\n"
        "done\n"
    )
    assert (tmp_path / "table.jsonl").read_text() == f"{good}\n{lines[1]}\n"
