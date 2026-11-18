"""Tests for the extra repo's fix PR when its PRs are on a forge.

A small HTTP server plays the forge's pulls API, so no test needs a
forge server. It answers only the calls skill-fix makes, with the
fields the API sends.
"""

from __future__ import annotations

import json
import threading
from collections.abc import Callable, Iterator
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import pytest
from conftest import (
    Reply,
    Repos,
    Run,
    install_skill,
    run_git,
    write,
    write_settings,
)

TOKEN = "test-token-not-real"
PULLS = "/api/v1/repos/owner/skills/pulls"
TOOL = """\
---
name: tool
description: Use when a test needs an extra skill.
metadata:
  scope: extra
---

# Tool

Use the tool with care.
"""
MESSAGE = """\
fix(tool): check step {n} before going on

Missed: the agent moved on before it checked step {n}.
Intent: agents check step {n} before they start the next one.
"""


@dataclass
class FakeForge:
    """The fake forge's PRs and the PRs it was asked to open.

    Attributes:
        pulls: Its PRs in the API's fields, oldest first.
        posts: The JSON body of each open request.
    """

    pulls: list[dict] = field(default_factory=list)
    posts: list[dict] = field(default_factory=list)


@dataclass
class Extra:
    """The extra repo's paths.

    Attributes:
        checkout: The extra checkout, on main.
        remote: The bare repository it pushes to.
    """

    checkout: Path
    remote: Path


def make_handler(fake: FakeForge) -> type[BaseHTTPRequestHandler]:
    """Return a handler that answers skill-fix's PR calls from fake."""

    class Handler(BaseHTTPRequestHandler):
        """The forge's pulls API, backed by fake."""

        def do_GET(self) -> None:
            """List every PR, newest first, as sort=recentupdate does."""
            if self.is_refused():
                return
            self.send_json(200, list(reversed(fake.pulls)))

        def do_POST(self) -> None:
            """Open a PR from the body's head branch."""
            if self.is_refused():
                return
            size = int(self.headers["Content-Length"])
            body = json.loads(self.rfile.read(size))
            fake.posts.append(body)
            pr = {
                "number": len(fake.pulls) + 1,
                "state": "open",
                "merged": False,
                "mergeable": True,
                "head": {"ref": body["head"], "sha": "0" * 40},
                "body": body["body"],
            }
            fake.pulls.append(pr)
            self.send_json(201, pr)

        def is_refused(self) -> bool:
            """Answer 401 and return True unless the request has TOKEN."""
            if self.path.startswith(PULLS) and (
                self.headers.get("Authorization") == f"token {TOKEN}"
            ):
                return False
            self.send_json(401, {"message": "token is required"})
            return True

        def send_json(self, status: int, value: object) -> None:
            """Answer with value as JSON."""
            data = json.dumps(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, *args: object) -> None:
            """Keep the server quiet in test output."""

    return Handler


@pytest.fixture
def forge() -> Iterator[tuple[FakeForge, str]]:
    """A fake forge on a free local port, and its repo's web address."""
    fake = FakeForge()
    server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(fake))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield fake, f"http://127.0.0.1:{server.server_port}/owner/skills"
    finally:
        server.shutdown()
        server.server_close()


@pytest.fixture
def extra(repos: Repos, tmp_path: Path, forge: tuple[FakeForge, str]) -> Extra:
    """An extra repo with the tool skill, set up to use the fake forge."""
    checkout = tmp_path / "extra"
    remote = tmp_path / "extra.git"
    run_git(tmp_path, "init", "--quiet", "-b", "main", str(checkout))
    write(checkout / "skills/tool/SKILL.md", TOOL)
    run_git(checkout, "add", ".")
    run_git(checkout, "commit", "--quiet", "-m", "Add tool")
    run_git(tmp_path, "init", "--quiet", "--bare", "-b", "main", str(remote))
    run_git(checkout, "remote", "add", "origin", str(remote))
    run_git(checkout, "push", "--quiet", "-u", "origin", "main")
    install_skill(repos.home, "tool", TOOL)
    token_file = repos.home / ".config/pde/forge-token"
    write(token_file, f"{TOKEN}\n")
    write_settings(
        repos.home,
        install_path=str(repos.checkout),
        skill_fix_label="desk",
        extra_skills_repo=str(checkout),
        extra_forge_repo_url=forge[1],
        extra_forge_token_file=str(token_file),
    )
    return Extra(checkout, remote)


