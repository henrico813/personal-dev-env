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


SHORT_QUESTIONS = (
    "**1. Lessons rework: how the repeat search survives squash merges**\n"
    "- A: the merge step copies the fix lines into the squash message"
    " **(rec)**\n"
    "- B: merge fix PRs by rebase\n"
    "- C: search merged PRs on GitHub\n"
    "\n"
    "**2. Model routing: do you want a routing table (DEMO-KB-7)?**\n"
    "- 1: yes, redesign around them **(rec)**\n"
    "- 2: no\n"
    "\n"
    "Reply like `1A 2-1`.\n"
)


class TestCheckUpdateQuestions:
    """The checker reports a short-form question with a broken part."""

    def test_passes_questions_in_short_form(self, tmp_path: Path) -> None:
        """Questions in the form that waiting-on-you.md shows pass.

        The draft is the skill's own example, with a card key after the
        title's words. Its first choice is 74 characters with its bold
        marks and 70 without, so the checker must leave the marks out.
        """
        result = check(tmp_path, SHORT_QUESTIONS)

        assert result.returncode == 0
        assert result.stdout == ""

    @pytest.mark.parametrize(
        ("draft", "finding"),
        [
            pytest.param(
                SHORT_QUESTIONS.replace("Reply like `1A 2-1`.\n", ""),
                "line 6: questions have no reply line,"
                " such as Reply like `1A`",
                id="no-reply-line",
            ),
            pytest.param(
                SHORT_QUESTIONS.replace("them **(rec)**", "them"),
                "line 6: question has 0 (rec) marks, give it one",
                id="no-recommended",
            ),
            pytest.param(
                SHORT_QUESTIONS.replace("**2. Model", "**2. DEMO-KB-7: Model"),
                "line 6: question title starts with a key",
                id="key-first",
            ),
            pytest.param(
                SHORT_QUESTIONS.replace("- 2: no\n", "Or no.\n"),
                "line 6: question needs 2 choices right under its title,"
                " such as - A: ...",
                id="one-choice",
            ),
            pytest.param(
                SHORT_QUESTIONS.replace("- 2: no", "- 2: no, " + "w" * 70),
                "line 8: choice has 79 characters, keep it to 72",
                id="long-choice",
            ),
        ],
    )
    def test_reports_each_broken_part(
        self, tmp_path: Path, draft: str, finding: str
    ) -> None:
        """Each broken part of the short form is reported at its line.

        The user answers questions on a phone. Without one reply line,
        one recommendation, two choices, the work named first and short
        choices, the user has to read the whole message to know what to
        type.
        """
        result = check(tmp_path, draft)

        assert result.returncode == 1
        assert result.stdout == f"{finding}\n"
