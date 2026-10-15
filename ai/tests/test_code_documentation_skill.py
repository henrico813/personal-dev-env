"""Guard the rules of the code-documentation skill."""

import os
import re
import runpy
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SKILL = "ai/skills/code-documentation/SKILL.md"
CHECKER = ROOT / "ai/skills/code-documentation/scripts/check_comments.py"
# The skill on main before checker support was added, with its function
# naming rules. Later changes add text next to the rule they support and
# keep every one of these lines.
BASE = "71467ccac1e61a60fdf753cacbbdb5698361c70b"


class TestSkillText:
    """Keep the skill's original rules, and keep the checker in step."""

    def test_skill_keeps_every_original_line(self) -> None:
        """Fail when a change drops or rewords an original skill line.

        An earlier fix for agents' comment mistakes rewrote most of this
        skill, and the owner rejected it: the rules were right, and
        agents had broken them. Fixes now only add text, so every
        original line must still be present, in order.
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

        # "in" on an iterator consumes it up to the match, so each line
        # must come after the previous one.
        missing = [line for line in original if line not in current]

        assert missing == []

    def test_skill_quotes_checker_rules_and_options(self) -> None:
        """Fail when the skill and the checker drift apart.

        Each rule may only enforce a sentence the skill states; an
        earlier draft added checks with no rule behind them and was
        rejected. Agents also run the skill's command word for word, so
        an option the checker lacks stops them with a usage error.
        """
        skill = (ROOT / SKILL).read_text()
        rules = runpy.run_path(str(CHECKER))["RULES"]
        shown = subprocess.run(
            [sys.executable, str(CHECKER), "--help"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout

        flat = " ".join(skill.split())
        unquoted = [rule for rule, quote in rules.items() if quote not in flat]
        named = set(re.findall(r"--[a-z][a-z-]*", skill))
        accepted = set(re.findall(r"--[a-z][a-z-]*", shown))

        assert unquoted == []
        assert named
        assert named - accepted == set()