@pytest.fixture
def fix_tool(run: Run, extra: Extra, tmp_path: Path) -> Callable[[], Reply]:
    """Return a function that makes one fix to tool and commits it.

    Each call adds one line to tool's SKILL.md and a correction case,
    as the ``fix`` fixture does for demo.
    """
    count = 0

    def make_fix() -> Reply:
        """Start, append a line to tool, and commit it."""
        nonlocal count
        count += 1
        started = run("start", "tool")
        assert started.code == 0, started.body
        skill_md = Path(started.body["file"])
        skill_md.write_text(skill_md.read_text() + f"Check step {count}.\n")
        case = {"id": f"step-{count}", "rule": "Tool", "correction": True}
        write(skill_md.parent / f"evals/step-{count}.json", json.dumps(case))
        message_file = tmp_path / f"message-{count}"
        message_file.write_text(MESSAGE.format(n=count))
        return run("commit", "tool", "--message-file", str(message_file))

    return make_fix


class TestForgeSync:
    """Opening and finding the extra repo's fix PR on a forge."""

    def test_fixes_share_one_forge_pr(
        self, fix_tool: Callable[[], Reply], forge: tuple[FakeForge, str]
    ) -> None:
        """The first fix opens a forge PR; the next joins it.

        gh and pde-gh-write cannot open a forge PR, so without this the
        fixes stop at a pushed branch. A second PR would split one
        PC's fixes across two reviews.
        """
        fake = forge[0]

        first = fix_tool()
        second = fix_tool()

        assert (first.body["sync"], first.body["pr"]) == ("pr_open", 1)
        assert (second.body["sync"], second.body["pr"]) == ("pushed", 1)
        assert [post["head"] for post in fake.posts] == [
            "fix/skill-fixes-desk-1"
        ]

    def test_merged_forge_pr_starts_new_batch(
        self,
        fix_tool: Callable[[], Reply],
        forge: tuple[FakeForge, str],
        extra: Extra,
        repos: Repos,
    ) -> None:
        """After its forge PR merges, the next fix opens a new one alone.

        The API marks a merged PR closed with ``merged`` true. Read as
        only closed, the next fix would ask for a second PR from the
        merged branch and carry the merged fix again.
        """
        fake = forge[0]
        fix_tool()
        worktree = repos.home / ".local/share/skill-fix/extra"
        merged = run_git(worktree, "rev-parse", "HEAD")
        run_git(extra.checkout, "cherry-pick", merged)
        run_git(extra.checkout, "push", "--quiet", "origin", "main")
        fake.pulls[0].update(state="closed", merged=True)
        fake.pulls[0]["head"]["sha"] = merged

        reply = fix_tool()

        new = "fix/skill-fixes-desk-2"
        assert (reply.body["branch"], reply.body["pr"]) == (new, 2)
        assert (
            run_git(extra.remote, "rev-list", "--count", f"main..{new}") == "1"
        )

    def test_unmergeable_forge_pr_skips_main_merge(
        self,
        fix_tool: Callable[[], Reply],
        forge: tuple[FakeForge, str],
        extra: Extra,
    ) -> None:
        """A forge PR marked not mergeable gets no merge from main.

        The forge also marks a PR not mergeable while it is still
        checking it. Read as a conflict, each sync after main moves
        would add a merge commit to the fix branch for no reason.
        """
        fake = forge[0]
        fix_tool()
        write(extra.checkout / "README.md", "Skills.\n")
        run_git(extra.checkout, "add", ".")
        run_git(extra.checkout, "commit", "--quiet", "-m", "Add README")
        run_git(extra.checkout, "push", "--quiet", "origin", "main")
        fake.pulls[0]["mergeable"] = False

        reply = fix_tool()

        branch = "fix/skill-fixes-desk-1"
        assert (reply.body["sync"], reply.body["pr"]) == ("pushed", 1)
        assert (
            run_git(extra.remote, "rev-list", "--merges", "--count", branch)
            == "0"
        )

    def test_conflicting_forge_pr_reports_conflict(
        self, fix_tool: Callable[[], Reply], extra: Extra
    ) -> None:
        """A fix branch that conflicts with main gives a conflict reply.

        The forge's own ``mergeable`` is not read, so sync must find
        the conflict itself. If it misses one, the fix PR cannot merge
        and nobody is told.
        """
        fix_tool()
        skill_md = extra.checkout / "skills/tool/SKILL.md"
        skill_md.write_text(skill_md.read_text() + "Use another tool.\n")
        run_git(extra.checkout, "commit", "--quiet", "-am", "Edit tool")
        run_git(extra.checkout, "push", "--quiet", "origin", "main")

        reply = fix_tool()

        assert (reply.body["sync"], reply.body["pr"]) == ("conflict", 1)

    def test_refused_token_error_omits_the_token(
        self, fix_tool: Callable[[], Reply], repos: Repos
    ) -> None:
        """A token the forge refuses gives an error that never holds it.

        The reply lands in agent logs and the update timer's journal,
        where a token would leak. An expired or wrong token is the
        likely forge failure.
        """
        wrong = "wrong-token-not-real"
        write(repos.home / ".config/pde/forge-token", f"{wrong}\n")

        reply = fix_tool()

        assert reply.code == 2
        assert "HTTP 401" in reply.body["detail"]
        assert wrong not in json.dumps(reply.body)
