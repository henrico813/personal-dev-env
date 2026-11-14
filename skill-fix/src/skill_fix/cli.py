"""Turn a correction into a commit in the skill's own repo.

The main flow is ``start``, then the agent edits the skill in the fix
worktree, then ``commit``, which checks the fix rules, commits and
syncs. Each command prints one JSON object, except ``held show``,
which prints a diff for the user. Exit codes are 0 for ok, committed
and held, 1 for refused, and 2 for error.

Example:
    skill-fix start herdr-hub
    skill-fix commit herdr-hub --message-file /tmp/fix.txt
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

from skill_fix import __version__, repos
from skill_fix.fix_rules import Finding, Hold, Refused, check_fix
from skill_fix.repos import RefusalError, Repo, Settings, ToolError
from skill_fix.sync import read_status, sync_repo

EXIT_CODES = {"ok": 0, "committed": 0, "held": 0, "refused": 1, "error": 2}


def main(argv: list[str] | None = None) -> int:
    """Run one command, print its reply and return its exit code."""
    args = build_parser().parse_args(argv)
    reply: dict | None
    try:
        if args.command == "start":
            reply = start_fix(args.skill, args.reset)
        elif args.command == "commit":
            reply = commit_fix(args.skill, Path(args.message_file))
        elif args.command == "sync":
            reply = sync_repos(args.repo)
        elif args.command == "status":
            reply = report_status()
        else:
            reply = manage_held(args.action, args.number)
    except RefusalError as refusal:
        reply = {
            "status": "refused",
            "rule": refusal.rule,
            "detail": refusal.detail,
        }
    except ToolError as error:
        reply = {"status": "error", "detail": str(error)}
    if reply is None:
        return 0
    print(json.dumps(reply))
    return EXIT_CODES[reply["status"]]


def start_fix(skill: str, reset: bool) -> dict:
    """Get the fix worktree ready and name the file to edit.

    The reply's ``edits`` is true when the skill already has edits
    that were never committed, which the next commit would include.

    Raises:
        RefusalError: ``other-files`` when the worktree has edits to
            another skill, or a refusal from ``find_repo``.
    """
    settings = repos.load_settings()
    repo = repos.find_repo(skill, settings)
    branch = repos.ensure_worktree(repo, settings.label)
    if reset:
        repos.discard_edits(repo, skill)
    folder = repo.find_skill_dir(skill)
    changed = repos.list_changed_paths(repo)
    others = [path for path in changed if not path.startswith(f"{folder}/")]
    if others:
        raise RefusalError("other-files", ", ".join(others))
    return {
        "status": "ok",
        "repo": repo.name,
        "worktree": str(repo.worktree),
        "file": str(repo.worktree / folder / "SKILL.md"),
        "branch": branch,
        "edits": bool(changed),
    }


def commit_fix(skill: str, message_file: Path) -> dict:
    """Check the fix rules, then commit or hold the fix and sync.

    Raises:
        ToolError: ``start`` has not made the fix worktree.
    """
    settings = repos.load_settings()
    repo = repos.find_repo(skill, settings)
    if not repos.has_worktree(repo):
        raise ToolError(f"no fix worktree; run skill-fix start {skill}")
    return commit_or_hold(repo, skill, message_file, settings, None)


def commit_or_hold(
    repo: Repo,
    skill: str,
    message_file: Path,
    settings: Settings,
    held_branch: str | None,
) -> dict:
    """Commit the staged fix when it passes the rules, then sync.

    Args:
        repo: The repo whose fix worktree holds the fix.
        skill: The skill the fix is for.
        message_file: The commit message.
        settings: This PC's settings.
        held_branch: The held branch a retry came from. It is deleted
            once the fix commits, and kept when the fix still holds.

    Returns:
        The reply for ``commit`` or ``held retry``.

    Raises:
        ToolError: The sync after the commit failed; the message names
            the commit, which is safe on its branch.
    """
    check = check_fix(repo, skill, message_file, settings)
    if isinstance(check, Refused):
        first = check.findings[0].rule
        places = [f.format_place() for f in check.findings if f.rule == first]
        return {
            "status": "refused",
            "repo": repo.name,
            "rule": first,
            # A withheld place repeats once per finding in its file.
            "detail": ", ".join(dict.fromkeys(places)),
        }
    if isinstance(check, Hold):
        if held_branch is None:
            held_branch = repos.hold_fix(repo, skill, message_file)
        else:
            repos.discard_edits(repo, skill)
        return {
            "status": "held",
            "repo": repo.name,
            "branch": held_branch,
            "held": len(repos.list_held_fixes(repo)),
        }
    branch = repos.read_branch(repo)
    commit = repos.commit_edits(repo, skill, message_file)
    if held_branch is not None:
        repos.run_git(repo.worktree, "branch", "-D", held_branch)
    try:
        result = sync_repo(repo, settings)
    except ToolError as error:
        raise ToolError(f"committed {commit}; sync failed: {error}") from error
    return {
        "status": "committed",
        "repo": repo.name,
        "branch": result.branch or branch,
        "commit": commit,
        "sync": result.state,
        "pr": result.pr,
        "request": result.request,
        "reported": [describe(found) for found in check.reported],
    }


def sync_repos(name: str | None) -> dict:
    """Sync every repo this PC has, or only the one named.

    A repo that fails gets ``sync`` ``error`` and its ``detail``, and
    the other repo still syncs. The reply's status is then ``error``,
    so a caller such as ``pde-installer update`` warns.
    """
    settings = repos.load_settings()
    results: list[dict] = []
    for repo in repos.list_repos(settings):
        if name is None or repo.name == name:
            try:
                result = sync_repo(repo, settings)
            except ToolError as error:
                results.append(
                    {"repo": repo.name, "sync": "error", "detail": str(error)}
                )
                continue
            results.append(
                {
                    "repo": result.repo,
                    "sync": result.state,
                    "branch": result.branch,
                    "pr": result.pr,
                    "request": result.request,
                }
            )
    if name == "work" and not results:
        results.append({"repo": "work", "sync": "skipped"})
    failed = [entry["repo"] for entry in results if entry["sync"] == "error"]
    if failed:
        return {
            "status": "error",
            "detail": f"sync failed for {', '.join(failed)}",
            "repos": results,
        }
    return {"status": "ok", "repos": results}


def report_status() -> dict:
    """Report each repo's fix branch, fix PR and held fixes."""
    settings = repos.load_settings()
    found = [read_status(repo) for repo in repos.list_repos(settings)]
    return {"status": "ok", "repos": found}


