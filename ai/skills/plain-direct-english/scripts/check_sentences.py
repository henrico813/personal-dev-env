"""Find filler words, hedges, closing offers, emojis and banned words.

Reads a draft message or doc from a file or stdin and prints one line
per problem, such as ``line 3: filler word "robust"``. Code blocks
and text in backticks are skipped, since a command or a name may need
one of these words.

Example:
    python3 check_sentences.py draft.md
    python3 check_sentences.py < draft.md
"""

import argparse
import re
import sys
from collections.abc import Callable
from pathlib import Path
from typing import NamedTuple

FENCE = re.compile(r"^\s*(```|~~~)")
INLINE_CODE = re.compile(r"`[^`\n]*`")
# Each rule's words, matched as whole words in any letter case.
WORDS = {
    "filler word": (
        r"comprehensive|robust|seamless(?:ly)?|leverag(?:e|es|ed|ing)"
        r"|ensur(?:e|es|ed|ing)"
    ),
    "banned word": r"canonical|contracts?",
    "hedge": r"probably|perhaps|maybe|hopefully|I think|I believe|it seems",
    "closing offer or praise": (
        r"let me know|feel free|hope this helps|great question"
    ),
}
RULES = [
    (name, re.compile(rf"\b(?:{words})\b", re.IGNORECASE))
    for name, words in WORDS.items()
]
# Symbols and pictographs, and the dingbats that hold check marks and
# warning signs.
EMOJI = re.compile("[\U0001f000-\U0001faff\u2600-\u27bf]")


class Line(NamedTuple):
    """One line of the text, such as ``Line(4, "rm -rf build", True)``."""

    number: int
    text: str
    in_code: bool


Problem = tuple[int, str]


def find_problems(text: str) -> list[Problem]:
    """Return each sentence problem in text, in line order.

    Args:
        text: The message or doc to check.

    Returns:
        Pairs of line number and message, such as
        ``(3, 'hedge "probably"')``.
    """
    problems = []
    for line in number_lines(text):
        if line.in_code:
            continue
        prose = INLINE_CODE.sub("", line.text)
        for name, pattern in RULES:
            for match in pattern.finditer(prose):
                problems.append((line.number, f'{name} "{match.group()}"'))
        if EMOJI.search(prose):
            problems.append((line.number, "emoji"))
    return problems


def number_lines(text: str) -> list[Line]:
    """Return text's lines with 1-based numbers, marking code block lines.

    Args:
        text: The message or doc.

    Returns:
        Every line, with the fence lines of a code block counted as
        code.
    """
    lines = []
    in_code = False
    for number, text_line in enumerate(text.splitlines(), start=1):
        fence = bool(FENCE.match(text_line))
        lines.append(Line(number, text_line, in_code or fence))
        if fence:
            in_code = not in_code
    return lines


def run(find: Callable[[str], list[Problem]], description: str) -> int:
    """Check the file named on the command line, or stdin, with find.

    Args:
        find: Returns the problems in a text.
        description: The command's help text.

    Returns:
        0 when find reports nothing, 1 when it reports a problem, and
        2 when the file cannot be read.
    """
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument(
        "file", nargs="?", type=Path, help="text to check; stdin when left out"
    )
    args = parser.parse_args()
    try:
        text = args.file.read_text() if args.file else sys.stdin.read()
    except OSError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    problems = find(text)
    for number, message in problems:
        print(f"line {number}: {message}")
    return 1 if problems else 0


def main() -> int:
    """Check a draft's sentences and print each problem.

    Returns:
        The exit code from ``run``.
    """
    return run(find_problems, __doc__.splitlines()[0])


if __name__ == "__main__":
    raise SystemExit(main())
