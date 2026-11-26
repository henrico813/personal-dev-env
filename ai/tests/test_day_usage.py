"""Behavior tests for the session-review skill's day usage script."""

import json
import os
import sqlite3
import subprocess
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/session-review/scripts/day_usage.py"


def make_db(home: Path, times: list[datetime]) -> None:
    """A database with one session and one model step at each time."""
    path = home / ".local/share/opencode/opencode.db"
    path.parent.mkdir(parents=True)
    with sqlite3.connect(path) as db:
        db.execute("create table session_v2 (id, title)")
        db.execute(
            "create table session_message"
            " (session_id, type, data, time_created)"
        )
        db.execute("insert into session_v2 values ('ses_main', 'Main')")
        step = json.dumps({"tokens": {"input": 10}})
        db.executemany(
            "insert into session_message"
            " values ('ses_main', 'assistant', ?, ?)",
            [(step, int(moment.timestamp() * 1000)) for moment in times],
        )


def test_day_usage_skips_steps_outside_day(tmp_path: Path) -> None:
    """Count a step only when it was created on the given local day.

    A cost review compares days, so a step from just before or just
    after midnight counted on the wrong day would shift cost between
    them.
    """
    # The script must use the local day, so the zone is not UTC.
    local = timezone(timedelta(hours=-6))
    make_db(
        tmp_path,
        [
            datetime(2026, 11, 23, 23, 59, 59, tzinfo=local),
            datetime(2026, 11, 24, 0, 0, 0, tzinfo=local),
            datetime(2026, 11, 24, 23, 59, 59, tzinfo=local),
            datetime(2026, 11, 25, 0, 0, 0, tzinfo=local),
        ],
    )
    env = os.environ | {"HOME": str(tmp_path), "TZ": "Etc/GMT+6"}

    result = subprocess.run(
        [sys.executable, str(SCRIPT), "--date", "2026-11-24"],
        env=env,
        capture_output=True,
        text=True,
    )

    assert result.stdout.splitlines()[0] == (
        "day 2026-11-24, model steps 2, sessions 1"
    )
