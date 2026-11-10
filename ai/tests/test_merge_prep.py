"""Behavior tests for the pull-request skill's merge prep script."""

import json
import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
MERGE_PREP = ROOT / "ai/skills/pull-request/scripts/merge_prep.py"
PR_NUMBER = 7
GOOD_TITLE = "feat: add the report page"
OVERVIEW = "Users can now see their report in one page."
FAKE_GH = """#!/bin/sh
case "$2" in
  view) cat "$FAKE_GH_DIR/view.json" ;;
  diff) cat "$FAKE_GH_DIR/files.txt" ;;
esac
"""


def make_isolated_env(tmp_path: Path) -> dict[str, str]:
    """Return an environment where git ignores the developer's config.

    Args:
        tmp_path: Test directory; git does not search above it.

    Returns:
        A copy of the process environment with git isolated, a fixed
        commit author, and the fake gh first on PATH.
    """
    return os.environ | {
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CEILING_DIRECTORIES": str(tmp_path),
        "GIT_AUTHOR_NAME": "Test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "Test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
        "FAKE_GH_DIR": str(tmp_path / "gh"),
        "PATH": f"{tmp_path / 'gh'}{os.pathsep}{os.environ['PATH']}",
    }


def run_git(repo: Path, *args: str) -> str:
    """Run a git command in repo, failing the test when it fails.

    Args:
        repo: Repository to run in.
        args: Git subcommand and options.

    Returns:
        The command's standard output.
    """
    return subprocess.run(
        ["git", *args],
        cwd=repo,
        env=make_isolated_env(repo.parent),
        check=True,
        capture_output=True,
        text=True,
    ).stdout


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
def clone(tmp_path: Path) -> Path:
    """A clone of a GitHub-like remote whose PR 7 adds a file to main.

    Args:
        tmp_path: Test directory for the remote, the clone and the
            fake gh folder.

    Returns:
        The clone, with the remote named origin.
    """
    remote = tmp_path / "remote"
    remote.mkdir()
    run_git(remote, "init", "-q", "-b", "main")
    commit_file(remote, "base.txt")
    run_git(remote, "switch", "-q", "-c", "pr")
    commit_file(remote, "pr.txt")
    run_git(remote, "update-ref", f"refs/pull/{PR_NUMBER}/head", "pr")
    run_git(remote, "switch", "-q", "main")
    commit_file(remote, "main.txt")
    run_git(tmp_path, "clone", "-q", str(remote), "clone")
    gh = tmp_path / "gh"
    gh.mkdir()
    (gh / "gh").write_text(FAKE_GH)
    (gh / "gh").chmod(0o755)
    return tmp_path / "clone"


def write_pr(
    clone: Path,
    title: str = GOOD_TITLE,
    checks: list[dict] | None = None,
    files: int = 2,
) -> None:
    """Make the fake gh describe PR 7 with these values.

    Args:
        clone: Clone made by the ``clone`` fixture.
        title: PR title.
        checks: ``statusCheckRollup`` entries; one passing check run
            by default.
        files: How many changed files ``gh pr diff`` lists.
    """
    head = run_git(clone, "rev-parse", "origin/pr").strip()
    pr = {
        "title": title,
        "body": f"## Overview\n\n{OVERVIEW}\n\n## Changes\n\n- Page.\n",
        "state": "OPEN",
        "isDraft": False,
        "mergeable": "MERGEABLE",
        "baseRefName": "main",
        "headRefOid": head,
        "statusCheckRollup": checks
        or [make_check_run("test", "COMPLETED", "SUCCESS")],
    }
    gh = clone.parent / "gh"
    (gh / "view.json").write_text(json.dumps(pr))
    (gh / "files.txt").write_text(
        "".join(f"src/file{n}.py\n" for n in range(files))
    )


def make_check_run(name: str, status: str, conclusion: str | None) -> dict:
    """Return a GitHub check run as ``gh pr view`` reports it.

    Args:
        name: Check name.
        status: Such as ``COMPLETED`` or ``IN_PROGRESS``.
        conclusion: Such as ``SUCCESS``; None while it runs.

    Returns:
        One ``statusCheckRollup`` entry.
    """
    return {
        "__typename": "CheckRun",
        "name": name,
        "status": status,
        "conclusion": conclusion,
    }


def run_merge_prep(
    clone: Path, *commands: str
) -> subprocess.CompletedProcess[str]:
    """Run the merge prep script on PR 7 from the clone.

    Args:
        clone: Clone made by the ``clone`` fixture.
        commands: Test commands for the script.

    Returns:
        The finished script with captured output.
    """
    return subprocess.run(
        [
            "python3",
            str(MERGE_PREP),
            str(PR_NUMBER),
            *commands,
            "--out",
            str(clone.parent / "out"),
        ],
        cwd=clone,
        env=make_isolated_env(clone.parent),
        check=False,
        capture_output=True,
        text=True,
    )


class TestMergePrep:
    """The merge prep script runs every merge check and says if it is ready."""

    def test_ready_merge_gets_summary_and_message(self, clone: Path) -> None:
        """A ready PR, tested merged into main, gets a summary and message.

        The test command passes only when both the PR's file and main's
        file are present, so it proves the tests ran on the merge.

        The summary is all a small local model reads before a merge, so
        it must stay short even for a PR that changes many files. The
        squash message and body are what the merge command sends, so
        they must hold the PR's title and Overview.
        """
        write_pr(clone, files=30)

        result = run_merge_prep(clone, "test -f pr.txt -a -f main.txt")

        assert result.returncode == 0, result.stdout
        lines = result.stdout.splitlines()
        assert len(lines) <= 20
        assert lines[-1] == "result: ready"
        assert "files: 30, all in" in result.stdout
        out = clone.parent / "out"
        assert (out / "squash.txt").read_text() == (
            f"{GOOD_TITLE}\n\n{OVERVIEW}\n"
        )
        assert (out / "body.txt").read_text() == f"{OVERVIEW}\n"

    @pytest.mark.parametrize(
        ("command", "title", "checks", "reasons"),
        [
            pytest.param(
                "true",
                GOOD_TITLE,
                [make_check_run("lint", "IN_PROGRESS", None)],
                ["checks"],
                id="pending-check",
            ),
            pytest.param(
                "true",
                "Add the report page",
                None,
                ["squash message"],
                id="bad-title",
            ),
            pytest.param(
                "false",
                GOOD_TITLE,
                [make_check_run("lint", "COMPLETED", "FAILURE")],
                ["tests", "checks"],
                id="failing-test-and-check",
            ),
        ],
    )
    def test_blocking_problem_makes_pr_not_ready(
        self,
        clone: Path,
        command: str,
        title: str,
        checks: list[dict] | None,
        reasons: list[str],
    ) -> None:
        """Each blocking problem is named, and none stops the others.

        A local model merges when the last line says ready, so a missed
        failure would merge a broken PR. Naming every problem in one
        run saves a run per problem.
        """
        write_pr(clone, title=title, checks=checks)

        result = run_merge_prep(clone, command)

        assert result.returncode == 1, result.stdout
        last = result.stdout.splitlines()[-1]
        assert last.startswith("result: not ready: ")
        assert last.removeprefix("result: not ready: ").split("; ") == reasons
