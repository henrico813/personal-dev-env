#!/usr/bin/env python3
"""Find work that was started, then lost, and list it in one file.

A restarted Herdr server or a closed session can drop work that
someone waits on. One pull request write sat unseen for 16 hours that
way. The sweep looks for six kinds of lost work and writes them to
``$XDG_STATE_HOME/pde/sweep/latest.md``:

- lost-requests: pde-gh-write requests nobody will approve or run
- stuck-cards: cards with no session and no ``next:`` note
- review-cards-pr-done: review cards whose pull request merged or
  closed
- dirty-worktrees: worktrees with uncommitted or unpushed work
- branches-no-pr: pushed branches that never got a pull request
- hub-leftovers: hub reports nobody read and messages never sent

It only reads. It never fetches, pushes, sends, approves or moves
anything. A systemd user timer runs it every hour. Read ``main`` and
``Sweep.run_all`` first.

Usage::

    pde-sweep            write latest.md
    pde-sweep --stdout   print the report instead
"""

import argparse
import enum
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

KINDS = [
    "lost-requests",
    "stuck-cards",
    "review-cards-pr-done",
    "dirty-worktrees",
    "branches-no-pr",
    "hub-leftovers",
]
MAX_ITEMS = 10
# pde-gh-write stops offering a pending request this long after its
# last change.
REQUEST_EXPIRY = timedelta(hours=4)
# Approved and running requests finish in minutes, so one unchanged
# for an hour lost its runner.
RUNNER_LOST_AFTER = timedelta(hours=1)
LEFTOVER_AGE = timedelta(minutes=60)
# A hub writes events.jsonl while it runs, so a quiet day means it
# ended.
LIVE_RUN_WINDOW = timedelta(hours=24)
COMMAND_TIMEOUT_SECONDS = 30
DEFAULT_BRANCH_NAMES = ("main", "master")
BRANCH_FORMAT = (
    "--format=%(refname:short)%09%(upstream)%09%(upstream:remoteref)"
    "%09%(committerdate:unix)"
)
CARD_KEY_RE = re.compile(r"^[A-Z][A-Z0-9]*-KB-\d+$")
PR_URL_RE = re.compile(r"https://github\.com/[\w.-]+/[\w.-]+/pull/\d+")


def main(argv: list[str]) -> int:
    """Run the sweep, then write or print its report.

    Args:
        argv: The command line arguments after the program name.

    Returns:
        The exit code.
    """
    parser = argparse.ArgumentParser(
        prog="pde-sweep",
        description="List lost requests, cards, worktrees and branches.",
    )
    parser.add_argument(
        "--stdout",
        action="store_true",
        help="print the report instead of writing latest.md",
    )
    args = parser.parse_args(argv)
    home = Path.home()
    state = Path(os.environ.get("XDG_STATE_HOME") or home / ".local/state")
    sweep = Sweep(home, state, datetime.now(timezone.utc))
    sweep.run_all()
    report = sweep.render()
    if args.stdout:
        sys.stdout.write(report)
    else:
        write_atomically(state / "pde/sweep/latest.md", report)
    return 0


class WorktreeVerdict(enum.Enum):
    """What the sweep does with one worktree."""

    LIST = "list"
    SKIP = "skip"
    FINISHED = "finished"
    CLEAN = "clean"


@dataclass
class PushedBranch:
    """A local branch that tracks a remote branch of its own."""

    name: str
    remote_name: str
    committed: float


@dataclass
class WorktreeStatus:
    """What ``git status`` says about one worktree."""

    branch: str | None
    ahead: int | None
    changed: int
    untracked: int


