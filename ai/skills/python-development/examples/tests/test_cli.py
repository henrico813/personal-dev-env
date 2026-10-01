import subprocess
import sys

import pytest


def test_cli_prints_normalized_count() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "python_skill_examples.cli", " 3 "],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0
    assert result.stdout == "3\n"
    assert result.stderr == ""


@pytest.mark.parametrize(
    "argument",
    [
        pytest.param("many", id="not-integer"),
        pytest.param("0", id="below-range"),
    ],
)
def test_cli_rejects_invalid_count(argument: str) -> None:
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "python_skill_examples.cli",
            argument,
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 2
    assert result.stdout == ""
    assert result.stderr.startswith("error: ")


def test_cli_missing_count_shows_usage() -> None:
    result = subprocess.run(
        [sys.executable, "-m", "python_skill_examples.cli"],
        check=False,
        capture_output=True,
        text=True,
    )

    assert result.returncode == 2
    assert result.stdout == ""
    assert result.stderr == "usage: worker-count COUNT\n"
