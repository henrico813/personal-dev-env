"""Behavior tests for implement-plan's local-step.sh.

A local model runs the script once per plan step, and the session
that started it reads only the summary file. The tests use a fake
``planner`` whose export writes a known tree, so they need no Go.
"""

import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
BUILD_STEP = ROOT / "ai/skills/implement-plan/scripts/build-step.sh"
FAKE_PLANNER = """\
#!/bin/sh
# The command named in $FAIL_ON fails. Otherwise check passes, and
# export copies $PLANNED to the folder after --out.
[ "$1" = "${FAIL_ON:-}" ] && exit 1
[ "$1" = check ] && exit 0
while [ $# -gt 0 ]; do
  [ "$1" = --out ] && cp -R "$PLANNED" "$2"
  shift
done
"""


def run_git(repo: Path, env: dict[str, str], *args: str) -> str:
    """Run a Git command in the repository and return its output.

    Args:
        repo: The scratch repository.
        env: Environment with the fake planner and a Git identity.
        args: Git arguments.

    Returns:
        Standard output without the trailing newline.
    """
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout.strip()


@pytest.fixture
def env(tmp_path: Path) -> dict[str, str]:
    """An environment whose planner plans app.txt as "new".

    Args:
        tmp_path: Folder for the fake planner and the planned tree.

    Returns:
        The environment with the fake planner first on PATH.
    """
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    planner = bin_dir / "planner"
    planner.write_text(FAKE_PLANNER)
    planner.chmod(0o755)
    planned = tmp_path / "planned"
    planned.mkdir()
    (planned / "app.txt").write_text("new\n")
    (tmp_path / "message").write_text("feat: plan step one\n")
    return {
        **os.environ,
        "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
        "PLANNED": str(planned),
        "GIT_AUTHOR_NAME": "test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }


@pytest.fixture
def repo(tmp_path: Path, env: dict[str, str]) -> Path:
    """A worktree whose one commit holds app.txt as "old".

    Args:
        tmp_path: Folder for the repository.
        env: Environment with a Git identity.

    Returns:
        The repository folder.
    """
    repo = tmp_path / "repo"
    repo.mkdir()
    run_git(repo, env, "init", "-q")
    (repo / "app.txt").write_text("old\n")
    run_git(repo, env, "add", "-A")
    run_git(repo, env, "commit", "-qm", "base")
    return repo


def run_build_step(
    repo: Path, env: dict[str, str], check: str
) -> subprocess.CompletedProcess[str]:
    """Run local-step.sh for step 1 from the repository.

    Args:
        repo: The scratch worktree; its HEAD is passed as the base.
        env: Environment with the fake planner.
        check: The check command, as one string.

    Returns:
        The finished script run.
    """
    tmp_path = repo.parent
    base = run_git(repo, env, "rev-list", "--max-parents=0", "HEAD")
    return subprocess.run(
        [
            "bash",
            str(BUILD_STEP),
            str(tmp_path / "plan.md"),
            base,
            "1",
            str(tmp_path / "message"),
            str(tmp_path / "job"),
            check,
        ],
        cwd=repo,
        env=env,
        capture_output=True,
        text=True,
    )


class TestBuildStep:
    """The script commits the planned tree and reports in summary.txt."""

    def test_passing_step_commits_planned_tree(
        self, repo: Path, env: dict[str, str]
    ) -> None:
        """A passing step commits the planned files with the message.

        The branch must hold exactly what the plan says, with the
        message the session wrote and linted, or the pull request
        differs from the reviewed plan. The summary must end with
        ``done`` so the session counts the step as passed.
        """
        base = run_git(repo, env, "rev-parse", "HEAD")

        result = run_build_step(repo, env, "true")

        assert result.returncode == 0
        summary = repo.parent / "job/summary.txt"
        assert summary.read_text() == "result: pass\nexit: 0\ndone\n"
        assert run_git(repo, env, "rev-parse", "HEAD^") == base
        assert run_git(repo, env, "log", "-1", "--format=%s") == (
            "feat: plan step one"
        )
        assert (repo / "app.txt").read_text() == "new\n"
        assert run_git(repo, env, "status", "--porcelain") == ""

    def test_failing_check_reports_failure_and_done(
        self, repo: Path, env: dict[str, str]
    ) -> None:
        """A failing check still ends the summary with its code and done.

        Without ``done`` the session would wait on a job that ended.
        Without the failure and its last output line, the session
        could not tell a failed step from a passed one or say why it
        moved the step to a stronger model.
        """
        result = run_build_step(repo, env, "echo 'tests broke'; exit 3")

        assert result.returncode == 3
        summary = repo.parent / "job/summary.txt"
        assert summary.read_text() == (
            "result: fail: check; last line: tests broke\nexit: 3\ndone\n"
        )

    def test_rerun_skips_commit_already_made(
        self, repo: Path, env: dict[str, str]
    ) -> None:
        """A rerun of a committed step adds no second commit.

        The session reruns a failed job once on the local model.
        When the first run had committed before its check failed, a
        second commit of the same tree would leave an empty commit
        in the pull request.
        """
        run_build_step(repo, env, "false")

        result = run_build_step(repo, env, "true")

        assert result.returncode == 0
        summary = repo.parent / "job/summary.txt"
        assert summary.read_text() == "result: pass\nexit: 0\ndone\n"
        assert run_git(repo, env, "rev-list", "--count", "HEAD") == "2"

    def test_dirty_worktree_stops_before_commit(
        self, repo: Path, env: dict[str, str]
    ) -> None:
        """Changes left in the worktree stop the step with no commit.

        A file the model or an earlier run left behind is not in the
        plan. The check would then run on code the plan lacks, so a
        pass would not show that the planned step works.
        """
        (repo / "stray.txt").write_text("left behind\n")

        result = run_build_step(repo, env, "true")

        assert result.returncode == 1
        summary = repo.parent / "job/summary.txt"
        assert summary.read_text() == (
            "result: fail: worktree has changes; nothing committed\n"
            "exit: 1\ndone\n"
        )
        assert run_git(repo, env, "rev-list", "--count", "HEAD") == "1"

    @pytest.mark.parametrize(
        ("failing", "result_line"),
        [
            pytest.param(
                "check",
                "result: fail: planner check; see planner.log",
                id="check-fails",
            ),
            pytest.param(
                "export",
                "result: fail: planner export of step 1",
                id="export-fails",
            ),
        ],
    )
    def test_planner_failure_stops_before_commit(
        self,
        repo: Path,
        env: dict[str, str],
        failing: str,
        result_line: str,
    ) -> None:
        """A failed planner check or export ends the summary, no commit.

        Either failure leaves the script with no checked tree to
        commit. The session must still see the reason, the code and
        ``done``, or it would wait on the job or blame the model for
        a plan that does not apply.
        """
        failing_env = {**env, "FAIL_ON": failing}

        result = run_build_step(repo, failing_env, "true")

        assert result.returncode == 1
        summary = repo.parent / "job/summary.txt"
        assert summary.read_text() == f"{result_line}\nexit: 1\ndone\n"
        assert run_git(repo, env, "rev-list", "--count", "HEAD") == "1"
