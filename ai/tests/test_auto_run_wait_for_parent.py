"""Behavior tests for the auto-run skill's wait-for-parent script."""

import os
import shlex
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/auto-run/scripts/wait-for-parent.sh"
PARENT = "feat/demo-12-sync-log"
GH_ARGS = f"pr view {PARENT} --json state --jq .state"


@pytest.fixture
def fake_gh(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A gh on PATH that finds the parent's pull request on its third call.

    Before that it fails as gh does when a branch has no pull request.
    It also fails on any other arguments, such as a missing ``--jq``,
    when run outside the worktree, or once parent.txt exists.
    """
    count = shlex.quote(str(tmp_path / "gh-calls"))
    worktree = shlex.quote(str(tmp_path / "worktree"))
    parent_file = shlex.quote(str(tmp_path / "pr" / "parent.txt"))
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    fake = bin_dir / "gh"
    fake.write_text(
        "#!/bin/sh\n"
        f"n=$(($(cat {count} 2>/dev/null || echo 0) + 1))\n"
        f"echo $n > {count}\n"
        f'[ "$*" = "{GH_ARGS}" ] || exit 1\n'
        f'[ "$PWD" = {worktree} ] || exit 1\n'
        f"[ ! -e {parent_file} ] || exit 1\n"
        '[ "$n" -ge 3 ] || '
        "{ echo 'no pull requests found for branch' >&2; exit 1; }\n"
        "echo OPEN\n"
    )
    fake.chmod(0o755)
    monkeypatch.setenv("PATH", f"{bin_dir}{os.pathsep}{os.environ['PATH']}")
    monkeypatch.setenv("PARENT_POLL_SECONDS", "0")


def run_wait_for_parent(pr_folder: Path) -> subprocess.CompletedProcess:
    """Run wait-for-parent.sh from a new PR folder with a worktree beside it.

    Args:
        pr_folder: Stands in for an auto run's PR folder; made here.

    Returns:
        The finished run of wait-for-parent.sh.
    """
    worktree = pr_folder.parent / "worktree"
    pr_folder.mkdir()
    worktree.mkdir()
    shutil.copy(SCRIPT, pr_folder)
    (pr_folder / "env").write_text(
        f"D={shlex.quote(str(pr_folder))}\n"
        f"W={shlex.quote(str(worktree))}\n"
        f"PARENT={PARENT}\n"
    )
    return subprocess.run(
        ["bash", str(pr_folder / "wait-for-parent.sh")],
        capture_output=True,
        text=True,
        check=False,
        timeout=10,
    )


class TestWaitForParent:
    """wait-for-parent.sh writes parent.txt once the parent PR exists."""

    def test_writes_state_once_parent_pr_opens(
        self, tmp_path: Path, fake_gh: None
    ) -> None:
        """parent.txt holds OPEN after GitHub first reports the pull request.

        A stacked worker files its own request when this file appears.
        Giving up on the first "no pull requests found", or writing the
        file before the parent opens, would leave the child unfiled or
        file it before its parent.
        """
        pr_folder = tmp_path / "pr"

        run = run_wait_for_parent(pr_folder)

        assert run.returncode == 0
        state = (pr_folder / "parent.txt").read_text().split()[0]
        assert state == "OPEN"

    def test_writes_none_after_last_try(
        self,
        tmp_path: Path,
        fake_gh: None,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """parent.txt holds NONE when the tries run out before the PR opens.

        The script runs under setsid nohup, so it outlives the worker's
        turn. Without a last try it would call gh every minute until the
        PC restarts, and the worker would never hear that the parent did
        not open.
        """
        monkeypatch.setenv("PARENT_POLL_TRIES", "2")
        pr_folder = tmp_path / "pr"

        run = run_wait_for_parent(pr_folder)

        assert run.returncode == 0
        state = (pr_folder / "parent.txt").read_text().split()[0]
        assert state == "NONE"
