"""Guard the rules of the code-documentation skill."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SKILL = "ai/skills/code-documentation/SKILL.md"
# The skill on main before checker support was added, with its function
# naming rules. Later changes add text next to the rule they support and
# keep every one of these lines.
BASE = "71467ccac1e61a60fdf753cacbbdb5698361c70b"


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


class TestWrappers:
    """Keep the skill's old script names running the checker command."""

    @pytest.mark.parametrize(
        ("script", "passed"),
        [
            ("check_comments.py", "--repo ."),
            ("check_python_rules.py", "python-rules --repo ."),
        ],
    )
    def test_wrapper_passes_arguments_but_refuses_loops(
        self, tmp_path: Path, script: str, passed: str
    ) -> None:
        """Pass the arguments on, and stop if the command runs it again.

        The old pde-comment-checker script ran the skill's script, so a
        machine with both looped forever and hung every review.
        """
        wrapper = ROOT / "ai/skills/code-documentation/scripts" / script
        fake = tmp_path / "pde-comment-checker"
        fake.write_text(
            f'#!/bin/sh\necho "$*"\nexec "{sys.executable}" "{wrapper}" "$@"\n'
        )
        fake.chmod(0o755)
        path = f"{tmp_path}{os.pathsep}{os.environ['PATH']}"

        result = subprocess.run(
            [sys.executable, str(wrapper), "--repo", "."],
            env={**os.environ, "PATH": path},
            capture_output=True,
            text=True,
            timeout=10,
        )

        assert result.stdout == f"{passed}\n"
        assert result.returncode == 2
        assert "run pde-installer" in result.stderr