class Sweep:
    """One pass over this machine that collects lost work by kind."""

    def __init__(self, home: Path, state: Path, now: datetime) -> None:
        """Start an empty sweep.

        Args:
            home: The home folder, which holds ``Projects``.
            state: The XDG state folder, such as ``~/.local/state``.
            now: The time that ages are measured from.
        """
        self.home = home
        self.state = state
        self.now = now
        # Each item is its time, for sorting newest first, and its line.
        self.items: dict[str, list[tuple[float, str]]] = {
            kind: [] for kind in KINDS
        }
        self.skipped_worktrees = 0
        self.finished_worktrees = 0
        self.errors: list[str] = []
        self.card_texts: dict[str, str | None] = {}
        self.pr_states: dict[tuple[Path, str], list[str] | None] = {}

    def run_all(self) -> None:
        """Look for each kind of lost work in turn."""
        self.find_lost_requests()
        self.find_stuck_cards()
        self.find_finished_review_cards()
        for repo in list_repos(self.home / "Projects"):
            self.check_repo(repo)
        self.find_hub_leftovers()

    def render(self) -> str:
        """Return the report, which always ends with a ``done`` line.

        Returns:
            Markdown with a count and the newest MAX_ITEMS items per
            kind.
        """
        local = self.now.astimezone()
        lines = [f"# Lost work sweep {local:%Y-%m-%d %H:%M}"]
        for kind in KINDS:
            items = sorted(self.items[kind], key=lambda i: i[0], reverse=True)
            lines += [f"## {kind}", f"count: {len(items)}"]
            if kind == "dirty-worktrees" and self.skipped_worktrees:
                lines.append(
                    f"skipped: {self.skipped_worktrees} on the default"
                    " branch with only untracked files"
                )
            if kind == "dirty-worktrees" and self.finished_worktrees:
                lines.append(
                    f"finished: {self.finished_worktrees} worktrees of"
                    " finished branches"
                )
            lines += [f"- {line}" for _, line in items[:MAX_ITEMS]]
        lines += ["## errors", f"count: {len(self.errors)}"]
        lines += [f"- {error}" for error in self.errors[:MAX_ITEMS]]
        lines.append("done")
        return "\n".join(lines) + "\n"

    def find_lost_requests(self) -> None:
        """List pde-gh-write requests nobody will approve or run."""
        for path in sorted((self.state / "pde/pr-write").glob("*.json")):
            try:
                record = json.loads(path.read_text())
            except (OSError, ValueError) as error:
                self.errors.append(f"{path.name}: {error}")
                continue
            if is_request_lost(record, self.now):
                self.items["lost-requests"].append(
                    (
                        read_updated(record).timestamp(),
                        describe_request(record),
                    )
                )

    def find_stuck_cards(self) -> None:
        """List cards with no live session and no ``next:`` note."""
        # With no live run, hub status picks the folder it used last.
        run_options = [
            ["--run", str(run)] for run in list_live_runs(self.state, self.now)
        ] or [[]]
        outputs = []
        for options in run_options:
            output = self.run_command(["hub", "status", "--json", *options])
            if output is not None:
                outputs.append(output)
        try:
            cards = merge_no_session_cards(outputs)
        except (ValueError, KeyError, TypeError) as error:
            self.errors.append(f"hub status --json: {error!r}")
            return
        for key, stage in cards:
            text = self.show_card(key)
            if text is not None and "next:" not in text:
                self.items["stuck-cards"].append((0, f"{key} {stage}"))

    def find_finished_review_cards(self) -> None:
        """List review cards whose pull request merged or closed."""
        listing = self.run_command(["card", "list", "--stage", "review"])
        for line in (listing or "").splitlines():
            fields = line.split()
            if not fields or not CARD_KEY_RE.match(fields[0]):
                continue
            key = fields[0]
            for url in find_card_pr_urls(self.show_card(key) or ""):
                state = self.run_command(
                    ["gh", "pr", "view", url, "--json", "state"]
                    + ["--jq", ".state"]
                )
                if state is not None and state.strip() in ("MERGED", "CLOSED"):
                    self.items["review-cards-pr-done"].append(
                        (0, f"{key} {url} {state.strip()}")
                    )

    def check_repo(self, repo: Path) -> None:
        """Check one repo's worktrees and its pushed branches.

        Args:
            repo: A main checkout, whose refs every worktree shares.
        """
        remote_refs = self.run_git(
            repo,
            "for-each-ref",
            "--format=%(refname) %(symref)",
            "refs/remotes",
        )
        if remote_refs is None:
            return
        defaults = find_default_refs(remote_refs)
        branch_refs = self.run_git(
            repo, "for-each-ref", BRANCH_FORMAT, "refs/heads"
        )
        default_refs = set(defaults.values())
        pushed = find_pushed_branches(branch_refs or "", default_refs)
        for branch in pushed:
            self.check_branch_has_pr(repo, branch)
        remote_names = {branch.name: branch.remote_name for branch in pushed}
        listing = self.run_git(repo, "worktree", "list", "--porcelain")
        for line in (listing or "").splitlines():
            if line.startswith("worktree "):
                path = Path(line.removeprefix("worktree "))
                self.check_worktree(repo, path, defaults, remote_names)

    def check_worktree(
        self,
        repo: Path,
        path: Path,
        defaults: dict[str, str],
        remote_names: dict[str, str],
    ) -> None:
        """List one worktree when it holds uncommitted or unpushed work.

        Args:
            repo: The main checkout, where gh finds the GitHub repo.
            path: The worktree folder.
            defaults: Each remote's default branch ref.
            remote_names: The remote branch name of each pushed branch.
        """
        # git keeps listing a worktree whose folder was deleted.
        if not path.is_dir():
            return
        text = self.run_git(path, "status", "--porcelain=v2", "--branch")
        if text is None:
            return
        status = read_worktree_status(text)
        base = defaults.get("origin") or next(iter(defaults.values()), None)
        if status.ahead is None and base is not None:
            count = self.run_git(path, "rev-list", "--count", f"{base}..HEAD")
            status.ahead = int(count) if count else None
        default_branches = (
            (base.split("/", 3)[3],) if base else DEFAULT_BRANCH_NAMES
        )
        finished = False
        # Only a worktree that would be listed for commits or new files
        # needs the gh call.
        has_work = status.ahead or status.untracked
        if status.branch not in (None, *default_branches) and has_work:
            remote_name = remote_names.get(status.branch, status.branch)
            states = self.list_pr_states(repo, remote_name)
            finished = bool(states) and "OPEN" not in states
        verdict = classify_worktree(status, default_branches, finished)
        if verdict is WorktreeVerdict.SKIP:
            self.skipped_worktrees += 1
        elif verdict is WorktreeVerdict.FINISHED:
            self.finished_worktrees += 1
        elif verdict is WorktreeVerdict.LIST:
            last_commit = self.run_git(path, "log", "-1", "--format=%ct")
            when = float(last_commit) if last_commit else 0
            self.items["dirty-worktrees"].append(
                (
                    when,
                    f"{format_date(when)} {self.show_path(path)}"
                    f" {status.branch or '(detached)'}"
                    f" changed={status.changed} untracked={status.untracked}"
                    f" ahead={status.ahead or 0}",
                )
            )

    def check_branch_has_pr(self, repo: Path, branch: PushedBranch) -> None:
        """List a pushed branch that never had a pull request.

        Args:
            repo: The main checkout, where gh finds the GitHub repo.
            branch: The branch and its remote branch.
        """
        if self.list_pr_states(repo, branch.remote_name) == []:
            self.items["branches-no-pr"].append(
                (
                    branch.committed,
                    f"{format_date(branch.committed)}"
                    f" {self.show_path(repo)} {branch.name}",
                )
            )

    def list_pr_states(self, repo: Path, remote_name: str) -> list[str] | None:
        """Return a branch's pull request states, once per sweep.

        Args:
            repo: The main checkout, where gh finds the GitHub repo.
            remote_name: The branch name on the remote.

        Returns:
            States such as OPEN, MERGED or CLOSED, empty when the branch
            never had a pull request, or None when gh failed.
        """
        key = (repo, remote_name)
        if key not in self.pr_states:
            output = self.run_command(
                ["gh", "pr", "list", "--head", remote_name, "--state", "all"]
                + ["--json", "state", "--jq", ".[].state"],
                cwd=repo,
            )
            self.pr_states[key] = None if output is None else output.split()
        return self.pr_states[key]

    def find_hub_leftovers(self) -> None:
        """List hub reports nobody read and messages hub never sent."""
        for run in sorted(self.state.glob("herdr-hub.*")):
            for path in [
                *run.glob("report-*.md"),
                *run.glob("sends/*.notsent"),
            ]:
                try:
                    mtime = path.stat().st_mtime
                except OSError:
                    # hub read or sent it after the listing.
                    continue
                modified = datetime.fromtimestamp(mtime, timezone.utc)
                age = self.now - modified
                if age > LEFTOVER_AGE:
                    hours = int(age / timedelta(hours=1))
                    line = f"{run.name} {path.relative_to(run)} {hours}h"
                    self.items["hub-leftovers"].append((mtime, line))

    def show_card(self, key: str) -> str | None:
        """Return the text of ``card show KEY``, run once per sweep.

        Args:
            key: A card key, such as ABC-KB-012.

        Returns:
            The card text, or None when card failed.
        """
        if key not in self.card_texts:
            self.card_texts[key] = self.run_command(["card", "show", key])
        return self.card_texts[key]

    def show_path(self, path: Path) -> str:
        """Return a path with the home folder written as ``~``.

        Args:
            path: An absolute path.

        Returns:
            The shorter path, or the path unchanged outside home.
        """
        if path.is_relative_to(self.home):
            return f"~/{path.relative_to(self.home)}"
        return str(path)

    def run_git(self, path: Path, *args: str) -> str | None:
        """Run a git command in one checkout.

        Args:
            path: The checkout or worktree folder.
            *args: The git arguments.

        Returns:
            Its output, or None when it failed.
        """
        return self.run_command(["git", "-C", str(path), *args])

    def run_command(
        self, argv: list[str], cwd: Path | None = None
    ) -> str | None:
        """Run a command, noting a failure without stopping the sweep.

        Each command gets COMMAND_TIMEOUT_SECONDS, so one hung tool
        cannot stall the hourly run.

        Args:
            argv: The command and its arguments.
            cwd: The folder to run it in, or None for the current one.

        Returns:
            Its output, or None when it failed or timed out.
        """
        command = " ".join(argv)
        try:
            done = subprocess.run(
                argv,
                cwd=cwd,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=COMMAND_TIMEOUT_SECONDS,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            self.errors.append(f"{command}: {error}")
            return None
        if done.returncode != 0:
            reason = " ".join(done.stderr.strip().splitlines()[-1:])
            self.errors.append(f"{command}: exit {done.returncode} {reason}")
            return None
        return done.stdout


def is_request_lost(record: dict[str, Any], now: datetime) -> bool:
    """Tell whether a pde-gh-write request waits on nobody.

    A pending request is lost once the user dismissed it or it expired.
    An approved or running request is lost when it has not changed for
    an hour. A record with no readable time counts as old.

    Args:
        record: One request file, as pdev/internal/approval writes it.
        now: The current time.

    Returns:
        True when the request is lost.
    """
    age = now - read_updated(record)
    state = record.get("state")
    if state == "pending":
        return bool(record.get("dismissed")) or age > REQUEST_EXPIRY
    if state in ("approved", "running"):
        return age > RUNNER_LOST_AFTER
    return False


def read_updated(record: dict[str, Any]) -> datetime:
    """Return when a request last changed.

    Go writes nanoseconds and trims trailing zeros, as in
    ``2026-11-11T23:28:12.6206099-08:00``. Before Python 3.11,
    ``datetime.fromisoformat`` takes exactly six digits, so the
    fraction is padded or cut.

    Args:
        record: One request file.

    Returns:
        The time, or the oldest possible time when it is missing or bad.
    """
    text = str(record.get("updated") or "").replace("Z", "+00:00")
    text = re.sub(r"\.(\d+)", lambda m: "." + f"{m[1]:0<6}"[:6], text)
    try:
        updated = datetime.fromisoformat(text)
    except ValueError:
        return datetime.min.replace(tzinfo=timezone.utc)
    if updated.tzinfo is None:
        return updated.replace(tzinfo=timezone.utc)
    return updated


def describe_request(record: dict[str, Any]) -> str:
    """Return one line naming a request, its repo and its command.

    Args:
        record: One request file.

    Returns:
        A line such as ``ab5246d8 pending 2026-11-11T23:28 o/r pr
        create | title``.
    """
    dismissed = " dismissed" if record.get("dismissed") else ""
    updated = str(record.get("updated") or "-")[:16]
    command = " ".join(record.get("args", [])[:2])
    title = (
        record.get("pr_title")
        or record.get("title")
        or record.get("session_title")
        or "-"
    )
    return (
        f"{str(record.get('id', ''))[:8]} {record.get('state')}{dismissed}"
        f" {updated} {record.get('repo') or '-'} {command} | {title}"
    )


def list_live_runs(state: Path, now: datetime) -> list[Path]:
    """Return the run folders of hubs that may still be running.

    Auto runs, the folders with an ``approved.md``, are skipped, as hub
    status skips them when two hubs run.

    Args:
        state: The XDG state folder that holds ``herdr-hub.*`` folders.
        now: The current time.

    Returns:
        The folders whose ``events.jsonl`` changed within a day.
    """
    live = []
    for run in sorted(state.glob("herdr-hub.*")):
        events = run / "events.jsonl"
        if (run / "approved.md").exists() or not events.is_file():
            continue
        modified = datetime.fromtimestamp(events.stat().st_mtime, timezone.utc)
        if now - modified < LIVE_RUN_WINDOW:
            live.append(run)
    return live


def merge_no_session_cards(outputs: list[str]) -> list[tuple[str, str]]:
    """Join the cards with no session from several hub status runs.

    Two hubs see the same board, so a card can appear in both.

    Args:
        outputs: The JSON text of each hub status run.

    Returns:
        Each card key and stage once, sorted by key.

    Raises:
        ValueError: When an output is not JSON.
        KeyError: When an entry has no card or stage.
    """
    cards: dict[str, str] = {}
    for output in outputs:
        for entry in json.loads(output).get("no_session", []):
            cards.setdefault(entry["card"], entry["stage"])
    return sorted(cards.items())


def find_card_pr_urls(card_text: str) -> list[str]:
    """Return the pull request links in a card's front matter.

    Only the ``prs:`` field counts. A link in the card's text may name
    another card's pull request.

    Args:
        card_text: The output of ``card show``.

    Returns:
        Each link once, sorted.
    """
    parts = card_text.split("---\n", 2)
    front_matter = parts[1] if card_text.startswith("---\n") else ""
    return sorted(set(PR_URL_RE.findall(front_matter)))


def list_repos(projects: Path) -> list[Path]:
    """Return each main checkout directly under ``projects``.

    A worktree there has a ``.git`` file instead of a folder, and is
    checked through its main checkout.

    Args:
        projects: The folder that holds the checkouts.

    Returns:
        The checkouts, sorted.
    """
    if not projects.is_dir():
        return []
    return sorted(p for p in projects.iterdir() if (p / ".git").is_dir())


def find_default_refs(remote_refs: str) -> dict[str, str]:
    """Map each remote to its default branch ref.

    The remote's HEAD names it when a clone set one. Otherwise the
    remote's ``main`` or ``master`` stands in.

    Args:
        remote_refs: ``git for-each-ref --format='%(refname)
            %(symref)'`` output.

    Returns:
        A map such as ``{"github": "refs/remotes/github/main"}``.
    """
    found: dict[str, str] = {}
    heads: dict[str, str] = {}
    for line in remote_refs.splitlines():
        ref, _, target = line.partition(" ")
        if not ref.startswith("refs/remotes/"):
            continue
        remote, _, name = ref.removeprefix("refs/remotes/").partition("/")
        if name == "HEAD" and target:
            heads[remote] = target
        elif name in DEFAULT_BRANCH_NAMES:
            found.setdefault(remote, ref)
    return {**found, **heads}


def read_worktree_status(text: str) -> WorktreeStatus:
    """Read ``git status --porcelain=v2 --branch`` output.

    Args:
        text: The status output.

    Returns:
        The branch, commits ahead of the upstream, and file counts.
        ``ahead`` is None when the branch has no upstream, or the
        upstream is gone.
    """
    status = WorktreeStatus(branch=None, ahead=None, changed=0, untracked=0)
    for line in text.splitlines():
        if line.startswith("# branch.head "):
            head = line.removeprefix("# branch.head ")
            status.branch = None if head == "(detached)" else head
        elif line.startswith("# branch.ab +"):
            status.ahead = int(line.split()[2].removeprefix("+"))
        elif line.startswith("? "):
            status.untracked += 1
        elif line[:2] in ("1 ", "2 ", "u "):
            status.changed += 1
    return status


def classify_worktree(
    status: WorktreeStatus,
    default_branches: tuple[str, ...],
    branch_finished: bool,
) -> WorktreeVerdict:
    """Decide whether a worktree holds work that could be lost.

    Changed tracked files are always listed. A branch whose pull
    request merged or closed is finished; its commits and new files
    are only counted, since a squash merge leaves commits that look
    unpushed. Untracked files alone are skipped on the default branch,
    where the main checkout collects scratch files. On another branch
    they may be new work nobody added yet.

    Args:
        status: The worktree's status.
        default_branches: Names that count as the default branch.
        branch_finished: Whether the branch's pull request merged or
            closed, with none still open.

    Returns:
        LIST, SKIP, FINISHED or CLEAN.
    """
    if status.changed:
        return WorktreeVerdict.LIST
    if branch_finished and (status.ahead or status.untracked):
        return WorktreeVerdict.FINISHED
    if (status.ahead or 0) > 0:
        return WorktreeVerdict.LIST
    if status.untracked:
        if status.branch in default_branches:
            return WorktreeVerdict.SKIP
        return WorktreeVerdict.LIST
    return WorktreeVerdict.CLEAN


def find_pushed_branches(
    branch_refs: str, default_refs: set[str]
) -> list[PushedBranch]:
    """Return each local branch that tracks its own remote branch.

    A branch that tracks a remote default branch, or another local
    branch, was never pushed under its own name, so it is left out.

    Args:
        branch_refs: ``git for-each-ref`` output in BRANCH_FORMAT.
        default_refs: The remote default branch refs, such as
            ``refs/remotes/origin/main``.

    Returns:
        The pushed branches, in the order given.
    """
    pushed = []
    for line in branch_refs.splitlines():
        name, upstream, remote_ref, committed = line.split("\t")
        if not upstream.startswith("refs/remotes/"):
            continue
        if upstream in default_refs:
            continue
        remote_name = remote_ref.removeprefix("refs/heads/") or name
        pushed.append(PushedBranch(name, remote_name, float(committed)))
    return pushed


def format_date(timestamp: float) -> str:
    """Return a Unix time as a local date, or ``-`` when it is unknown.

    Args:
        timestamp: Seconds since 1970, or 0 when unknown.

    Returns:
        A date such as ``2026-11-12``.
    """
    if not timestamp:
        return "-"
    return f"{datetime.fromtimestamp(timestamp):%Y-%m-%d}"


def write_atomically(path: Path, text: str) -> None:
    """Replace a file so a reader never sees half of it.

    Args:
        path: The file to replace.
        text: Its new text.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
    try:
        with os.fdopen(fd, "w") as file:
            file.write(text)
        os.replace(temp, path)
    except BaseException:
        os.unlink(temp)
        raise


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
