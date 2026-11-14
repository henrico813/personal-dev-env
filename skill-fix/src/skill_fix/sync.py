"""Push a repo's fix branch and keep one fix PR open for it.

Read ``sync_repo`` first. Pushes are plain ``git push``, never forced.
Each batch gets its own branch, so a merged batch's branch is never
pushed to again. The only PR write is one ``pde-gh-write pr create``
per batch, and the user approves it there.

Fetches and pushes go to the HTTPS form of a GitHub SSH remote, as in
``pde-installer update``, because its timer has no SSH agent.
"""

from __future__ import annotations

import json
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

from skill_fix import repos
from skill_fix.repos import Repo, RepoName, Settings, ToolError

SyncState = Literal[
    "pushed",
    "pr_waiting",
    "pr_open",
    "offline",
    "conflict",
    "no_label",
    "scan",
    "skipped",
]
REQUEST_ID = re.compile(r"(?:pde-pr-approve|approval of) ([0-9a-f]{64})")
GITHUB_SSH = ("git@github.com:", "ssh://git@github.com/")
# Git's words when the remote answered but refused, which no later
# sync fixes by itself, unlike a network failure.
AUTH_FAILURE = re.compile(
    r"could not read (?:Username|Password)|Authentication failed"
    r"|Permission denied|returned error: 40[13]|Repository not found",
    re.IGNORECASE,
)
PR_TITLE = "fix(skills): corrections from {label}"
# The body never changes, so a rerun of the same create command picks up
# the waiting request instead of filing a new one.
PR_BODY = """\
## Overview

This PR holds one commit per correction to a skill, all made on one
PC. Read the commits one at a time; each message says what the agent
missed and what the fix changes.

## Changes

See the commit list. New fixes join this PR as new commits until it
merges.

## Testing

- [x] skill-fix fix rules: each commit passed them before it was made
- [x] `ci/scan commits` on every commit before each push
- [ ] Tried in a session: not run; a fix takes effect after merge and
  update
"""


@dataclass(frozen=True)
class SyncResult:
    """What one sync did to one repo.

    Attributes:
        repo: The repo synced.
        state: ``skipped`` when the repo has no fix to sync.
        branch: The fix branch, or "" when there is no fix worktree.
        pr: The fix PR's number once it exists.
        request: The pde-gh-write request ID while it waits.
    """

    repo: RepoName
    state: SyncState
    branch: str
    pr: int | None = None
    request: str | None = None


def sync_repo(repo: Repo, settings: Settings) -> SyncResult:
    """Push the fix branch and open its fix PR when none is open.

    Offline, with no label, or after a refused scan, the commits wait
    on their branch for a later sync. After the last fix PR merged,
    fixes not in it move to a new batch branch.

    Args:
        repo: The repo to sync.
        settings: This PC's settings.

    Returns:
        The sync's result.

    Raises:
        ToolError: Git, gh or pde-gh-write failed, or the remote
            refused the fetch, such as for a missing credential.
    """
    if not repos.has_worktree(repo):
        return SyncResult(repo.name, "skipped", "")
    branch = repos.read_branch(repo)
    url = find_remote_url(repo)
    if not fetch_remote(repo, url):
        return SyncResult(repo.name, "offline", branch)
    main = f"{repo.remote}/main"
    if settings.label is None:
        waiting = repos.count_commits(repo, main) > 0
        return SyncResult(
            repo.name, "no_label" if waiting else "skipped", branch
        )
    if branch.startswith(f"{repos.BATCH_PREFIX}unset-"):
        # Nothing was pushed under the unset name, so no remote branch moves.
        new = repos.name_next_batch(repo, settings.label)
        repos.run_git(repo.worktree, "branch", "-m", new)
        branch = new
    pr = find_pr(repo, branch)
    if pr is not None and pr["state"] == "MERGED":
        batch = start_new_batch(repo, settings.label, pr["headRefOid"])
        if batch is None:
            return SyncResult(repo.name, "conflict", branch, pr["number"])
        branch, pr = batch, None
    if pr is not None and pr["state"] != "OPEN":
        pr = None
    number = pr["number"] if pr else None
    pushed = False
    if repos.count_commits(repo, find_upstream(repo, branch)) > 0:
        if repo.name == "personal" and not run_scan(
            repo, "commits", main, "HEAD"
        ):
            return SyncResult(repo.name, "scan", branch, number)
        push_branch(repo, url, branch)
        pushed = True
    if pr is not None and pr.get("mergeable") == "CONFLICTING":
        if not repos.try_git(
            repo.worktree, "merge", "--quiet", "--no-edit", main
        ):
            repos.try_git(repo.worktree, "merge", "--abort")
            return SyncResult(repo.name, "conflict", branch, number)
        push_branch(repo, url, branch)
        pushed = True
    if pr is not None:
        return SyncResult(
            repo.name, "pushed" if pushed else "pr_open", branch, number
        )
    if repos.count_commits(repo, main) == 0:
        return SyncResult(repo.name, "skipped", branch)
    return open_fix_pr(repo, branch, settings.label)


