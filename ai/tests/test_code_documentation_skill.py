"""Guard the rules of the code-documentation skill."""

import os
import runpy
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SKILL = "ai/skills/code-documentation/SKILL.md"
CHECKER = ROOT / "ai/skills/code-documentation/scripts/check_comments.py"
# The skill before checker support was added. Later changes add text
# next to the rule they support and keep every one of these lines.
BASE = "e1d01592d5e8347d416473e53b6f40875cc0b8fe"
# Original lines a change removed or edited on purpose, each with why.
CHANGED: dict[str, str] = {}


def test_skill_keeps_every_original_line() -> None:
    """Fail when a change drops or rewords an original skill line.

    An earlier fix for agents' comment mistakes rewrote most of this
    skill, and the owner rejected it: the rules were right, and agents
    had broken them. Fixes now only add text, so every original line
    must still be present, in order, unless ``CHANGED`` gives a reason.
    """
    source = Path(os.environ.get("PDE_REPO_ROOT", ROOT))
    shown = subprocess.run(
        ["git", "-C", str(source), "show", f"{BASE}:{SKILL}"],
        capture_output=True,
        text=True,
    )
    if shown.returncode != 0:
        # Exported trees have no history; CI checks out all of it.
        message = f"commit {BASE} is not in {source}"
        if os.environ.get("CI"):
            raise AssertionError(message)
        pytest.skip(message)
    original = shown.stdout.splitlines()
    current = iter((ROOT / SKILL).read_text().splitlines())

    # "in" on an iterator consumes it up to the match, so each line must
    # come after the previous one.
    missing = [
        line
        for line in original
        if line not in CHANGED and line not in current
    ]

    assert missing == []


def test_every_rule_quotes_the_skill() -> None:
    """Fail when a check cites a sentence the skill no longer has.

    The checker may only enforce rules code-documentation states; an
    earlier draft added checks with no rule behind them and was
    rejected. When someone rewords a cited sentence, this points at the
    check to update or remove.
    """
    rules = runpy.run_path(str(CHECKER))["RULES"]
    skill = " ".join((ROOT / SKILL).read_text().split())

    missing = [rule for rule, quote in rules.items() if quote not in skill]

    assert missing == []
