"""The ``dream`` command line: ``dream run`` and ``dream check``.

Exit 0 when the night's report is written or a check passes, 1 when a
check fails, 2 on bad arguments or a config ``dream`` cannot use.
"""

from __future__ import annotations

import argparse
import datetime
import json
import sys
from pathlib import Path

from dream.checks import check_answer
from dream.config import DEFAULT_PATH, ConfigError, load_config
from dream.forms import Fact
from dream.night import run_night


def main(argv: list[str] | None = None) -> int:
    """Run a dream command; exit 2 on a bad call or config."""
    parser = argparse.ArgumentParser(prog="dream")
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser("run", help="run one night")
    run.add_argument("--config", type=Path, default=DEFAULT_PATH)
    run.add_argument("--only", choices=("A", "B"))
    check = commands.add_parser("check", help="check one answer file")
    check.add_argument("form", choices=("A", "B"))
    check.add_argument("answer", type=Path)
    check.add_argument(
        "given", type=Path, help="the item text (A) or a .facts.json (B)"
    )
    args = parser.parse_args(argv)
    if args.command == "run":
        try:
            config = load_config(args.config)
        except ConfigError as error:
            print(f"dream: {error}", file=sys.stderr)
            return 2
        return run_night(config, args.only, datetime.datetime.now)
    return check_file(args.form, args.answer, args.given)


def check_file(form: str, answer: Path, given: Path) -> int:
    """Print PASS, or FAIL and the reasons; return the exit code."""
    try:
        answer_text = answer.read_text()
        given_text = given.read_text()
        facts = (
            [
                Fact(f["id"], f["line"], frozenset(f["numbers"]))
                for f in json.loads(given_text)
            ]
            if form == "B"
            else None
        )
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"dream: cannot read input: {error}", file=sys.stderr)
        return 2
    # An empty fact list is falsy, so test for None rather than truth.
    given_value = given_text if facts is None else facts
    reasons = check_answer(form, answer_text, given_value)
    print("PASS" if not reasons else "FAIL " + " ; ".join(reasons))
    return 1 if reasons else 0
