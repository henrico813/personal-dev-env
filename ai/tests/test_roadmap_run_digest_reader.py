"""Behavior tests for the roadmap-run skill's digest reader.

No test calls a model. The quote check runs on answer files, and a fake
``opencode`` on PATH stands in for the model.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/roadmap-run/scripts/digest_reader.py"
DIGEST = """\
/notes/Hub.md
12:The hub keeps one inbox per
13-session, under run/sends.
14-Held text waits there.
"""
QUOTE = "The hub keeps one inbox per session"


def write_digest(folder: Path) -> Path:
    """Write a digest with one passage that wraps over two lines.

    Args:
        folder: The folder to write the digest in.

    Returns:
        The digest's path.
    """
    digest = folder / "design-H1.md"
    digest.write_text(DIGEST)
    return digest


def run_reader(
    *args: str, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess:
    """Run digest_reader.py with the given arguments.

    Args:
        args: The command and its arguments.
        env: The environment, or None for this process's.

    Returns:
        The finished run.
    """
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


def ask_with_fake_model(
    tmp_path: Path, reply: str | None
) -> subprocess.CompletedProcess:
    """Run ask with a fake ``opencode`` that prints the reply.

    Args:
        tmp_path: The test's folder.
        reply: The fake model's reply, or None for no ``opencode``.

    Returns:
        The finished run.
    """
    digest = write_digest(tmp_path)
    bin_folder = tmp_path / "bin"
    bin_folder.mkdir()
    if reply is not None:
        fake = bin_folder / "opencode"
        fake.write_text(f"#!/bin/sh\necho '> build'\necho '{reply}'\n")
        fake.chmod(0o755)
    # PATH holds only the fake, so a real opencode never runs.
    env = {**os.environ, "PATH": str(bin_folder)}
    return run_reader(
        "ask",
        "--model",
        "local",
        str(digest),
        "Where is the inbox?",
        env=env,
    )


class TestCheck:
    """``check`` accepts a quote only on the line it cites."""

    @pytest.mark.parametrize(
        ("line", "quote", "exit_code"),
        [
            pytest.param(2, QUOTE, 0, id="right-quote"),
            pytest.param(1, QUOTE, 1, id="wrong-line"),
            pytest.param(2, "The hub keeps two inboxes", 1, id="made-up"),
        ],
    )
    def test_passes_only_quote_on_cited_line(
        self, tmp_path: Path, line: int, quote: str, exit_code: int
    ) -> None:
        """A quote passes only when it starts on its cited digest line.

        Planners act on the answer and read around the cited line. If
        the check passed a made-up quote or a wrong line, planners
        would build from text the design does not hold.
        """
        digest = write_digest(tmp_path)
        answer = tmp_path / "answer.json"
        answer.write_text(json.dumps({"line": line, "quote": quote}))

        result = run_reader("check", str(digest), str(answer))

        assert result.returncode == exit_code


class TestAsk:
    """``ask`` hands back only a quote that passed the check."""

    @pytest.mark.parametrize(
        ("line", "exit_code"),
        [
            pytest.param(2, 0, id="right-line"),
            pytest.param(4, 1, id="wrong-line"),
        ],
    )
    def test_prints_only_quote_on_cited_line(
        self, tmp_path: Path, line: int, exit_code: int
    ) -> None:
        """ask prints the model's answer only when its quote is on its line.

        A planner trusts what ask prints. Without the check a wrong quote
        would reach the plan, and a refused right quote would send the
        planner back to reading the whole digest.
        """
        reply = json.dumps(
            {"answer": "per session", "line": line, "quote": QUOTE}
        )

        result = ask_with_fake_model(tmp_path, reply)

        assert result.returncode == exit_code
        assert result.stdout == (f"{reply}\n" if exit_code == 0 else "")

    @pytest.mark.parametrize(
        "reply",
        [
            pytest.param("I could not find it.", id="no-json-reply"),
            pytest.param(None, id="no-opencode"),
        ],
    )
    def test_reports_error_when_no_answer(
        self, tmp_path: Path, reply: str | None
    ) -> None:
        """ask prints one error line and exits 1 when no answer comes back.

        Agents run ask unattended and read its error to decide whether to
        retry. A Python traceback would bury that reason.
        """
        result = ask_with_fake_model(tmp_path, reply)

        assert result.returncode == 1
        assert result.stderr.startswith("error: ")
        assert result.stdout == ""
