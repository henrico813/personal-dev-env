"""Behavior tests for the screen-check skill's screen script.

Each test runs a small Python program on a real tmux screen, the way
an agent checks a TUI or CLI change, so these tests need tmux on PATH.
"""

import json
import os
import subprocess
import sys
import time
import uuid
from collections.abc import Iterator
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/screen-check/scripts/screen.py"


def run_screen(*args: str) -> subprocess.CompletedProcess:
    """Run screen.py with the given arguments.

    Args:
        args: The subcommand and its arguments.

    Returns:
        The finished run.
    """
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def start_python(name: str, code: str) -> None:
    """Start Python code on a phone-size screen, failing on an error.

    Args:
        name: The screen's name.
        code: Python source to run with ``-c``.
    """
    result = run_screen("start", name, "--", sys.executable, "-c", code)
    assert result.returncode == 0, result.stderr


def wait_for_exit(pid: int) -> bool:
    """Return whether process pid ends within five seconds.

    Args:
        pid: The process to watch.

    Returns:
        True once the process is gone, False if it still runs.
    """
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return True
        time.sleep(0.05)
    return False


@pytest.fixture
def name() -> Iterator[str]:
    """A screen name no other test uses, stopped after the test."""
    screen = f"test-{uuid.uuid4().hex[:8]}"
    yield screen
    run_screen("stop", screen)


class TestStart:
    """``start`` runs a program on a screen of a set size."""

    def test_start_wraps_output_at_phone_width(self, name: str) -> None:
        """A program started with no size draws on a 40x16 screen.

        Agents check changes at phone size because the user reads on a
        phone. A screen of any other size would let a line that wraps
        or a cut-off row on the phone pass the check.
        """
        start_python(name, "import time; print('x' * 60); time.sleep(60)")

        result = run_screen("wait", name, "xxxx")

        screen = json.loads(result.stdout)
        assert result.returncode == 0
        assert (screen["width"], screen["height"]) == (40, 16)
        assert screen["lines"][:2] == ["x" * 40, "x" * 20]


class TestWait:
    """``wait`` prints the screen once its text shows or cannot show."""

    def test_wait_keeps_screen_after_program_exits(self, name: str) -> None:
        """A CLI's last output and exit code stay readable after it exits.

        tmux 3.7 blanks a dead pane's screen, so without the script
        holding the pane open an agent would see an empty screen for
        every CLI. wait also stops early, rather than waiting out its
        timeout for text that can no longer appear.
        """
        start_python(name, "print('done'); raise SystemExit(3)")
        started = time.monotonic()

        result = run_screen("wait", name, "missing", "--timeout", "30")

        screen = json.loads(result.stdout)
        assert result.returncode == 1
        assert time.monotonic() - started < 20
        assert screen["lines"][0] == "done"
        assert (screen["exited"], screen["exit_code"]) == (True, 3)


class TestKeys:
    """``type`` and ``keys`` reach the program as a person's typing."""

    def test_typed_text_and_enter_reach_program(self, name: str) -> None:
        """Typed text and a named key arrive at the program's input.

        Agents drive a TUI this way to reach the screen they changed.
        If keys were lost, every check past the first screen would fail
        or time out.
        """
        start_python(
            name,
            "import time; print('hi ' + input('name? ')); time.sleep(60)",
        )
        run_screen("wait", name, "name?")

        run_screen("type", name, "sam")
        run_screen("keys", name, "Enter")
        result = run_screen("wait", name, "hi sam")

        assert result.returncode == 0


class TestStop:
    """``stop`` ends the program and its screen."""

    def test_stop_ends_program_and_screen(
        self, name: str, tmp_path: Path
    ) -> None:
        """After stop, the program is gone and show says how to start one.

        Agents must close what they open. A screen left running keeps
        its program and a tmux server alive on the user's machine.
        """
        pid_file = tmp_path / "pid"
        start_python(
            name,
            "import os, time; "
            f"open({str(pid_file)!r}, 'w').write(str(os.getpid())); "
            "print('ready'); time.sleep(60)",
        )
        run_screen("wait", name, "ready")
        pid = int(pid_file.read_text())

        stopped = run_screen("stop", name)
        shown = run_screen("show", name)

        assert stopped.returncode == 0
        assert wait_for_exit(pid)
        assert shown.returncode == 2
        assert f"no screen named {name}" in shown.stderr
