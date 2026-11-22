"""Tests for ``sync`` against a bare remote and stub gh and pde-gh-write."""

from __future__ import annotations

import shutil
from collections.abc import Callable
from pathlib import Path

import pytest
from conftest import (
    REQUEST_ID,
    Reply,
    Repos,
    Run,
    read_requests,
    run_git,
    save_pr,
    write,
    write_settings,
    write_tool,
)

BRANCH = "fix/demo-lessons"
SSH_URL = "git@github.com:owner/repo.git"
HTTPS_URL = "https://github.com/owner/repo.git"


def push_to_main(repos: Repos, line: int, text: str) -> None:
    """Replace or add one line of demo's SKILL.md on main, as a merge does."""
    run_git(repos.upstream, "pull", "--quiet", "origin", "main")
    skill_md = repos.upstream / "ai/skills/demo/SKILL.md"
    lines = skill_md.read_text().splitlines()
    lines[line - 1 : line] = [text]
    skill_md.write_text("\n".join(lines) + "\n")
    run_git(repos.upstream, "commit", "--quiet", "-am", "Change main")
    run_git(repos.upstream, "push", "--quiet", "origin", "main")


def use_ssh_remote(repos: Repos, target: str) -> None:
    """Set the checkout's remote to a GitHub SSH address.

    Git sends the address's HTTPS form to target, and every SSH
    connection fails at once.
    """
    run_git(repos.checkout, "remote", "set-url", "github", SSH_URL)
    run_git(
        repos.checkout,
        "config",
        "--global",
        f"url.{target}.insteadOf",
        HTTPS_URL,
    )
    run_git(repos.checkout, "config", "--global", "core.sshCommand", "false")


def commit_skills(repos: Repos, *skills: str) -> None:
    """Add the fix worktree and commit a new SKILL.md for each skill."""
    run_git(
        repos.checkout,
        "worktree",
        "add",
        "--quiet",
        "-b",
        "fix/skill-fixes-unpushed",
        str(repos.worktree),
        "github/main",
    )
    for skill in skills:
        write(repos.worktree / f"ai/skills/{skill}/SKILL.md", f"# {skill}\n")
        run_git(repos.worktree, "add", ".")
        run_git(repos.worktree, "commit", "--quiet", "-m", f"fix({skill}): x")


def list_branches(repos: Repos) -> str:
    """Return the remote's branch names, one per line."""
    return run_git(
        repos.remote, "for-each-ref", "--format=%(refname:short)", "refs/heads"
    )


