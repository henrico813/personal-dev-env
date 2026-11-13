"""Keep only the answer lines whose quote is in the source they name.

A local model reads notes or web pages and writes one JSON line per
answer, with ``quote`` and either ``file`` or ``url``. This script
finds each quote in its source after collapsing runs of whitespace,
since a quote often joins wrapped lines. It reads a web page's text
without its HTML tags. It prints one line per problem, then writes
the found lines to the table file and prints a count and ``done``.

Example:
    python3 check_quotes.py answers.jsonl table.jsonl
"""

import argparse
import json
import sys
import urllib.request
from html.parser import HTMLParser
from pathlib import Path

FETCH_SECONDS = 30


class PageText(HTMLParser):
    """Collects the text of an HTML page, without its tags."""

    def __init__(self) -> None:
        """Start with no text collected."""
        super().__init__()
        self.parts: list[str] = []

    def handle_data(self, data: str) -> None:
        """Keep one run of text between tags.

        Args:
            data: The text, with entities such as ``&ldquo;`` decoded.
        """
        # Tags such as <code> sit inside sentences, so joining the
        # parts with no space keeps those sentences whole.
        self.parts.append(data)


def check_quotes(answers: Path, table: Path) -> int:
    """Copy each answer line whose quote is found to table.

    Args:
        answers: The model's answer file, one JSON object per line.
        table: File that gets the found lines, in order.

    Returns:
        0 when every line's quote was found, otherwise 1.
    """
    lines = answers.read_text().splitlines()
    found = []
    for number, line in enumerate(lines, start=1):
        problem = find_problem(line)
        if problem:
            print(f"line {number}: {problem}")
        else:
            found.append(line)
    table.write_text("".join(f"{line}\n" for line in found))
    print(f"found: {len(found)} of {len(lines)}")
    print("done")
    return 0 if len(found) == len(lines) else 1


def find_problem(line: str) -> str | None:
    """Return why line's quote cannot be trusted, or None when it can.

    Args:
        line: One line of the answer file.

    Returns:
        A short reason, such as ``quote not in cards/a.md``, or None.
    """
    try:
        answer = json.loads(line)
        quote = answer["quote"]
        source = answer.get("file") or answer["url"]
    except (json.JSONDecodeError, KeyError, TypeError, AttributeError):
        return "not a JSON object with quote and file or url"
    if not isinstance(quote, str) or not quote.split():
        return "empty quote"
    try:
        text = read_source(answer)
    except (OSError, TypeError, ValueError):
        return f"cannot read {source}"
    if " ".join(quote.split()) not in " ".join(text.split()):
        return f"quote not in {source}"
    return None


def read_source(answer: dict) -> str:
    """Return the text of the file or web page that answer names.

    Args:
        answer: One answer, with ``file`` or ``url``.

    Returns:
        The file's text, or the page's text without its HTML tags.

    Raises:
        OSError: When the file or page cannot be read.
        TypeError: When ``file`` is not a string or path-like value.
        ValueError: When the URL is not one urllib can open.
    """
    if answer.get("file"):
        return Path(answer["file"]).read_text()
    request = urllib.request.Request(
        answer["url"], headers={"User-Agent": "check_quotes"}
    )
    with urllib.request.urlopen(request, timeout=FETCH_SECONDS) as page:
        html = page.read().decode("utf-8", "replace")
    parser = PageText()
    parser.feed(html)
    return "".join(parser.parts)


def main() -> int:
    """Check the answer file named on the command line.

    Returns:
        0 when every quote was found, 1 when one was not, and 2 when
        the answer file cannot be read or the table cannot be written.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("answers", type=Path, help="the model's JSON lines")
    parser.add_argument("table", type=Path, help="file for the found lines")
    args = parser.parse_args()
    try:
        return check_quotes(args.answers, args.table)
    except OSError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
