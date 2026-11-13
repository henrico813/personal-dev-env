"""Behavior tests for the auto-run skill's plant function."""

import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
FNS = ROOT / "ai/skills/auto-run/scripts/fns.sh"


def run_plant(tmp_path: Path, *test_command: str) -> int:
    """Plant "bug" into a file holding "ok" and return plant's exit code.

    Args:
        tmp_path: Folder for the planted file and plant's backup.
        test_command: The test command, as plant lines give it.

    Returns:
        The exit code of plant.
    """
    (tmp_path / "code.txt").write_text("ok\n")
    script = '. "$0"; D="$1"; cd "$1"; shift; plant code.txt ok bug "$@"'
    result = subprocess.run(
        ["bash", "-c", script, str(FNS), str(tmp_path), *test_command],
        capture_output=True,
        text=True,
        check=False,
    )
    assert (tmp_path / "code.txt").read_text() == "ok\n"
    return result.returncode


@pytest.mark.parametrize(
    ("test_command", "expected"),
    [
        pytest.param(("grep -q bug code.txt",), 1, id="quoted-misses-bug"),
        pytest.param(("grep", "-q", "ok", "code.txt"), 0, id="words-catch-bug"),
    ],
)
def test_plant_runs_quoted_and_split_commands(
    tmp_path: Path, test_command: tuple[str, ...], expected: int
) -> None:
    """Plant runs a test command given as one string or as words.

    Plan lines often quote the whole command, such as
    'cd app && npm test'. When plant ran that string as a command
    name, the shell failed to find it, and every line passed as if a
    test had caught its bug.
    """
    assert run_plant(tmp_path, *test_command) == expected


def test_plant_reports_command_that_cannot_run(tmp_path: Path) -> None:
    """Plant returns 3, not success, when the test command is missing.

    A missing command exits nonzero like a failing test, so without
    this check plant would report a bug as caught when no test ran.
    """
    assert run_plant(tmp_path, "no-such-test-command-xyz") == 3
