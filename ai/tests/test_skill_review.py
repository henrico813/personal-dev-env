"""Behavior tests for the session-review skill's skill review script."""

import json
import os
import sqlite3
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/session-review/scripts/skill_review.py"
LOAD = {
    "type": "tool",
    "name": "skill",
    "state": {
        "input": {"id": "create-plan"},
        "content": [
            {"type": "text", "text": '<skill_content name="create-plan">'}
        ],
    },
}


def make_home(home: Path, sessions: list[tuple[str, str | None]]) -> None:
    """Write an installed create-plan skill and sessions that load it.

    Each session has a user message at seq 1 and an assistant step at
    seq 2 that loads create-plan. Sessions are created at 1000 ms, 2000
    ms and so on.

    Args:
        home: Folder that stands in for the home folder.
        sessions: ID and parent ID of each session, None for top-level.
    """
    skill = home / ".agents/skills/create-plan/SKILL.md"
    skill.parent.mkdir(parents=True)
    skill.touch()
    path = home / ".local/share/opencode/opencode.db"
    path.parent.mkdir(parents=True)
    with sqlite3.connect(path) as db:
        db.execute("create table session_v2 (id, parent_id, time_created)")
        db.execute(
            "create table session_message (session_id, type, seq, data)"
        )
        for created, (session, parent) in enumerate(sessions, 1):
            db.execute(
                "insert into session_v2 values (?, ?, ?)",
                (session, parent, created * 1000),
            )
            db.executemany(
                "insert into session_message values (?, ?, ?, ?)",
                [
                    (session, "user", 1, json.dumps({"text": "use\nrg"})),
                    (session, "assistant", 2, json.dumps({"content": [LOAD]})),
                ],
            )


def write_state(home: Path) -> None:
    """A state file that has counted sessions since the epoch."""
    path = home / ".local/state/pde/skill-reviews.json"
    path.parent.mkdir(parents=True)
    path.write_text(json.dumps({"started": 0, "reviewed": {}}))


def run_review(home: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run the script with ``home`` as the home folder.

    Args:
        home: Folder that stands in for the home folder.
        args: Command and its arguments.

    Returns:
        The finished process with captured output.
    """
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        env=os.environ | {"HOME": str(home)},
        capture_output=True,
        text=True,
    )


class TestDue:
    """Deciding which skills conclude-session offers a review of."""

    @pytest.mark.parametrize(
        "sessions,printed",
        [
            pytest.param(
                [("ses_a", None), ("ses_b", None)],
                "create-plan 2\n",
                id="two-sessions",
            ),
            pytest.param([("ses_a", None)], "", id="one-session"),
            pytest.param(
                [("ses_a", None), ("ses_sub", "ses_a")],
                "",
                id="subagent-not-counted",
            ),
        ],
    )
    def test_due_lists_skill_after_two_sessions(
        self,
        tmp_path: Path,
        sessions: list[tuple[str, str | None]],
        printed: str,
    ) -> None:
        """Offer a review once two top-level sessions have loaded a skill.

        One session teaches too little to review. A subagent works for a
        session that already counts, and the user rarely corrects it, so
        counting subagents would offer reviews with nothing to learn.
        """
        make_home(tmp_path, sessions)
        write_state(tmp_path)

        result = run_review(tmp_path, "due")

        assert (result.returncode, result.stdout) == (0, printed)

    def test_mark_resets_skill_count(self, tmp_path: Path) -> None:
        """Stop offering a review of a skill once it has been reviewed.

        Without the reset, every later close-out would offer the same
        review of the same sessions again.
        """
        make_home(tmp_path, [("ses_a", None), ("ses_b", None)])
        write_state(tmp_path)
        assert run_review(tmp_path, "due").stdout == "create-plan 2\n"

        run_review(tmp_path, "mark", "create-plan")

        assert run_review(tmp_path, "due").stdout == ""

    def test_first_run_reports_nothing_due(self, tmp_path: Path) -> None:
        """Start counting on the first run instead of from old sessions.

        The database holds months of sessions, so counting them all
        would offer a review of nearly every skill on the first day.
        """
        make_home(tmp_path, [("ses_a", None), ("ses_b", None)])

        result = run_review(tmp_path, "due")

        assert (result.returncode, result.stdout) == (0, "")
        assert (tmp_path / ".local/state/pde/skill-reviews.json").is_file()


class TestCorrections:
    """Printing the user messages a local model sorts into lessons."""

    def test_corrections_prints_one_line_per_message(
        self, tmp_path: Path
    ) -> None:
        """Print each user message as one line that names its session and seq.

        The local model reads only these lines, so a lost line break or a
        wrong message would corrupt its input. The check reads the same
        lines, so it would not catch such a bug.
        """
        make_home(tmp_path, [("ses_a", None), ("ses_b", None)])
        write_state(tmp_path)

        result = run_review(tmp_path, "corrections", "create-plan")

        assert result.stdout == "ses_a 1: use rg\nses_b 1: use rg\n"


class TestCheck:
    """Checking the candidate lessons a local model wrote."""

    @pytest.mark.parametrize(
        "candidates,passes",
        [
            pytest.param("ses_a 1: Search with rg.\n", True, id="printed"),
            pytest.param("none\n", True, id="none"),
            pytest.param("ses_a 2: Search with rg.\n", False, id="not-user"),
            pytest.param("ses_x 1: Search with rg.\n", False, id="unknown"),
        ],
    )
    def test_check_accepts_only_printed_messages(
        self, tmp_path: Path, candidates: str, passes: bool
    ) -> None:
        """Pass a candidate list only when it cites messages the user wrote.

        A local model can make up a session or cite its own step. The
        check is what lets the session trust the list without
        reading the sessions again.
        """
        make_home(tmp_path, [("ses_a", None), ("ses_b", None)])
        write_state(tmp_path)
        file = tmp_path / "candidates.txt"
        file.write_text(candidates)

        result = run_review(tmp_path, "check", "create-plan", str(file))

        assert (result.returncode == 0) == passes
