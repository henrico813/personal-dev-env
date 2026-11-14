"""Tests for ``start`` and ``commit`` against temporary repos."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Callable
from pathlib import Path

import pytest
from conftest import (
    MESSAGE,
    OTHER,
    Reply,
    Repos,
    Run,
    install_skill,
    run_git,
    write,
    write_settings,
)

WORK_SKILL = """\
---
name: tool
description: Use when a test needs a work skill.
metadata:
  scope: work
---

# Tool

Use the tool with care.
"""


class TestStart:
    """Getting the fix worktree ready with ``start``."""

    @pytest.mark.parametrize(
        ("skill", "repo", "skills_dir"),
        [
            pytest.param("demo", "personal", "ai/skills", id="personal"),
            pytest.param("tool", "work", "skills", id="work"),
        ],
    )
    def test_start_routes_skill_by_scope_tag(
        self,
        run: Run,
        repos: Repos,
        tmp_path: Path,
        skill: str,
        repo: str,
        skills_dir: str,
    ) -> None:
        """A skill's scope tag picks the repo whose worktree it gets.

        A work skill routed to the personal repo would put work text in
        a public repository, and a personal one routed to the work repo
        would never reach the other PCs.
        """
        work = tmp_path / "work"
        run_git(tmp_path, "init", "--quiet", "-b", "main", str(work))
        write(work / "skills/tool/SKILL.md", WORK_SKILL)
        run_git(work, "add", ".")
        run_git(work, "commit", "--quiet", "-m", "Add tool")
        run_git(
            tmp_path,
            "clone",
            "--quiet",
            "--bare",
            str(work),
            str(tmp_path / "work.git"),
        )
        run_git(work, "remote", "add", "origin", str(tmp_path / "work.git"))
        run_git(work, "fetch", "--quiet", "origin")
        install_skill(repos.home, "tool", WORK_SKILL)
        write_settings(
            repos.home,
            install_path=str(repos.checkout),
            skill_fix_label="desk",
            work_skills_repo=str(work),
        )

        reply = run("start", skill)

        worktree = repos.home / ".local/share/skill-fix" / repo
        assert reply.code == 0, reply.body
        assert reply.body["repo"] == repo
        assert reply.body["file"] == str(
            worktree / skills_dir / skill / "SKILL.md"
        )
        assert Path(reply.body["file"]).is_file()

    def test_start_reports_edits_left_from_before(
        self, run: Run, repos: Repos
    ) -> None:
        """``start`` says when the skill has edits never committed.

        The next commit takes every edit in the skill's folder. Without
        the warning, an abandoned fix would ride along with the next
        one the user approved.
        """
        first = run("start", "demo")
        skill_md = Path(first.body["file"])
        skill_md.write_text(skill_md.read_text() + "An abandoned fix.\n")

        again = run("start", "demo")

        assert first.body["edits"] is False
        assert again.body["edits"] is True


class TestCommit:
    """Checking the fix rules and committing with ``commit``."""

    def test_leaves_user_checkout_alone(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """The user's checkout keeps its branch and its unsaved work.

        The checkout may hold work in progress on another branch. A fix
        that switched its branch or touched its files could lose that
        work.
        """
        run_git(repos.checkout, "switch", "--quiet", "-c", "mine")
        skill_md = repos.checkout / "ai/skills/demo/SKILL.md"
        skill_md.write_text(skill_md.read_text() + "My own edit.\n")
        (repos.checkout / "notes.txt").write_text("draft\n")
        before = run_git(repos.checkout, "status", "--porcelain", "--branch")

        reply = fix()

        assert reply.body["status"] == "committed", reply.body
        assert (
            run_git(repos.checkout, "status", "--porcelain", "--branch")
            == before
        )
        assert skill_md.read_text().endswith("My own edit.\n")

    @pytest.mark.parametrize(
        ("change", "rule", "detail"),
        [
            pytest.param(
                {"extra": "ai/README.md"},
                "other-files",
                "ai/README.md",
                id="other-files",
            ),
            pytest.param(
                {"message": MESSAGE.split("\nIntent:")[0] + "\n"},
                "message",
                "no Intent: line",
                id="message",
            ),
            pytest.param(
                {"line": "Check " + "each step " * 10 + "before you go on."},
                "width",
                "ai/skills/demo/SKILL.md:12",
                id="width",
            ),
        ],
    )
    def test_commit_refuses_fix_breaking_a_rule(
        self,
        fix: Callable[..., Reply],
        repos: Repos,
        change: dict[str, str],
        rule: str,
        detail: str,
    ) -> None:
        """A fix that breaks a rule is refused, and nothing is committed.

        Each commit must be one skill's readable fix with a message an
        agent can redo after a conflict. The edits stay, so the agent
        can correct them and commit again.
        """
        start = run_git(repos.checkout, "rev-parse", "github/main")

        reply = fix(**change)

        assert (reply.code, reply.body["status"]) == (1, "refused")
        assert (reply.body["rule"], reply.body["detail"]) == (rule, detail)
        assert run_git(repos.worktree, "rev-parse", "HEAD") == start
        assert run_git(repos.worktree, "status", "--porcelain") != ""

    @pytest.mark.parametrize(
        ("change", "detail"),
        [
            pytest.param(
                {"line": "Ask Exname before you merge."},
                "ai/skills/demo/SKILL.md:12",
                id="line",
            ),
            pytest.param(
                {"extra": "ai/skills/demo/references/exname.md"},
                "(path withheld)",
                id="file-name",
            ),
            pytest.param(
                {"extra": "ai/skills/demo/references/éxname.md"},
                "(path withheld)",
                id="non-ascii-file-name",
            ),
            pytest.param(
                {
                    "extra": "ai/skills/demo/references/über.md",
                    "extra_text": "Ask Exname first.\n",
                },
                "ai/skills/demo/references/über.md:1",
                id="non-ascii-file-text",
            ),
            pytest.param(
                {
                    "extra": "ai/skills/demo/references/exname.md",
                    "extra_text": "Ask Exname first.\n",
                },
                "(path withheld)",
                id="file-name-and-text",
            ),
            pytest.param(
                {
                    "extra": "ai/skills/demo/references/my exname.md",
                    "extra_text": "Ask Exname first.\n",
                },
                "(path withheld)",
                id="spaced-file-name-and-text",
            ),
        ],
    )
    def test_commit_refuses_fix_holding_private_term(
        self,
        fix: Callable[..., Reply],
        repos: Repos,
        monkeypatch: pytest.MonkeyPatch,
        change: dict[str, str],
        detail: str,
    ) -> None:
        """A fix with a private term is refused before it commits.

        Sync's scan before each push refuses the same text, so a fix
        committed with it would stop every later sync. File names go
        public too, and git quotes a non-ASCII one unless asked not to.
        The reply names the place, never the term, so a file whose name
        holds the term has every place in it withheld.
        """
        monkeypatch.setenv("PRIVATE_TERMS", "exname\néxname")
        start = run_git(repos.checkout, "rev-parse", "github/main")

        reply = fix(**change)

        assert (reply.code, reply.body["rule"], reply.body["detail"]) == (
            1,
            "scan",
            detail,
        )
        assert (
            "exname" not in json.dumps(reply.body, ensure_ascii=False).lower()
        )
        assert run_git(repos.worktree, "rev-parse", "HEAD") == start

    def test_commit_refuses_unplaced_scan_finding(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A scan refusal with no line still refuses the fix.

        Sync's scan before each push would refuse the same text, so
        letting it commit would stop every later sync.
        """
        write(repos.home / "stub/text-mode", "unplaced\n")

        reply = fix()

        assert (reply.code, reply.body["rule"], reply.body["detail"]) == (
            1,
            "scan",
            "(path withheld)",
        )

    def test_commit_error_shows_only_scan_lines(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A scan that cannot run shows only its own ``scan:`` lines.

        A tool inside ci/scan, such as jq, may quote a private term in
        its error, and the reply reaches the agent and the user.
        """
        write(repos.home / "stub/text-mode", "broken\n")

        reply = fix()

        assert (reply.code, reply.body["status"]) == (2, "error")
        assert reply.body["detail"].endswith(
            "scan: cannot read the private terms"
        )
        assert "exname" not in reply.body["detail"]

    def test_commit_reports_repeats_but_commits(
        self, fix: Callable[..., Reply]
    ) -> None:
        """Words another skill repeats are reported, and the fix commits.

        skill_check.py leaves repeats for the author to judge, since a
        shared phrase is sometimes right. Blocking on one would lose the
        user's approved fix.
        """
        repeated = OTHER.splitlines()[-1]

        reply = fix(repeated)

        assert reply.body["status"] == "committed", reply.body
        assert reply.body["reported"] == [
            {"rule": "repeats", "path": "ai/skills/demo/SKILL.md", "line": 12}
        ]

    def test_commit_holds_fix_matching_work_term(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A fix with a work term goes to a held branch, not the fix branch.

        The personal repo is public, so a work term on its fix branch
        would be pushed. The reply names no term, and the worktree ends
        clean for the next fix.
        """
        terms = repos.home / ".config/pde/work-terms.txt"
        write(terms, "# made-up terms\nexjira\n")
        write_settings(
            repos.home,
            install_path=str(repos.checkout),
            skill_fix_label="desk",
            work_terms_file=str(terms),
        )

        reply = fix("Close the EXJIRA-12 worker first.")

        assert reply.code == 0
        assert reply.body == {
            "status": "held",
            "repo": "personal",
            "branch": "fix/skill-fixes-held-1",
            "held": 1,
        }
        assert "exjira" not in json.dumps(reply.body).lower()
        assert (
            run_git(repos.worktree, "log", "--format=%s", "github/main..HEAD")
            == ""
        )
        assert run_git(repos.worktree, "status", "--porcelain") == ""


class TestHeld:
    """Keeping held fixes off the remote and retrying them."""

    def test_held_branch_refuses_plain_push(
        self, fix: Callable[..., Reply], repos: Repos
    ) -> None:
        """A plain ``git push`` of a held branch fails.

        A held fix holds a work term. The missing push remote is the
        last guard when someone pushes it by hand.
        """
        terms = repos.home / ".config/pde/work-terms.txt"
        write(terms, "exjira\n")
        write_settings(
            repos.home,
            install_path=str(repos.checkout),
            skill_fix_label="desk",
            work_terms_file=str(terms),
        )
        fix("Close the EXJIRA-12 worker first.")
        run_git(repos.worktree, "switch", "--quiet", "fix/skill-fixes-held-1")

        # This config makes a plain push send any branch to github.
        push = subprocess.run(
            [
                "git",
                "-c",
                "push.default=current",
                "-c",
                "remote.pushDefault=github",
                "push",
            ],
            cwd=repos.worktree,
            capture_output=True,
        )

        assert push.returncode != 0
        assert run_git(repos.remote, "branch", "--list", "*held*") == ""

    def test_retry_holds_until_term_is_gone(
        self, fix: Callable[..., Reply], run: Run, repos: Repos
    ) -> None:
        """A retry holds the fix again while its work term is listed.

        Once the term is gone, the fix commits and its held branch is
        deleted. A retry that committed early would push a work term,
        and a held branch left behind would be retried again later.
        """
        terms = repos.home / ".config/pde/work-terms.txt"
        write(terms, "exjira\n")
        write_settings(
            repos.home,
            install_path=str(repos.checkout),
            skill_fix_label="desk",
            work_terms_file=str(terms),
        )
        fix("Close the EXJIRA-12 worker first.")

        still = run("held", "retry", "1")
        log = run_git(repos.worktree, "log", "--format=%s", "github/main..")
        write(terms, "otherterm\n")
        passed = run("held", "retry", "1")

        assert (still.body["status"], still.body["held"]) == ("held", 1)
        assert log == ""
        assert passed.body["status"] == "committed", passed.body
        assert run_git(repos.worktree, "log", "-1", "--format=%s") == (
            "fix(demo): check step 1 before going on"
        )
        assert run_git(repos.checkout, "branch", "--list", "*held*") == ""