class TestSync:
    """Pushing fix branches and keeping one fix PR per batch."""

    def test_sync_offline_keeps_commit_and_succeeds(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """Offline, the fix commits and the reply is a success.

        Fixes are commits so they survive a PC being offline. An error
        here would make agents retry a fix that is already safe.
        """
        repos.remote.rename(repos.remote.with_suffix(".away"))

        reply = fix()

        assert reply.code == 0
        assert (reply.body["status"], reply.body["sync"]) == (
            "committed",
            "offline",
        )
        assert run_git(repos.worktree, "log", "-1", "--format=%s").startswith(
            "fix(demo): check step 1"
        )

    def test_sync_pushes_ssh_remote_over_https(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A GitHub SSH remote is fetched and pushed by its HTTPS address.

        The update timer has no SSH agent, so a sync over SSH would fail
        there on every run and the fixes would never leave the PC.
        """
        use_ssh_remote(repos, str(repos.remote))

        reply = fix()

        assert reply.body["sync"] == "pr_waiting", reply.body
        assert list_branches(repos) == f"{BRANCH}\nmain"

    @pytest.mark.parametrize(
        ("message", "state", "hint", "code"),
        [
            pytest.param(
                "fatal: Authentication failed for 'https://github.com/'",
                "error",
                True,
                2,
                id="login",
            ),
            pytest.param(
                "fatal: unable to access: Could not resolve host: github.com",
                "offline",
                False,
                0,
                id="offline",
            ),
        ],
    )
    def test_login_failure_errors_but_offline_succeeds(
        self,
        fix: Callable[..., Reply],
        run: Run,
        repos: Repos,
        message: str,
        state: str,
        hint: bool,
        code: int,
    ) -> None:
        """A refused fetch login exits 2 with its fix; offline exits 0.

        A login failure never clears by itself, so reporting it as
        offline would leave the fixes unpushed with no warning, and
        pde-installer update warns only on a nonzero exit.
        """
        fix()
        write_tool(
            repos.home / "stub/git-remote-fake",
            f'#!/bin/sh\necho "{message}" >&2\nexit 128\n',
        )
        use_ssh_remote(repos, "fake::x")

        reply = run("sync")

        entry = reply.body["repos"][0]
        assert (reply.code, entry["sync"]) == (code, state), entry
        assert ("run gh auth setup-git" in entry.get("detail", "")) is hint

    def test_gh_failure_keeps_commit_and_errors(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A failed PR lookup replies error, and the fix stays committed.

        The detail names the commit, so the agent tells the user the fix
        is safe and a later sync pushes it, instead of redoing it.
        """
        write(repos.home / "stub/gh-fails", "")

        reply = fix()

        assert (reply.code, reply.body["status"]) == (2, "error")
        assert reply.body["detail"].startswith("committed ")
        assert run_git(repos.worktree, "log", "-1", "--format=%s").startswith(
            "fix(demo): check step 1"
        )

    def test_sync_continues_after_one_repo_fails(
        self, fix: Callable[..., Reply], run: Run, repos: Repos, tmp_path: Path
    ) -> None:
        """A repo that fails gets its own error; the next repo is still synced.

        A broken login or gh failure in one repo must not keep the
        other repo's fixes from leaving the PC, and the exit code still
        tells pde-installer update to warn.
        """
        fix()
        extra = tmp_path / "extra"
        run_git(tmp_path, "init", "--quiet", "-b", "main", str(extra))
        write_settings(
            repos.home,
            install_path=str(repos.checkout),
            extra_skills_repo=str(extra),
        )
        write(repos.home / "stub/gh-fails", "")

        reply = run("sync")

        states = [(e["repo"], e["sync"]) for e in reply.body["repos"]]
        assert reply.code == 2
        assert states == [("personal", "error"), ("extra", "skipped")]

    def test_sync_files_one_request_per_batch(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A batch makes one PR request, and later fixes join its PR.

        Each request needs the user's approval. A rerun while the first
        waits must repeat the same command and body, which pde-gh-write
        reads as the same request, and an open PR gets no new one.
        """
        first = fix()
        second = fix()
        save_pr(repos.home, BRANCH, number=7, state="OPEN", headRefOid="x")
        third = fix()

        assert (first.body["sync"], first.body["request"]) == (
            "pr_waiting",
            REQUEST_ID,
        )
        assert second.body["sync"] == "pr_waiting"
        assert (third.body["sync"], third.body["pr"]) == ("pushed", 7)
        requests = read_requests(repos.home)
        assert len(requests) == 2
        assert requests[0] == requests[1]
        assert requests[0].startswith(
            f"pr create --base main --head {BRANCH} "
            "--title fix(skills): demo-lessons --body-file "
        )
        pushed = run_git(
            repos.remote, "rev-list", "--count", f"main..{BRANCH}"
        )
        assert pushed == "3"

    def test_sync_renames_old_branch_before_push(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """An unpushed branch with an older name is pushed under its skills.

        Older versions named branches after a PC label. Branch names are
        public, so such a name must never reach the remote.
        """
        run_git(
            repos.checkout,
            "worktree",
            "add",
            "--quiet",
            "-b",
            "fix/skill-fixes-oldlabel-4",
            str(repos.worktree),
            "github/main",
        )

        reply = fix()

        assert (reply.body["branch"], reply.body["sync"]) == (
            BRANCH,
            "pr_waiting",
        )
        assert list_branches(repos) == f"{BRANCH}\nmain"

    def test_branch_name_lists_each_changed_skill(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A batch that changes two skills names both, first changed first.

        The name tells the user which skills a fix PR changes before
        they open it.
        """
        commit_skills(repos, "tidy")

        reply = fix()

        assert reply.body["branch"] == "fix/tidy-demo-lessons", reply.body
        assert list_branches(repos) == "fix/tidy-demo-lessons\nmain"

    def test_long_branch_name_ends_in_and_more(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """Skills that would pass 60 characters become "and-more".

        A branch name that lists every skill of a large batch would be
        too long to read.
        """
        commit_skills(
            repos, "tidy", "plant-watering-schedule", "seed-catalog-cleanup"
        )

        reply = fix()

        assert reply.body["branch"] == (
            "fix/tidy-plant-watering-schedule-and-more-lessons"
        )

    def test_branch_skips_name_of_merged_pr(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A name a merged PR used gets a number, even with its branch gone.

        GitHub deletes a merged branch, and a new branch of the same
        name would be taken for that merged PR and never get its own.
        """
        save_pr(repos.home, BRANCH, number=5, state="MERGED", headRefOid="x")

        reply = fix()

        assert reply.body["branch"] == f"{BRANCH}-2", reply.body

    def test_new_worktree_returns_to_pushed_branch(
        self, fix: Callable[..., Reply], run: Run, repos: Repos
    ) -> None:
        """A worktree deleted by hand comes back on the pushed batch branch.

        A fresh unpushed branch would start a second batch and a second
        fix PR while the first one is still open.
        """
        fix()
        shutil.rmtree(repos.worktree)

        reply = run("start", "demo")

        assert reply.body["branch"] == BRANCH, reply.body

    def test_sync_after_merge_starts_new_batch(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """After a merge, only newer fixes go to a new batch branch.

        The merged branch stays on GitHub, so pushing a new batch to it
        would need a force push. A merged fix that came back would show
        up twice for review.
        """
        fix()
        merged = run_git(repos.worktree, "rev-parse", "HEAD")
        push_to_main(repos, 12, "Check step 1 first.")
        save_pr(
            repos.home, BRANCH, number=7, state="MERGED", headRefOid=merged
        )

        reply = fix()

        new = f"{BRANCH}-2"
        assert (reply.body["branch"], reply.body["sync"]) == (
            new,
            "pr_waiting",
        )
        titles = run_git(repos.remote, "log", "--format=%s", f"main..{new}")
        assert titles == "fix(demo): check step 2 before going on"
        assert run_git(repos.remote, "rev-parse", BRANCH) == merged

    def test_sync_reports_conflict_without_pushing(
        self, fix: Callable[..., Reply], run: Run, repos: Repos
    ) -> None:
        """A same-line conflict with main leaves the fix branch as it was.

        An agent redoes the fix from its Intent line. A half-merged
        branch, here or on GitHub, would leave it nothing clean to
        start from.
        """
        fix()
        head = run_git(repos.worktree, "rev-parse", "HEAD")
        push_to_main(repos, 12, "Check every step twice.")
        save_pr(
            repos.home,
            BRANCH,
            number=7,
            state="OPEN",
            headRefOid=head,
            mergeable="CONFLICTING",
        )

        reply = run("sync")

        assert reply.body["repos"][0]["sync"] == "conflict"
        assert run_git(repos.worktree, "rev-parse", "HEAD") == head
        assert run_git(repos.worktree, "status", "--porcelain") == ""
        assert run_git(repos.remote, "rev-parse", BRANCH) == head

    def test_sync_merges_main_into_conflicting_pr(
        self, fix: Callable[..., Reply], run: Run, repos: Repos
    ) -> None:
        """A fix PR GitHub cannot merge gets main merged in and pushed.

        The remote refuses non-fast-forward pushes, so this also shows
        the sync never rewrites a pushed fix to get past a conflict.
        """
        fix()
        head = run_git(repos.worktree, "rev-parse", "HEAD")
        push_to_main(repos, 8, "# Demo skill")
        save_pr(
            repos.home,
            BRANCH,
            number=7,
            state="OPEN",
            headRefOid=head,
            mergeable="CONFLICTING",
        )

        reply = run("sync")

        assert reply.body["repos"][0]["sync"] == "pushed"
        parents = run_git(repos.remote, "log", "-1", "--format=%P", BRANCH)
        assert head in parents.split()

    def test_sync_runs_scan_before_push(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A refused scan stops the push, and the fix stays committed.

        The scan is the last check before text goes public. The user
        reads its findings before anything leaves the PC.
        """
        write(repos.home / "scan-refuses", "")

        reply = fix()

        assert (reply.body["status"], reply.body["sync"]) == (
            "committed",
            "scan",
        )
        assert list_branches(repos) == "main"
