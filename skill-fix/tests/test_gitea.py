"""Tests for the fix PR of an extra repo whose fix PRs are on Gitea.

A small HTTP server plays Gitea's pulls API, so no test needs a real
Gitea. It answers only the calls skill-fix makes, with the fields
Gitea 1.25 sends; the real server is tried by hand.
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
PULLS = "/api/v1/repos/admin/skills/pulls"
BRANCH = "fix/skill-fixes-desk-1"
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
class FakeGitea:
    """The fake Gitea's PRs and the requests it saw.

    Attributes:
        pulls: Its PRs in Gitea's fields, oldest first.
        seen: Each request as method, path, Authorization header and
            JSON body.
        refuse: Whether it answers every request with HTTP 401.
    """

    pulls: list[dict] = field(default_factory=list)
    seen: list[tuple[str, str, str, dict | None]] = field(default_factory=list)
    refuse: bool = False


@dataclass
class Extra:
    """The extra repo's paths.

    Attributes:
        checkout: The user's extra checkout, on main.
        remote: The bare repository that stands in for Gitea's git.
    """

    checkout: Path
    remote: Path


def make_handler(fake: FakeGitea) -> type[BaseHTTPRequestHandler]:
    """Return a handler that answers list and create PR calls from fake."""

    class Handler(BaseHTTPRequestHandler):
        """Gitea's pulls API, backed by fake."""

        def do_GET(self) -> None:
            """List every PR, newest first, as sort=recentupdate does."""
            if self.note_request(None):
                self.send_json(200, list(reversed(fake.pulls)))

        def do_POST(self) -> None:
            """Open a PR from the body's head branch."""
            size = int(self.headers["Content-Length"])
            body = json.loads(self.rfile.read(size))
            if self.note_request(body):
                pr = {
                    "number": len(fake.pulls) + 1,
                    "state": "open",
                    "merged": False,
                    "mergeable": True,
                    "head": {"ref": body["head"], "sha": "0" * 40},
                }
                fake.pulls.append(pr)
                self.send_json(201, pr)

        def note_request(self, body: dict | None) -> bool:
            """Save the request and return False once it was refused."""
            path = self.path.split("?")[0]
            auth = self.headers.get("Authorization", "")
            fake.seen.append((self.command, path, auth, body))
            if fake.refuse:
                self.send_json(401, {"message": "token is required"})
                return False
            return True

        def send_json(self, status: int, value: object) -> None:
            """Answer with status and value as JSON."""
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
def gitea() -> Iterator[tuple[FakeGitea, str]]:
    """A fake Gitea on a free local port, and its repo's web address."""
    fake = FakeGitea()
    server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(fake))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield fake, f"http://127.0.0.1:{server.server_port}/admin/skills"
    finally:
        server.shutdown()
        server.server_close()


@pytest.fixture
def extra(repos: Repos, tmp_path: Path, gitea: tuple[FakeGitea, str]) -> Extra:
    """An extra repo with the tool skill, set up to use the fake Gitea."""
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
    token_file = repos.home / ".config/pde/gitea-token"
    write(token_file, f"{TOKEN}\n")
    write_settings(
        repos.home,
        install_path=str(repos.checkout),
        skill_fix_label="desk",
        extra_skills_repo=str(checkout),
        extra_gitea_repo_url=gitea[1],
        extra_gitea_token_file=str(token_file),
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


class TestGiteaSync:
    """Opening and finding the extra repo's fix PR on Gitea."""

    def test_fixes_share_one_gitea_pr(
        self,
        fix_tool: Callable[[], Reply],
        gitea: tuple[FakeGitea, str],
        extra: Extra,
    ) -> None:
        """The first fix opens a Gitea PR with the token; the next joins it.

        gh and pde-gh-write cannot reach Gitea, so without this the
        fixes stop at a pushed branch. A second PR would split one
        PC's fixes across two reviews.
        """
        fake = gitea[0]

        first = fix_tool()
        second = fix_tool()

        assert (first.body["sync"], first.body["pr"]) == ("pr_open", 1)
        assert (second.body["sync"], second.body["pr"]) == ("pushed", 1)
        posts = [seen for seen in fake.seen if seen[0] == "POST"]
        assert len(posts) == 1
        _, path, auth, body = posts[0]
        assert (path, auth) == (PULLS, f"token {TOKEN}")
        assert body is not None
        assert (body["base"], body["head"], body["title"]) == (
            "main",
            BRANCH,
            "fix(skills): corrections from desk",
        )
        pushed = run_git(
            extra.remote, "rev-list", "--count", f"main..{BRANCH}"
        )
        assert pushed == "2"

    def test_merged_gitea_pr_starts_new_batch(
        self,
        fix_tool: Callable[[], Reply],
        gitea: tuple[FakeGitea, str],
        extra: Extra,
        repos: Repos,
    ) -> None:
        """After the user merges the Gitea PR, the next fix opens a new one.

        Gitea marks a merged PR closed with ``merged`` true. Read as
        only closed, the next fix would ask for a second PR from the
        merged branch and carry the merged fix again.
        """
        fake = gitea[0]
        fix_tool()
        worktree = repos.home / ".local/share/skill-fix/extra"
        merged = run_git(worktree, "rev-parse", "HEAD")
        run_git(extra.checkout, "cherry-pick", merged)
        run_git(extra.checkout, "push", "--quiet", "origin", "main")
        fake.pulls[0].update(state="closed", merged=True, mergeable=False)
        fake.pulls[0]["head"]["sha"] = merged

        reply = fix_tool()

        new = "fix/skill-fixes-desk-2"
        assert (
            reply.body["branch"],
            reply.body["sync"],
            reply.body["pr"],
        ) == (
            new,
            "pr_open",
            2,
        )
        titles = run_git(extra.remote, "log", "--format=%s", f"main..{new}")
        assert titles == "fix(tool): check step 2 before going on"

    def test_gitea_refusal_errors_without_the_token(
        self,
        fix_tool: Callable[[], Reply],
        gitea: tuple[FakeGitea, str],
    ) -> None:
        """A refused token is an error that names the status, not the token.

        The reply lands in agent logs and the update timer's journal,
        where a token would leak.
        """
        gitea[0].refuse = True

        reply = fix_tool()

        assert reply.code == 2
        assert "HTTP 401" in reply.body["detail"]
        assert TOKEN not in json.dumps(reply.body)
