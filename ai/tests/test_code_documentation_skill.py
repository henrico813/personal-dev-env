"""Guard the rules of the code-documentation skill."""

import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SKILL = "ai/skills/code-documentation/SKILL.md"
# The skill on main before checker support was added, with its function
# naming rules. Later changes add text next to the rule they support and
# keep every one of these lines.
BASE = "73689661f223e6e7ea5fb627a53fb208f7956ce6"


class TestSkillText:
    """Keep the skill's original rules."""

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
