"""Check a draft update's phone limits, question form and sentences.

Reads the draft from a file or stdin and prints one line per problem,
such as ``line 4: command has 41 characters, keep it under 35``. It
also runs the sentence checks of the plain-direct-english skill, whose
folder sits next to the status-updates skill folder.

Example:
    python3 check_update.py draft.md
"""

import importlib.util
import math
import re
import sys
from pathlib import Path
from types import ModuleType

MAX_LINES = 20
MAX_COMMAND = 35
MAX_COLUMNS = 3
MAX_PARAGRAPH = 3
# A longer line wraps on screen, so a long unwrapped line counts as
# several lines, as a hard-wrapped one does.
SCREEN_WIDTH = 72
SENTENCE_CHECKS = (
    Path(__file__).resolve().parents[2]
    / "plain-direct-english/scripts/check_sentences.py"
)
LIST_ITEM = re.compile(r"^\s*(?:[-*+]|\d+[.)])\s")
# The short question form of references/waiting-on-you.md.
QUESTION_TITLE = re.compile(r"^\*\*\d+\.\s+(.*?)\*\*\s*$")
CHOICE = re.compile(r"^\s*[-*+]\s+(?:[A-Z]|\d+):\s")
CARD_KEY = re.compile(r"^[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)*-\d+\b")
REPLY_LINE = re.compile(r"^Reply like\b")
MIN_CHOICES = 2


def find_problems(text: str, sentences: ModuleType) -> list[tuple[int, str]]:
    """Return the phone-limit and sentence problems in text, in line order.

    Args:
        text: The draft update.
        sentences: The loaded sentence checks module.

    Returns:
        Pairs of line number and message.
    """
    lines = sentences.number_lines(text)
    problems = (
        sentences.find_problems(text)
        + find_long_update(lines)
        + find_long_commands(lines)
        + find_wide_tables(lines)
        + find_long_paragraphs(lines)
        + find_question_problems(lines)
    )
    return sorted(problems)


def find_long_update(lines: list) -> list[tuple[int, str]]:
    """Return a problem at line 1 when the draft fills too many lines.

    Blank lines do not count, since they only space the parts.

    Args:
        lines: The draft's lines from ``number_lines``.

    Returns:
        No pair or one pair of line number and message.
    """
    total = sum(screen_lines(line.text) for line in lines if line.text.strip())
    if total <= MAX_LINES:
        return []
    return [(1, f"update fills {total} lines, keep it to about {MAX_LINES}")]


def find_long_commands(lines: list) -> list[tuple[int, str]]:
    """Return a problem at each code block line over the length limit.

    Args:
        lines: The draft's lines from ``number_lines``.

    Returns:
        Pairs of line number and message.
    """
    problems = []
    for line in lines:
        command = line.text.strip()
        fence = command.startswith(("```", "~~~"))
        if line.in_code and not fence and len(command) > MAX_COMMAND:
            message = (
                f"command has {len(command)} characters, "
                f"keep it under {MAX_COMMAND}"
            )
            problems.append((line.number, message))
    return problems


def find_wide_tables(lines: list) -> list[tuple[int, str]]:
    """Return a problem at the first row of each table over the column limit.

    Args:
        lines: The draft's lines from ``number_lines``.

    Returns:
        Pairs of line number and message.
    """
    problems = []
    in_table = False
    for line in lines:
        row = not line.in_code and line.text.lstrip().startswith("|")
        if row and not in_table:
            columns = len(line.text.strip().strip("|").split("|"))
            if columns > MAX_COLUMNS:
                message = f"table has {columns} columns, keep it to {MAX_COLUMNS}"
                problems.append((line.number, message))
        in_table = row
    return problems


