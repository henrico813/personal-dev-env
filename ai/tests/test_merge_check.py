"""Behavior tests for the pull-request skill's merge check script."""

import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
MERGE_CHECK = ROOT / "ai/skills/pull-request/scripts/merge-check.sh"


def make_isolated_env(tmp_path: Path) -> dict[str, str]:
    """Return an environment where git ignores the developer's config.

    Args:
        tmp_path: Test directory; git does not search above it.

    Returns:
        A copy of the process environment with git isolated and a
        fixed commit author.
    """
    return os.environ | {
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CEILING_DIRECTORIES": str(tmp_path.parent),
        "GIT_AUTHOR_NAME": "Test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "Test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }


def run_git(repo: Path, *args: str) -> None:
    """Fail the test unless the git command succeeds in repo.

    Args:
        repo: Repository to run in.
        args: Git subcommand and options.
    """
    subprocess.run(
        ["git", *args],
        cwd=repo,
        env=make_isolated_env(repo.parent),
        check=True,
        capture_output=True,
    )


def commit_file(repo: Path, name: str) -> None:
    """Add a file named name to repo and commit it.

    Args:
        repo: Repository to commit in.
        name: File to create with one line of text.
    """
    (repo / name).write_text(f"{name}\n")
    run_git(repo, "add", name)
    run_git(repo, "commit", "-q", "-m", f"add {name}")


@pytest.fixture
def pr_repo(tmp_path: Path) -> Path:
    """A repository where main and a PR branch each added a file.

    Args:
        tmp_path: Test directory to make the repository in.

    Returns:
        The repository, with branch main checked out.
    """
    repo = tmp_path / "repo"
    repo.mkdir()
    run_git(repo, "init", "-q", "-b", "main")
    commit_file(repo, "base.txt")
    run_git(repo, "switch", "-q", "-c", "pr")
    commit_file(repo, "pr.txt")
    run_git(repo, "switch", "-q", "main")
    commit_file(repo, "main.txt")
    return repo


def run_merge_check(
    repo: Path, out: Path, *commands: str
) -> subprocess.CompletedProcess[str]:
    """Run the merge check of branch pr into main.

    Args:
        repo: Repository holding both branches; the script runs in it.
        out: Folder for the summary and logs.
        commands: Test commands for the script to run.

    Returns:
        The finished script with captured output.
    """
    return subprocess.run(
        ["bash", str(MERGE_CHECK), "main", "pr", str(out), *commands],
        cwd=repo,
        check=False,
        env=make_isolated_env(repo.parent),
        capture_output=True,
        text=True,
    )


class TestMergeCheck:
    """The merge check tests a PR merged into its base before a merge."""

    def test_commands_run_on_merged_git_checkout(
        self, pr_repo: Path, tmp_path: Path
    ) -> None:
        """Commands see both sides of the merge, tracked by git.

        A copy made with git archive has no index, so tests that call
        git ls-files failed on a merge that was fine and cost several
        turns to explain.
        """
        out = tmp_path / "out"

        result = run_merge_check(
            pr_repo,
            out,
            "git ls-files --error-unmatch base.txt main.txt pr.txt",
        )

        assert result.returncode == 0, result.stdout
        summary = (out / "summary.txt").read_text().splitlines()
        assert summary[-2].startswith("exit 0: git ls-files")
        assert summary[-1] == "done"

    def test_scratch_clone_deleted_after_commands(
        self, pr_repo: Path, tmp_path: Path
    ) -> None:
        """The scratch clone and its state folder are gone after the run.

        Each run clones the whole repository, and tests leave build
        output such as a cargo target folder in it, so kept clones
        would fill the disk over a roadmap run.
        """
        out = tmp_path / "out"

        result = run_merge_check(pr_repo, out, "true")

        assert result.returncode == 0, result.stdout
        summary = (out / "summary.txt").read_text().splitlines()
        assert summary[-1] == "done"

        assert not list(out.glob("work.*"))

    def test_merge_check_rejects_conflicting_merge(
        self, pr_repo: Path, tmp_path: Path
    ) -> None:
        """Reject conflicting edits before running tests.

        A conflict must stop the merge check so a PR cannot pass tests
        against a tree that would not merge cleanly.
        """
        out = tmp_path / "out"
        run_git(pr_repo, "switch", "-q", "pr")
        (pr_repo / "base.txt").write_text("PR change\n")
        run_git(pr_repo, "add", "base.txt")
        run_git(pr_repo, "commit", "-q", "-m", "change base on PR")
        run_git(pr_repo, "switch", "-q", "main")
        (pr_repo / "base.txt").write_text("main change\n")
        run_git(pr_repo, "add", "base.txt")
        run_git(pr_repo, "commit", "-q", "-m", "change base on main")

        result = run_merge_check(pr_repo, out, "true")

        assert result.returncode == 1
        summary = (out / "summary.txt").read_text()
        assert "no clean merge" in summary

    def test_failed_command_fails_without_skipping_others(
        self, pr_repo: Path, tmp_path: Path
    ) -> None:
        """A failed command fails the check, and later commands still run.

        A merge check that passed with a failing suite would let a
        broken PR merge. Running every command gives the whole picture
        in one run instead of one failure per run.
        """
        out = tmp_path / "out"

        result = run_merge_check(pr_repo, out, "false", "true")

        assert result.returncode == 1
        summary = (out / "summary.txt").read_text().splitlines()
        assert summary[-3].startswith("exit 1: false")
        assert summary[-2].startswith("exit 0: true")
        assert summary[-1] == "done"