def find_remote_url(repo: Repo) -> str:
    """Return the remote's URL, with a GitHub SSH address made HTTPS.

    This is the rule of ``toHTTPSURL`` in pde-installer's update.go.
    Any other address is returned as it is.
    """
    url = repos.run_git(
        repo.checkout, "remote", "get-url", repo.remote
    ).strip()
    for prefix in GITHUB_SSH:
        if url.startswith(prefix):
            return "https://github.com/" + url[len(prefix) :]
    return url


def fetch_remote(repo: Repo, url: str) -> bool:
    """Fetch every branch of url into the remote's tracking refs.

    Returns:
        True on success, False when the fetch failed, such as offline.

    Raises:
        ToolError: The remote refused the fetch, such as for a
            missing credential.
    """
    refs = f"+refs/heads/*:refs/remotes/{repo.remote}/*"
    result = repos.call_git(repo.worktree, ["fetch", "--quiet", url, refs])
    if result.returncode == 0:
        return True
    if AUTH_FAILURE.search(result.stderr):
        raise ToolError(describe_git_failure("fetch", url, result))
    return False


def push_branch(repo: Repo, url: str, branch: str) -> None:
    """Push branch to url and move its tracking ref to match.

    Git moves no tracking ref after a push to a URL, and the next
    sync counts unpushed commits from that ref.

    Raises:
        ToolError: The push failed.
    """
    result = repos.call_git(
        repo.worktree,
        ["push", "--quiet", url, f"{branch}:refs/heads/{branch}"],
    )
    if result.returncode != 0:
        raise ToolError(describe_git_failure("push", url, result))
    repos.run_git(
        repo.worktree,
        "update-ref",
        f"refs/remotes/{repo.remote}/{branch}",
        "HEAD",
    )


def describe_git_failure(
    command: str, url: str, result: subprocess.CompletedProcess[str]
) -> str:
    """Return git's last error line, with a fix for a GitHub login failure.

    Over HTTPS, git logs in to GitHub through a credential helper,
    which ``gh auth setup-git`` sets up.
    """
    lines = result.stderr.strip().splitlines() or [f"exit {result.returncode}"]
    detail = f"git {command} failed: {lines[-1]}"
    if AUTH_FAILURE.search(result.stderr) and url.startswith(
        "https://github.com/"
    ):
        detail += "; run gh auth setup-git"
    return detail


def find_pr(repo: Repo, branch: str) -> dict | None:
    """Return the newest PR from branch, open, closed or merged, or None.

    Raises:
        ToolError: gh failed.
    """
    output = run_tool(
        repo,
        [
            "gh",
            "pr",
            "list",
            "--head",
            branch,
            "--state",
            "all",
            "--limit",
            "1",
            "--json",
            "number,state,headRefOid,mergeable",
        ],
    )
    if output.returncode != 0:
        raise ToolError(f"gh pr list failed for {branch}")
    found = parse_json_list(output.stdout)
    return found[0] if found else None


def start_new_batch(repo: Repo, label: str, merged_head: str) -> str | None:
    """Move the fixes made after a merged PR's head to a new batch branch.

    The new branch starts from the remote's ``main``, and gets only
    the commits after ``merged_head``. No PR has shown those yet, so
    moving them rewrites no reviewed commit.

    Args:
        repo: The repo to work in.
        label: This PC's label.
        merged_head: The head of the merged fix PR.

    Returns:
        The new branch, or None when the move conflicts, which leaves
        the old branch checked out as it was.
    """
    old = repos.read_branch(repo)
    new = repos.name_next_batch(repo, label)
    repos.run_git(repo.worktree, "switch", "--quiet", "-c", new)
    main = f"{repo.remote}/main"
    if repos.try_git(
        repo.worktree, "rebase", "--quiet", "--onto", main, merged_head
    ):
        return new
    repos.try_git(repo.worktree, "rebase", "--abort")
    repos.run_git(repo.worktree, "switch", "--quiet", old)
    repos.run_git(repo.worktree, "branch", "-D", new)
    return None