def find_long_paragraphs(lines: list) -> list[tuple[int, str]]:
    """Return a problem at the start of each paragraph over the limit.

    A list item counts as its own paragraph. A blank line, a heading, a
    table row or a code block ends one.

    Args:
        lines: The draft's lines from ``number_lines``.

    Returns:
        Pairs of line number and message.
    """
    sizes = {}
    start = None
    for line in lines:
        text = line.text
        if not text.strip() or line.in_code or text.lstrip()[:1] in ("|", "#"):
            start = None
            continue
        if start is None or LIST_ITEM.match(text):
            start = line.number
            sizes[start] = 0
        sizes[start] += screen_lines(text)
    return [
        (start, f"paragraph fills {size} lines, keep it to {MAX_PARAGRAPH}")
        for start, size in sizes.items()
        if size > MAX_PARAGRAPH
    ]


def find_question_problems(lines: list) -> list[tuple[int, str]]:
    """Return a problem for each short-form question with a missing part.

    A question starts at a bold numbered title, and its choices are the
    list lines right after it. One reply line must follow the last
    question. Code block lines do not count, so an example in a fence
    is never checked.

    Args:
        lines: The draft's lines from ``number_lines``.

    Returns:
        Pairs of line number and message.
    """
    prose = [line for line in lines if not line.in_code]
    problems = []
    last_title = None
    for index, line in enumerate(prose):
        title = QUESTION_TITLE.match(line.text)
        if title:
            last_title = line.number
            rest = prose[index + 1 :]
            problems += find_problems_in_question(
                title.group(1), line.number, rest
            )
        elif last_title and REPLY_LINE.match(line.text.strip()):
            last_title = None
    if last_title:
        message = "questions have no reply line, such as Reply like `1A`"
        problems.append((last_title, message))
    return problems


def find_problems_in_question(
    title: str, number: int, rest: list
) -> list[tuple[int, str]]:
    """Return the problems in one question's title and choices.

    Args:
        title: The title's text between the number and the closing bold.
        number: The title's line number.
        rest: The prose lines after the title.

    Returns:
        Pairs of line number and message.
    """
    problems = []
    if CARD_KEY.match(title):
        problems.append((number, "question title starts with a key"))
    choices = []
    for choice in rest:
        if not CHOICE.match(choice.text):
            break
        choices.append(choice)
    if len(choices) < MIN_CHOICES:
        message = (
            f"question needs {MIN_CHOICES} choices right under its title, "
            "such as - A: ..."
        )
        problems.append((number, message))
    recommended = sum("(rec)" in choice.text for choice in choices)
    if choices and recommended != 1:
        message = f"question has {recommended} (rec) marks, give it one"
        problems.append((number, message))
    for choice in choices:
        # Bold and code marks do not show on screen.
        shown = len(choice.text.strip().replace("**", "").replace("`", ""))
        if shown > SCREEN_WIDTH:
            message = (
                f"choice has {shown} characters, keep it to {SCREEN_WIDTH}"
            )
            problems.append((choice.number, message))
    return problems


def screen_lines(text: str) -> int:
    """Return how many screen lines text fills, such as 2 for 100 characters.

    Args:
        text: One line of the draft.

    Returns:
        At least 1.
    """
    return max(1, math.ceil(len(text) / SCREEN_WIDTH))


def load_sentence_checks() -> ModuleType:
    """Load check_sentences.py from the plain-direct-english skill.

    Returns:
        The loaded module.

    Raises:
        OSError: When the file cannot be read.
    """
    spec = importlib.util.spec_from_file_location(
        "check_sentences", SENTENCE_CHECKS
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    """Check a draft update and print each problem.

    Returns:
        0 when the draft has no problem, 1 when it has one, and 2 when
        the draft or the sentence checks cannot be read.
    """
    try:
        sentences = load_sentence_checks()
    except OSError as error:
        print(f"error: cannot load the sentence checks: {error}", file=sys.stderr)
        return 2
    return sentences.run(
        lambda text: find_problems(text, sentences), __doc__.splitlines()[0]
    )


if __name__ == "__main__":
    raise SystemExit(main())
