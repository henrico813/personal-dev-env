"""Behavior tests for the herdr-hub skill's send script."""

import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SEND = ROOT / "ai/skills/herdr-hub/scripts/send.sh"

FAKE_HERDR = """#!/bin/sh
printf '%s\\n' "$*" >> "$FAKE/calls.log"
[ "$1" = --machine ] && shift 2
case "$1 $2" in
"agent get")
  left=$(cat "$FAKE/focused_reads")
  if [ "$left" -gt 0 ]; then
    echo $((left - 1)) > "$FAKE/focused_reads"
    echo '{"result":{"agent":{"focused":true}}}'
  else
    echo '{"result":{"agent":{"focused":false}}}'
  fi ;;
"agent wait") exit "$(cat "$FAKE/wait_status")" ;;
"agent prompt") printf '%s' "$4" > "$FAKE/message" ;;
*) exit 1 ;;
esac
"""


@pytest.fixture
def fake_herdr(tmp_path: Path) -> Path:
    """A folder with a fake herdr command and a sleep that returns at once.

    The fake herdr records each call and answers from files instead of reaching
    real sessions, so no test sends a real message.

    Args:
        tmp_path: Test directory to make the folder in.

    Returns:
        The folder. Its calls.log lists each herdr call, message holds
        the prompt text, focused_reads counts how many more times the pane
        reads as focused, and wait_status is the exit code of every agent
        wait.
    """
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    (bin_dir / "herdr").write_text(FAKE_HERDR)
    (bin_dir / "sleep").write_text("#!/bin/sh\nexit 0\n")
    for tool in bin_dir.iterdir():
        tool.chmod(0o755)
    (tmp_path / "focused_reads").write_text("0\n")
    (tmp_path / "wait_status").write_text("0\n")
    return tmp_path


def run_send(fake: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run send.sh with the fake herdr, using fake as the run folder.

    Args:
        fake: Folder from the fake fixture.
        args: Arguments after the script name, without RUN_FOLDER.

    Returns:
        The finished process, with its output captured as text.
    """
    *options, name, message = args
    env = os.environ | {
        "FAKE": str(fake),
        "PATH": f"{fake / 'bin'}{os.pathsep}{os.environ['PATH']}",
    }
    return subprocess.run(
        ["bash", str(SEND), *options, name, str(fake), message],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
    )


def read_calls(fake: Path) -> list[str]:
    """Return each herdr call the script made, in order.

    Args:
        fake: Folder from the fake fixture.

    Returns:
        One string per call, its arguments joined by spaces.
    """
    return (fake / "calls.log").read_text().splitlines()


class TestSendScript:
    """send.sh, which the hub uses for every message to a session."""

    def test_send_delivers_exact_message_once(self, fake_herdr: Path) -> None:
        """A free session gets the message once, word for word.

        A message split or expanded by the shell would arrive garbled, and a
        second prompt would repeat it. Without --wait and the closing exit
        line, the hub could not tell when the session has answered.
        """
        message = 'Rebase on "main"; then reply with $HOME as typed.'

        result = run_send(fake_herdr, "docs-fix", message)

        assert result.returncode == 0
        assert (fake_herdr / "message").read_text() == message
        prompts = [
            c for c in read_calls(fake_herdr) if c.startswith("agent prompt")
        ]
        assert len(prompts) == 1
        assert prompts[0].endswith(" --wait")
        log = (fake_herdr / "send-docs-fix.txt").read_text()
        assert log.splitlines()[-1] == "exit 0"

    def test_send_passes_machine_to_every_call(self, fake_herdr: Path) -> None:
        """With --machine, every herdr call goes to that machine.

        A call that loses --machine asks this machine's Herdr about the
        session, or sends the message to a session of the same name here.
        """
        result = run_send(
            fake_herdr,
            "--machine",
            "laptop",
            "docs-fix",
            "Rebase on main.",
        )

        assert result.returncode == 0
        calls = read_calls(fake_herdr)
        assert calls[-1].startswith("--machine laptop agent prompt")
        assert all(call.startswith("--machine laptop ") for call in calls)

    def test_send_waits_while_pane_focused(self, fake_herdr: Path) -> None:
        """No prompt goes out while the user's pane is focused.

        A message typed into a focused pane mixes with what the user is typing
        there.
        """
        (fake_herdr / "focused_reads").write_text("2\n")

        result = run_send(fake_herdr, "docs-fix", "Rebase on main.")

        assert result.returncode == 0
        verbs = [" ".join(call.split()[:2]) for call in read_calls(fake_herdr)]
        assert verbs == [
            "agent wait",
            "agent get",
            "agent get",
            "agent get",
            "agent prompt",
        ]

    def test_send_refuses_while_earlier_send_pending(
        self, fake_herdr: Path
    ) -> None:
        """A second send to a session with one pending exits 2.

        Two pending sends would deliver the same message twice once the session
        is free. The refused send also leaves the pending file alone, because
        the first reply lands there.
        """
        pending = fake_herdr / "send-docs-fix.txt"
        pending.write_text("waiting\n")

        result = run_send(fake_herdr, "docs-fix", "Rebase on main.")

        assert result.returncode == 2
        assert pending.read_text() == "waiting\n"
        assert not (fake_herdr / "calls.log").exists()

    def test_send_sends_after_earlier_send_finished(
        self, fake_herdr: Path
    ) -> None:
        """A send file that ends with an exit line does not block a new send.

        A wrong pending check would refuse every later message to that
        session.
        """
        (fake_herdr / "send-docs-fix.txt").write_text(
            "earlier reply\nexit 0\n"
        )

        result = run_send(fake_herdr, "docs-fix", "Rebase on main.")

        assert result.returncode == 0
        assert (fake_herdr / "message").read_text() == "Rebase on main."

    def test_send_skips_prompt_when_wait_fails(self, fake_herdr: Path) -> None:
        """A failed wait sends nothing and leaves a nonzero exit line.

        The wait fails when the session stays blocked past the timeout.
        A prompt sent anyway would hit a blocked session, and an exit 0
        would tell the hub the message arrived.
        """
        (fake_herdr / "wait_status").write_text("1\n")

        run_send(fake_herdr, "docs-fix", "Rebase on main.")

        assert not (fake_herdr / "message").exists()
        log = (fake_herdr / "send-docs-fix.txt").read_text()
        assert log.splitlines()[-1] == "exit 1"
