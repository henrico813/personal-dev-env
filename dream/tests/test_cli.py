"""Tests for ``dream check`` through its command-line entry point."""

import json
from pathlib import Path

import pytest

from dream.cli import check_file


def test_check_file_fails_empty_fact_list(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Form B with an empty fact list prints FAIL and exits 1.

    An empty list once fell through to the raw file text and crashed the
    check, so a person checking a digest got a traceback, not a reason.
    """
    answer = tmp_path / "answer.jsonl"
    answer.write_text(
        json.dumps({"fact": "F1", "finding": "Re-read.", "fix": "Use head."})
    )
    given = tmp_path / "night.facts.json"
    given.write_text("[]")

    code = check_file("B", answer, given)

    assert code == 1
    assert capsys.readouterr().out == "FAIL unknown-fact: F1\n"