def manage_held(action: str, number: int | None) -> dict | None:
    """List, show, drop or retry the personal repo's held fixes.

    ``show`` prints the held commit for the user and returns None.
    The list names each fix's number and skill only, because a held
    fix's title may hold a work term.

    Raises:
        ToolError: The action needs a number, or no held fix has it.
    """
    settings = repos.load_settings()
    repo = repos.pick_repo("personal", settings)
    held = dict(repos.list_held_fixes(repo))
    if action == "list":
        listed = [{"n": n, "skill": skill} for n, skill in held.items()]
        return {"status": "ok", "repo": repo.name, "held": listed}
    if number not in held:
        raise ToolError(f"no held fix {number}; run skill-fix held")
    branch = f"{repos.HELD_PREFIX}{number}"
    if action == "show":
        print(
            repos.run_git(repo.checkout, "show", "--no-color", branch), end=""
        )
        return None
    if action == "drop":
        repos.run_git(repo.checkout, "branch", "-D", branch)
        return {"status": "ok", "repo": repo.name, "held": len(held) - 1}
    return retry_held(repo, branch, held[number], settings)


def retry_held(
    repo: Repo, branch: str, skill: str, settings: Settings
) -> dict:
    """Check a held fix again and commit it to the fix branch if it passes.

    Raises:
        ToolError: The fix worktree is missing or has edits, or the
            held fix no longer applies to the fix branch.
    """
    if not repos.has_worktree(repo):
        raise ToolError(f"no fix worktree; run skill-fix start {skill}")
    if repos.list_changed_paths(repo):
        raise ToolError("the fix worktree has edits; commit or reset them")
    if not repos.try_git(repo.worktree, "cherry-pick", "--no-commit", branch):
        repos.run_git(repo.worktree, "reset", "--quiet", "--merge")
        raise ToolError(f"{branch} no longer applies; redo it from Intent")
    message = repos.run_git(repo.worktree, "log", "-1", "--format=%B", branch)
    with tempfile.TemporaryDirectory() as folder:
        message_file = Path(folder) / "message"
        message_file.write_text(message, encoding="utf-8")
        return commit_or_hold(repo, skill, message_file, settings, branch)


def describe(found: Finding) -> dict:
    """Return a finding as reply JSON with its rule, path and line."""
    return {"rule": found.rule, "path": found.path, "line": found.line}


def build_parser() -> argparse.ArgumentParser:
    """Return the parser for the five commands."""
    parser = argparse.ArgumentParser(
        prog="skill-fix",
        description="Commit a correction to a skill in the skill's repo.",
    )
    parser.add_argument(
        "--version", action="version", version=f"%(prog)s {__version__}"
    )
    commands = parser.add_subparsers(dest="command", required=True)
    start = commands.add_parser("start", help="get the fix worktree ready")
    start.add_argument("skill")
    start.add_argument(
        "--reset", action="store_true", help="drop the skill's edits"
    )
    commit = commands.add_parser("commit", help="check, commit and sync")
    commit.add_argument("skill")
    commit.add_argument("--message-file", required=True)
    sync = commands.add_parser("sync", help="push and open the fix PR")
    sync.add_argument("--repo", choices=["personal", "work"])
    commands.add_parser("status", help="show each repo's fix PR")
    held = commands.add_parser("held", help="list, show, drop or retry")
    held.add_argument(
        "action",
        nargs="?",
        default="list",
        choices=["list", "show", "drop", "retry"],
    )
    held.add_argument("number", nargs="?", type=int)
    return parser


if __name__ == "__main__":
    sys.exit(main())