def open_fix_pr(repo: Repo, branch: str, label: str) -> SyncResult:
    """Ask pde-gh-write to open the fix PR; exit 3 means it waits.

    A rerun of the same command, with the same title, body and folder,
    reads the waiting request instead of filing a second one.

    Raises:
        ToolError: pde-gh-write is missing, refused the command, or
            the request was declined.
    """
    title = PR_TITLE.format(label=label)
    body = repos.find_state_dir() / f"{repo.name}-pr-body.md"
    body.write_text(PR_BODY, encoding="utf-8")
    if repo.name == "personal":
        with tempfile.TemporaryDirectory() as folder:
            text = Path(folder) / "text"
            text.write_text(f"{title}\n\n{PR_BODY}", encoding="utf-8")
            if not run_scan(repo, "text", str(text)):
                return SyncResult(repo.name, "scan", branch)
    result = run_tool(
        repo,
        [
            "pde-gh-write",
            "pr",
            "create",
            "--base",
            "main",
            "--head",
            branch,
            "--title",
            title,
            "--body-file",
            str(body),
        ],
    )
    if result.returncode == 3:
        requests = REQUEST_ID.findall(result.stderr)
        request = requests[-1] if requests else None
        return SyncResult(repo.name, "pr_waiting", branch, request=request)
    if result.returncode != 0:
        raise ToolError(f"pde-gh-write pr create exited {result.returncode}")
    pr = find_pr(repo, branch)
    return SyncResult(
        repo.name, "pr_open", branch, pr["number"] if pr else None
    )


def run_scan(repo: Repo, *args: str) -> bool:
    """Run the worktree's ``ci/scan`` and return whether it found nothing.

    A scan that cannot run, such as with a missing work-term list,
    counts as a refusal, so nothing is pushed unchecked.
    """
    result = run_tool(repo, [str(repo.worktree / "ci/scan"), *args])
    return result.returncode == 0


def read_status(repo: Repo) -> dict:
    """Return a repo's fix branch, unpushed count, PR and held count.

    It reads git and GitHub and changes nothing. A failed PR lookup,
    such as offline, gives ``pr_state`` ``unknown``.
    """
    held = len(repos.list_held_fixes(repo))
    if not repos.has_worktree(repo):
        return {
            "repo": repo.name,
            "branch": None,
            "unpushed": 0,
            "pr": None,
            "pr_state": None,
            "conflict": False,
            "held": held,
        }
    branch = repos.read_branch(repo)
    unpushed = repos.count_commits(repo, find_upstream(repo, branch))
    try:
        pr = find_pr(repo, branch)
    except ToolError:
        pr = {"number": None, "state": "unknown"}
    return {
        "repo": repo.name,
        "branch": branch,
        "unpushed": unpushed,
        "pr": pr["number"] if pr else None,
        "pr_state": pr["state"].lower() if pr else None,
        "conflict": bool(pr and pr.get("mergeable") == "CONFLICTING"),
        "held": held,
    }


def find_upstream(repo: Repo, branch: str) -> str:
    """Return the branch on the remote, or the remote's main before a push."""
    pushed = f"{repo.remote}/{branch}"
    if repos.try_git(
        repo.worktree, "rev-parse", "--verify", "--quiet", pushed
    ):
        return pushed
    return f"{repo.remote}/main"


def run_tool(
    repo: Repo, command: list[str]
) -> subprocess.CompletedProcess[str]:
    """Run a program in the fix worktree and capture its output.

    Raises:
        ToolError: The program is not installed.
    """
    try:
        return subprocess.run(
            command, cwd=repo.worktree, capture_output=True, text=True
        )
    except FileNotFoundError as error:
        raise ToolError(f"{Path(command[0]).name} is not installed") from error


def parse_json_list(text: str) -> list:
    """Return gh's JSON output as a list.

    Raises:
        ToolError: The output is not a JSON list.
    """
    try:
        value = json.loads(text)
    except ValueError as error:
        raise ToolError("gh printed something other than JSON") from error
    if not isinstance(value, list):
        raise ToolError("gh printed something other than a list")
    return value
