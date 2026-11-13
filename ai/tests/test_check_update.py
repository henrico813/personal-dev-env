"""Behavior tests for the checker of the status-updates skill.

Each test writes a draft update to a file and runs check_update.py on
it, as an agent does before it sends an update to the user's phone.
"""

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/status-updates/scripts/check_update.py"


def check(tmp_path: Path, draft: str) -> subprocess.CompletedProcess:
    """Write draft to update.md in tmp_path and run the checker on it.

    Args:
        tmp_path: Empty folder for the run.
        draft: The update's text.

    Returns:
        The finished script.
    """
    path = tmp_path / "update.md"
    path.write_text(draft)
    return subprocess.run(
        [sys.executable, str(SCRIPT), str(path)],
        capture_output=True,
        text=True,
    )


class TestCheckUpdate:
    """The checker reports each update too big for a phone screen."""

    @pytest.mark.parametrize(
        "draft",
        [
            pytest.param(
                "Fernwood, the app that waters the garden on a timer:\n"
                "\n"
                "| Bed | Plants | Next watering |\n"
                "|---|---|---|\n"
                "| North | 4 | 06:00 |\n"
                "\n"
                "- A separate schedule for each garden bed (#8): 18 of 19\n"
                "  tests pass. The failing test covers a bed with no plants.\n"
                "  Not tried on the real valve controller yet.\n"
                "\n"
                "```bash\n"
                "cd ~/src/fernwood/wt/beds-schedules\n"
                "```\n",
                id="recap-shapes",
            ),
            pytest.param("- item\n" * 20, id="twenty-lines"),
        ],
    )
    def test_passes_update_at_each_limit(
        self, tmp_path: Path, draft: str
    ) -> None:
        """An update that meets every limit exactly has no finding.

        The recap in the skill's examples has list items that wrap
        onto three lines, three-column tables and short commands, and
        an update may fill all 20 lines. A checker that reports them
        makes agents ignore it.
        """
        result = check(tmp_path, draft)

        assert result.returncode == 0
        assert result.stdout == ""

    @pytest.mark.parametrize(
        ("draft", "finding"),
        [
            pytest.param(
                "- item\n" * 21,
                "line 1: update fills 21 lines, keep it to about 20",
                id="long-update",
            ),
            pytest.param(
                "```\n" + "x" * 36 + "\n```\n",
                "line 2: command has 36 characters, keep it under 35",
                id="long-command",
            ),
            pytest.param(
                "| a | b | c | d |\n|---|---|---|---|\n",
                "line 1: table has 4 columns, keep it to 3",
                id="wide-table",
            ),
            pytest.param(
                "Intro.\n\n" + "w" * 100 + "\n" + "w" * 100 + "\n",
                "line 3: paragraph fills 4 lines, keep it to 3",
                id="unwrapped-paragraph",
            ),
        ],
    )
    def test_reports_each_phone_limit(
        self, tmp_path: Path, draft: str, finding: str
    ) -> None:
        """Each limit over by one is reported at the line that starts it.

        The user reads updates on a phone, where a long line, a wide
        table or a wall of text is hard to read and a long command
        cannot be copied whole. A 100-character line fills two lines
        on screen, so the last case is a four-line paragraph.
        """
        result = check(tmp_path, draft)

        assert result.returncode == 1
        assert result.stdout == f"{finding}\n"

    def test_runs_the_sentence_checks(self, tmp_path: Path) -> None:
        """The plain-direct-english sentence checks run on the update too.

        The update checker loads them from the skill folder next to
        its own, as both skills install. One command then checks both
        skills' rules.
        """
        result = check(tmp_path, "Fernwood is robust now.\n")

        assert result.returncode == 1
        assert result.stdout == 'line 1: filler word "robust"\n'
