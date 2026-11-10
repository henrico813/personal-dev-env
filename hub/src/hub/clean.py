"""Close the Herdr tabs whose work is finished, for ``hub clean``.

Read ``run_clean`` first, then ``judge_pane``, which decides for one
pane.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

from hub.herdr import FREE_STATES, call_herdr, list_agents
from hub.status import CARD_KEY, CARD_LINE, call_card

FINISHED_STAGES = ("accepted", "dropped")


def run_clean(args: argparse.Namespace) -> int:
    """Close each tab whose panes all hold finished work, and say why.

    A tab closes only when every pane in it may close. The tab holding
    the hub's own pane and the tab the user looks at always stay.

    Args:
        args: The parsed ``clean`` command line.

    Returns:
        0 after closing, or 2 with ``--machine``.
    """
    if args.machine is not None:
        print(
            "hub: clean checks worktrees with git on this machine,"
            " so it takes no --machine",
            file=sys.stderr,
        )
        return 2
    stages = read_finished_cards()
    names = {agent["pane_id"]: agent["name"] for agent in list_agents(None)}
    panes = json.loads(call_herdr(None, "pane", "list"))["result"]["panes"]
    tabs: dict[str, list[dict]] = {}
    for pane in panes:
        tabs.setdefault(pane["tab_id"], []).append(pane)
    own_pane = os.environ.get("HERDR_PANE_ID")
    closed = 0
    for tab_id, tab_panes in tabs.items():
        if any(
            pane["pane_id"] == own_pane or pane.get("focused")
            for pane in tab_panes
        ):
            continue
        verdicts = [judge_pane(pane, names, stages) for pane in tab_panes]
        if all(closes for closes, _ in verdicts):
            call_herdr(None, "tab", "close", tab_id)
            print(f"closed {tab_id} " + "; ".join(why for _, why in verdicts))
            closed += 1
        else:
            for closes, why in verdicts:
                if not closes and why:
                    print(f"kept {why}")
    if not closed:
        print("nothing to close")
    return 0


def judge_pane(
    pane: dict, names: dict[str, str], stages: dict[str, str]
) -> tuple[bool, str]:
    """Decide whether one pane's tab may close.

    Args:
        pane: The pane as ``herdr pane list`` prints it.
        names: Agent names by pane ID.
        stages: The stage of each accepted or dropped card, by key.

    Returns:
        Whether the pane may close, and why. The reason is empty when
        the pane stays for a reason the user does not need to see.
    """
    if pane.get("agent") is None:
        at_prompt = is_shell_at_prompt(pane["pane_id"])
        return at_prompt, "no agent" if at_prompt else ""
    name = names.get(pane["pane_id"], "")
    card = CARD_KEY.match(name)
    key = card.group().upper() if card else ""
    if key not in stages:
        return False, ""
    if pane["agent_status"] not in FREE_STATES:
        return False, f"{name}: {pane['agent_status']}"
    folders = {
        folder
        for folder in (pane.get("cwd"), pane.get("foreground_cwd"))
        if folder is not None
    }
    try:
        for worktree in find_card_worktrees(folders, key.lower()):
            problem = find_unsaved_work(worktree)
            if problem:
                return False, f"{name}: {problem}"
    except GitError as error:
        return False, f"{name}: git failed, {error}"
    return True, f"{name}: {key} {stages[key]}"


def read_finished_cards() -> dict[str, str]:
    """Read the keys of accepted and dropped cards.

    Returns:
        Each card's stage by its key, such as ``DEMO-KB-007``.

    Raises:
        CardError: The card command failed or is not installed.
    """
    stages = {}
    for stage in FINISHED_STAGES:
        for line in call_card("list", "--stage", stage).splitlines():
            if CARD_LINE.match(line):
                stages[line.split()[0]] = stage
    return stages


def is_shell_at_prompt(pane_id: str) -> bool:
    """Tell whether a pane's shell runs nothing, so closing loses nothing.

    Herdr shows only the foreground process, so the shell's background
    and stopped jobs come from ``/proc``.

    Args:
        pane_id: The pane's ID.

    Returns:
        True when the shell itself is the pane's foreground process and
        owns no job, or False when ``/proc`` cannot tell.
    """
    info = json.loads(
        call_herdr(None, "pane", "process-info", "--pane", pane_id)
    )["result"]["process_info"]
    shell_pid = info["shell_pid"]
    if info["foreground_process_group_id"] != shell_pid:
        return False
    try:
        return not has_shell_jobs(shell_pid)
    except OSError:
        return False


def has_shell_jobs(shell_pid: int) -> bool:
    """Tell whether a shell owns a job, such as one stopped with Ctrl-Z.

    A shell gives each job its own process group, while helpers it
    starts without job control stay in the shell's group.

    Args:
        shell_pid: The shell's process ID.

    Returns:
        True when a child of the shell is in another process group.

    Raises:
        OSError: ``/proc`` has no readable entry for the shell.
    """
    shell_group = read_process_group(shell_pid)
    for task in Path(f"/proc/{shell_pid}/task").iterdir():
        for child in (task / "children").read_text().split():
            try:
                group = read_process_group(int(child))
            except OSError:
                # The child ended after the shell listed it.
                continue
            if group != shell_group:
                return True
    return False


def read_process_group(pid: int) -> int:
    """Read a process's group ID from ``/proc``.

    Args:
        pid: The process ID.

    Returns:
        The process group ID.

    Raises:
        OSError: The process does not exist.
    """
    stat = Path(f"/proc/{pid}/stat").read_text()
    # The name before ")" may hold spaces, and the group is third after it.
    return int(stat.rpartition(")")[2].split()[2])


def find_card_worktrees(folders: set[str], key: str) -> list[Path]:
    """Find the linked worktrees a card's agent may have worked in.

    These are the worktrees holding the agent's folders and those whose
    branch names the card. The main checkout is never one, since it
    holds the user's own files.

    Args:
        folders: The agent's working folders, as Herdr prints them.
        key: The card's key in lowercase, such as ``demo-kb-007``.

    Returns:
        Each worktree once.

    Raises:
        GitError: git failed.
    """
    found: list[Path] = []
    for text in folders:
        # Herdr adds this mark to a removed folder, such as a merged worktree.
        folder = Path(text.removesuffix(" (deleted)"))
        if not folder.exists() or not is_in_repository(folder):
            continue
        for path, branch in list_linked_worktrees(folder):
            in_use = folder.resolve().is_relative_to(path.resolve())
            if (in_use or key in branch) and path not in found:
                found.append(path)
    return found


def is_in_repository(folder: Path) -> bool:
    """Tell whether a folder sits in a git repository.

    Args:
        folder: An existing folder.

    Returns:
        True when the folder or a parent holds ``.git``.
    """
    return any((path / ".git").exists() for path in (folder, *folder.parents))


def list_linked_worktrees(folder: Path) -> list[tuple[Path, str]]:
    """List the worktrees of a folder's repository, except the main one.

    Worktrees whose folder is gone are left out, since they hold no
    files.

    Args:
        folder: A folder in the repository.

    Returns:
        Each worktree's folder and branch, or an empty branch when
        detached.

    Raises:
        GitError: git failed.
    """
    blocks = run_git(folder, "worktree", "list", "--porcelain").split("\n\n")
    worktrees = []
    # git lists the main worktree first.
    for block in blocks[1:]:
        fields = {}
        for line in block.splitlines():
            name, _, value = line.partition(" ")
            fields[name] = value
        if "worktree" in fields and Path(fields["worktree"]).exists():
            worktrees.append(
                (Path(fields["worktree"]), fields.get("branch", ""))
            )
    return worktrees


def find_unsaved_work(worktree: Path) -> str:
    """Describe work in a worktree that only this folder holds.

    No fetch runs first, so a remote branch git has not seen yet makes
    pushed commits look unpushed and the tab stays.

    Args:
        worktree: The worktree's folder.

    Returns:
        What is unsaved, or an empty string when nothing is.

    Raises:
        GitError: git failed.
    """
    if run_git(worktree, "status", "--porcelain"):
        return f"uncommitted files in {worktree}"
    if run_git(worktree, "rev-list", "HEAD", "--not", "--remotes"):
        return f"unpushed commits in {worktree}"
    return ""


class GitError(Exception):
    """A git command that failed."""


def run_git(folder: Path, *args: str) -> str:
    """Run git in a folder and return what it prints.

    Args:
        folder: The folder git runs in.
        *args: The git arguments, such as ``status --porcelain``.

    Returns:
        git's standard output.

    Raises:
        GitError: git exited nonzero or is not installed.
    """
    try:
        result = subprocess.run(
            ["git", "-C", str(folder), *args],
            capture_output=True,
            text=True,
            check=False,
        )
    except FileNotFoundError:
        raise GitError("git is not installed") from None
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "git failed")
    return result.stdout
