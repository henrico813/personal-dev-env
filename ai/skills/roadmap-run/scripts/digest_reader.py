"""Ask a model a question about a design digest, and check its quote.

A planner or reviewer asks instead of reading the whole digest::

    python3 digest_reader.py ask --model MODEL DIGEST "QUESTION"

``ask`` prints one JSON line with ``answer``, ``line`` and ``quote``,
where ``line`` is the digest line the quote starts on. It exits 1
when the model gives no answer or the quote does not start on that
line. ``check DIGEST ANSWER`` runs the same quote check on an answer
file.
"""

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# Passage lines in a digest start with their design note's line number,
# such as `216:` or `216-`.
NOTE_LINE_NUMBER = re.compile(r"^\d+[:-]")
# A quote may run onto later lines, since design notes wrap long text.
QUOTE_LINES = 10
MODEL_TIMEOUT_SECONDS = 600
PROMPT = """\
Answer this question about the attached design digest: {question}
Each line of the attachment starts with its line number and a tab.
Reply with one JSON line and nothing else, in this form:
{{"answer": "<one or two sentences>", "line": <the number before the \
tab on the line where your quote starts>, "quote": "<the exact words \
from the digest that support the answer, without line numbers>"}}
Use no tool and edit no file.
"""


def main(argv: list[str] | None = None) -> int:
    """Run the ask or check command and return the exit code.

    Args:
        argv: The command-line arguments, without the program name.

    Returns:
        0 when the quote starts on its cited line, and 1 otherwise.
    """
    args = parse_args(argv)
    try:
        lines = args.digest.read_text().splitlines()
        if args.command == "ask":
            answer = ask_model(args.model, lines, args.question)
        else:
            answer = read_answer(args.answer)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    problem = find_quote_problem(lines, answer)
    if problem:
        print(f"error: {problem}", file=sys.stderr)
        return 1
    print(json.dumps(answer))
    return 0


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse the ask and check commands.

    Args:
        argv: The command-line arguments, without the program name.

    Returns:
        The parsed arguments.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    ask = commands.add_parser("ask", help="ask a model, then check")
    ask.add_argument("--model", required=True, help="OpenCode model name")
    ask.add_argument("digest", type=Path)
    ask.add_argument("question")
    check = commands.add_parser("check", help="check an answer file")
    check.add_argument("digest", type=Path)
    check.add_argument("answer", type=Path, help="one JSON answer")
    return parser.parse_args(argv)


def ask_model(model: str, lines: list[str], question: str) -> dict:
    """Return the model's JSON answer to the question.

    The model reads a copy of the digest with each line's number in
    front. It then cites the digest's own line numbers, not the note
    line numbers that passage lines start with.

    Args:
        model: The OpenCode model, such as the run's local model.
        lines: The digest's lines.
        question: The question to answer from the digest.

    Returns:
        The answer, with ``answer``, ``line`` and ``quote`` keys.

    Raises:
        OSError: ``opencode`` could not start.
        ValueError: The reply holds no JSON object.
        subprocess.TimeoutExpired: The model did not finish in time.
    """
    with tempfile.TemporaryDirectory() as folder:
        numbered = Path(folder) / "digest.txt"
        numbered.write_text(
            "".join(f"{n}\t{text}\n" for n, text in enumerate(lines, 1))
        )
        result = subprocess.run(
            [
                "opencode",
                "run",
                "--standalone",
                "-m",
                model,
                "-f",
                str(numbered),
                "--",
                PROMPT.format(question=question),
            ],
            capture_output=True,
            text=True,
            cwd=folder,
            timeout=MODEL_TIMEOUT_SECONDS,
            check=False,
        )
    return parse_last_json_object(result.stdout)


def read_answer(path: Path) -> dict:
    """Return the JSON answer saved in a file.

    Args:
        path: A file holding one JSON answer.

    Returns:
        The answer.

    Raises:
        OSError: The file cannot be read.
        ValueError: The file holds no JSON object.
    """
    return parse_last_json_object(path.read_text())


def parse_last_json_object(text: str) -> dict:
    """Return the last line of text that is a JSON object.

    Args:
        text: The model's reply, which may hold other lines.

    Returns:
        The parsed object.

    Raises:
        ValueError: No line is a JSON object.
    """
    for line in reversed(text.splitlines()):
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict):
            return value
    raise ValueError("the reply holds no JSON answer")


def find_quote_problem(lines: list[str], answer: dict) -> str | None:
    """Return why the quote does not start on its cited line, or None.

    The check drops the note line number at the start of each digest
    line and collapses runs of whitespace, so a quote may join wrapped
    lines.

    Args:
        lines: The digest's lines.
        answer: The answer, with ``line`` and ``quote`` keys.

    Returns:
        The problem to report, or None when the quote starts on the
        cited line.
    """
    number = answer.get("line")
    quote = collapse_spaces(str(answer.get("quote", "")))
    if not isinstance(number, int) or not quote:
        return "the answer needs a line number and a quote"
    if not 1 <= number <= len(lines):
        return f"line {number} is not in the digest"
    cited = [
        NOTE_LINE_NUMBER.sub("", text, count=1)
        for text in lines[number - 1 : number - 1 + QUOTE_LINES]
    ]
    first_line = collapse_spaces(cited[0])
    start = collapse_spaces(" ".join(cited)).find(quote)
    if start == -1 or start >= len(first_line):
        return f"the quote does not start on line {number}"
    return None


def collapse_spaces(text: str) -> str:
    """Return the text with each run of whitespace made one space.

    Args:
        text: Any text.

    Returns:
        The text, trimmed, with single spaces.
    """
    return " ".join(text.split())


if __name__ == "__main__":
    sys.exit(main())
