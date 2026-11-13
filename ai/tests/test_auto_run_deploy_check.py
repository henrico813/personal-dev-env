"""Behavior tests for the auto-run skill's deploy check script."""

import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/auto-run/scripts/deploy-check.sh"


def run_deploy_check(
    run_folder: Path, checks: str
) -> subprocess.CompletedProcess:
    """Run deploy-check.sh in a PR folder whose deploy.checks holds CHECKS.

    Args:
        run_folder: Stands in for an auto run's PR folder.
        checks: The try lines to write to deploy.checks.

    Returns:
        The finished run of deploy-check.sh.
    """
    shutil.copy(SCRIPT, run_folder)
    (run_folder / "env").write_text(f"D='{run_folder}'\n")
    (run_folder / "deploy.checks").write_text(checks)
    return subprocess.run(
        ["bash", str(run_folder / "deploy-check.sh")],
        capture_output=True,
        text=True,
        check=False,
    )


class TestDeployCheck:
    """deploy-check.sh runs the try lines and writes deploy.txt."""

    @pytest.mark.parametrize(
        ("checks", "last_line", "exit_code"),
        [
            pytest.param(
                "try build 'echo built'\ntry send 'echo sent hello' sent\n",
                "done",
                0,
                id="all-pass",
            ),
            pytest.param(
                "try build 'echo built'\ntry send 'exit 3'\n",
                "FAIL send: exit 3",
                1,
                id="command-fails",
            ),
            pytest.param(
                "try send 'echo held hello' sent\n",
                "FAIL send: output lacks sent",
                1,
                id="output-lacks-text",
            ),
        ],
    )
    def test_writes_done_only_when_all_pass(
        self, tmp_path: Path, checks: str, last_line: str, exit_code: int
    ) -> None:
        """deploy.txt ends in done only when every try line passed.

        The orchestrator reads done as a passed try and ships the pull
        request. A done line after a failed try, such as a command that
        exits 0 but prints the wrong result, would ship a broken change.
        """
        result = run_deploy_check(tmp_path, checks)

        lines = (tmp_path / "deploy.txt").read_text().splitlines()
        assert lines[-1] == last_line
        assert result.returncode == exit_code
