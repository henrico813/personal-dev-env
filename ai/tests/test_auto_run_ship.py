"""Behavior tests for the request step of the auto-run skill's ship.sh."""

import os
import shlex
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SHIP = ROOT / "ai/skills/auto-run/scripts/ship.sh"
# Runs only step_request from ship.sh, since its earlier steps need a
# built branch and a remote.
RUN_REQUEST_STEP = """
. "$1/env"
eval "$(sed -n '/^step_request() {/,/^}/p' "$2")"
step_request
"""


@pytest.fixture
def gh_write_called(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A file that a fake pde-gh-write on PATH creates when it runs."""
    called = tmp_path / "called"
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    fake = bin_dir / "pde-gh-write"
    fake.write_text(f"#!/bin/sh\ntouch {shlex.quote(str(called))}\nexit 3\n")
    fake.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    return called


@pytest.mark.parametrize(
    "result",
    [
        pytest.param("exit 4 at 2026-01-01T00:00:00Z", id="declined"),
        pytest.param("exit 6 at 2026-01-01T00:00:00Z", id="interrupted"),
    ],
)
def test_step_request_skips_declined_or_interrupted(
    tmp_path: Path, gh_write_called: Path, result: str
) -> None:
    """The request step files nothing after a decline or an interruption.

    The user may run requests.sh twice, and pde-gh-write files a fresh
    request once the old one is four hours old. Without this stop, a
    pull request the user declined would come back to the phone.
    """
    folder = shlex.quote(str(tmp_path))
    (tmp_path / "env").write_text(f"D={folder}\nRUN={folder}\n")
    (tmp_path / "request.sh").write_text("pde-gh-write pr create\n")
    (tmp_path / "request.result").write_text(f"{result}\n")

    run = subprocess.run(
        ["bash", "-c", RUN_REQUEST_STEP, "bash", str(tmp_path), str(SHIP)],
        capture_output=True,
        text=True,
        check=False,
    )

    assert run.returncode == 1
    assert not gh_write_called.exists()


@pytest.mark.parametrize(
    ("status", "lines"),
    [
        pytest.param("waiting for approval in the popup", 1, id="new"),
        pytest.param("still waiting for approval of id", 0, id="rerun"),
    ],
)
def test_step_request_logs_only_new_requests(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, status: str, lines: int
) -> None:
    """The request step adds a requests.log line only for a new request.

    The auto hub queues an expired request again four hours after its
    newest line. A line for each rerun of a waiting request would push
    that time past the real expiry, so the request would sit expired.
    """
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    fake = bin_dir / "pde-gh-write"
    fake.write_text(f"#!/bin/sh\necho 'pde-gh-write: {status}' >&2\nexit 3\n")
    fake.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    folder = shlex.quote(str(tmp_path))
    (tmp_path / "env").write_text(
        f"D={folder}\nRUN={folder}\nW={folder}\nPARENT=main\nBR=feat/x\n"
    )
    (tmp_path / "FILE").write_text("feat: retry uploads\n\nBody.\n")
    log = tmp_path / "requests.log"
    log.write_text("")

    run = subprocess.run(
        ["bash", "-c", RUN_REQUEST_STEP, "bash", str(tmp_path), str(SHIP)],
        capture_output=True,
        text=True,
        check=False,
    )

    assert run.returncode == 3
    assert len(log.read_text().splitlines()) == lines
