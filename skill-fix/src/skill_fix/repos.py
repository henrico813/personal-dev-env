"""Find a skill's repo from its scope tag and run git in its fix worktree.

Each repo's fix worktree is ``~/.local/share/skill-fix/<repo>``. It
shares the checkout's ``.git`` but has its own files and branch, so
the user's checkout keeps its branch and its uncommitted work.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

RepoName = Literal["personal", "work"]
SKILLS_DIRS: dict[RepoName, str] = {"personal": "ai/skills", "work": "skills"}
BATCH_PREFIX = "fix/skill-fixes-"
HELD_PREFIX = "fix/skill-fixes-held-"
# No remote has this name, so a plain git push of a held branch fails.
NO_REMOTE = "skill-fix-held-never-pushed"
LABEL = re.compile(r"[a-z0-9]+")
SKILL_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
# A fetch or push that hangs must not hold an agent's command forever.
GIT_TIMEOUT = 120


class RefusalError(Exception):
    """A fix or command that breaks a fix rule, such as ``route``."""

    def __init__(self, rule: str, detail: str) -> None:
        """Keep the rule and the place it names for the reply.

        Args:
            rule: The rule's name, such as ``route``.
            detail: What broke it, never a matched work term.
        """
        super().__init__(f"{rule}: {detail}")
        self.rule = rule
        self.detail = detail


class ToolError(Exception):
    """A broken repository, setting or tool, reported as ``error``."""


@dataclass(frozen=True)
class Settings:
    """Per-PC settings read from ``~/.config/pde/config.json``.

    Attributes:
        install_path: The personal-dev-env checkout install ran from.
        label: ``skill_fix_label``, the word for this PC in branch
            names and PR titles, or None until it is set.
        work_skills_repo: The work repo's checkout, or None.
        work_terms_file: The work-term list, or None.
    """

    install_path: Path | None
    label: str | None
    work_skills_repo: Path | None
    work_terms_file: Path | None


@dataclass(frozen=True)
class Repo:
    """One skills repo on this PC and where its fix worktree lives.

    Attributes:
        name: ``personal`` or ``work``.
        checkout: The user's checkout, which only lends its ``.git``.
        skills_dir: The skills folder inside the repo.
        worktree: The fix worktree.
        remote: The remote that ``main`` tracks, else ``origin``.
        term_check: Whether fixes are checked against the work terms.
    """

    name: RepoName
    checkout: Path
    skills_dir: str
    worktree: Path
    remote: str
    term_check: bool

    def find_skill_dir(self, skill: str) -> str:
        """Return the repo-relative folder of ``skill``."""
        return f"{self.skills_dir}/{skill}"


def load_settings() -> Settings:
    """Read the per-PC settings; a missing file means none are set.

    Unknown keys are ignored. A leading ``~/`` in a path is expanded.

    Returns:
        The settings.

    Raises:
        ToolError: The file is not a JSON object, or the label is not
            lowercase letters and digits.
    """
    path = Path.home() / ".config/pde/config.json"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        data = {}
    except (OSError, ValueError) as error:
        raise ToolError(f"cannot read {path}") from error
    if not isinstance(data, dict):
        raise ToolError(f"{path} is not a JSON object")
    label = data.get("skill_fix_label") or None
    if label is not None and not (
        isinstance(label, str) and LABEL.fullmatch(label)
    ):
        raise ToolError("skill_fix_label must be lowercase letters and digits")
    return Settings(
        install_path=to_path(data.get("install_path")),
        label=label,
        work_skills_repo=to_path(data.get("work_skills_repo")),
        work_terms_file=to_path(data.get("work_terms_file")),
    )


def to_path(value: object) -> Path | None:
    """Return a setting as a path, or None when it is empty or not text."""
    if not isinstance(value, str) or not value:
        return None
    return Path(value).expanduser()


def find_repo(skill: str, settings: Settings) -> Repo:
    """Return the repo that holds the source of an installed skill.

    The installed copy's scope tag names the repo, and the repo's
    ``main`` must hold the skill.

    Args:
        skill: The skill's folder name.
        settings: This PC's settings.

    Returns:
        The skill's repo.

    Raises:
        RefusalError: ``no-skill`` when the skill is not installed, or
            ``route`` when its tag is missing or names a repo that is
            not set up here or does not hold it.
        ToolError: The personal checkout is not known.
    """
    installed = Path.home() / ".agents/skills" / skill / "SKILL.md"
    if not SKILL_NAME.fullmatch(skill) or not installed.is_file():
        raise RefusalError("no-skill", f"{skill} is not installed")
    scope = read_scope(installed.read_text(encoding="utf-8"))
    if scope is None:
        raise RefusalError("route", f"{skill} has no metadata.scope")
    repo = pick_repo(scope, settings)
    source = f"{repo.remote}/main:{repo.find_skill_dir(skill)}/SKILL.md"
    if not try_git(repo.checkout, "cat-file", "-e", source):
        raise RefusalError("route", f"{skill} is not in the {scope} repo")
    return repo


def read_scope(text: str) -> RepoName | None:
    """Return ``metadata.scope`` from a SKILL.md header.

    Args:
        text: The whole SKILL.md.

    Returns:
        ``personal`` or ``work``, or None when the tag is missing or
        holds another value.
    """
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    in_metadata = False
    for line in lines[1:]:
        if line.strip() == "---":
            break
        if not line.startswith((" ", "\t")):
            in_metadata = line.rstrip() == "metadata:"
            continue
        key, _, value = line.strip().partition(":")
        if in_metadata and key == "scope":
            value = value.strip().strip("\"'")
            if value == "personal":
                return "personal"
            if value == "work":
                return "work"
            return None
    return None


def pick_repo(scope: RepoName, settings: Settings) -> Repo:
    """Return the repo a scope tag names.

    Args:
        scope: The skill's tag.
        settings: This PC's settings.

    Returns:
        The repo, with its fix worktree path.

    Raises:
        RefusalError: ``route`` when the tag is ``work`` and this PC has no
            ``work_skills_repo``.
        ToolError: The tag is ``personal`` and config.json has no
            ``install_path``.
    """
    if scope == "personal":
        checkout = settings.install_path
        if checkout is None:
            raise ToolError(
                "install_path is not set; run pde-installer install"
            )
    else:
        checkout = settings.work_skills_repo
        if checkout is None:
            raise RefusalError(
                "route", "tag is work, and work_skills_repo is unset"
            )
    return Repo(
        name=scope,
        checkout=checkout,
        skills_dir=SKILLS_DIRS[scope],
        worktree=find_state_dir() / scope,
        remote=find_remote(checkout),
        term_check=scope == "personal"
        and settings.work_terms_file is not None,
    )


def list_repos(settings: Settings) -> list[Repo]:
    """Return the repos this PC has, personal first."""
    names: list[RepoName] = ["personal"]
    if settings.work_skills_repo is not None:
        names.append("work")
    return [pick_repo(name, settings) for name in names]


def find_state_dir() -> Path:
    """Return the folder that holds the fix worktrees."""
    return Path.home() / ".local/share/skill-fix"


def find_remote(checkout: Path) -> str:
    """Return the remote that ``main`` tracks, else ``origin``.

    A clone may name its GitHub remote something other than origin.
    """
    result = call_git(checkout, ["config", "--get", "branch.main.remote"])
    return result.stdout.strip() or "origin"


def ensure_worktree(repo: Repo, label: str | None) -> str:
    """Add the fix worktree on first use and return its branch.

    A new worktree checks out the highest batch branch, local or on
    the remote, else a new ``-1`` branch from the remote's ``main``.
    The label reads ``unset`` until ``skill_fix_label`` is set; sync
    renames that branch before its first push.

    Args:
        repo: The repo to work in.
        label: This PC's label, or None.

    Returns:
        The branch checked out in the worktree.
    """
    if has_worktree(repo):
        return read_branch(repo)
    # A worktree folder deleted by hand stays registered until a prune.
    run_git(repo.checkout, "worktree", "prune")
    name = label or "unset"
    numbers = list_batch_numbers(repo, name)
    branch = f"{BATCH_PREFIX}{name}-{max(numbers, default=1)}"
    repo.worktree.parent.mkdir(parents=True, exist_ok=True)
    where = str(repo.worktree)
    if try_git(repo.checkout, "rev-parse", "--verify", f"refs/heads/{branch}"):
        run_git(repo.checkout, "worktree", "add", "--quiet", where, branch)
    elif try_git(
        repo.checkout, "rev-parse", "--verify", f"{repo.remote}/{branch}"
    ):
        run_git(
            repo.checkout,
            "worktree",
            "add",
            "--quiet",
            "-b",
            branch,
            where,
            f"{repo.remote}/{branch}",
        )
    else:
        run_git(
            repo.checkout,
            "worktree",
            "add",
            "--quiet",
            "--no-track",
            "-b",
            branch,
            where,
            f"{repo.remote}/main",
        )
    return branch


def has_worktree(repo: Repo) -> bool:
    """Return whether the fix worktree exists."""
    return (repo.worktree / ".git").exists()


def read_branch(repo: Repo) -> str:
    """Return the branch checked out in the fix worktree."""
    return run_git(repo.worktree, "symbolic-ref", "--short", "HEAD").strip()


def list_batch_numbers(repo: Repo, label: str) -> list[int]:
    """Return the batch numbers of this label's branches, local or remote."""
    output = run_git(
        repo.checkout,
        "for-each-ref",
        "--format=%(refname)",
        f"refs/heads/{BATCH_PREFIX}{label}-*",
        f"refs/remotes/{repo.remote}/{BATCH_PREFIX}{label}-*",
    )
    pattern = re.compile(rf"/{re.escape(BATCH_PREFIX + label)}-(\d+)$")
    return [
        int(match.group(1))
        for match in map(pattern.search, output.splitlines())
        if match
    ]


def name_next_batch(repo: Repo, label: str) -> str:
    """Return the branch name one batch past this label's highest."""
    number = max(list_batch_numbers(repo, label), default=0) + 1
    return f"{BATCH_PREFIX}{label}-{number}"


def list_changed_paths(repo: Repo) -> list[str]:
    """Return every file the fix worktree changed, staged or not.

    NUL-separated output gives each name as it is, never quoted.
    """
    tracked = run_git(repo.worktree, "diff", "--name-only", "-z", "HEAD")
    untracked = run_git(
        repo.worktree, "ls-files", "-z", "--others", "--exclude-standard"
    )
    names = tracked.split("\0") + untracked.split("\0")
    return sorted({name for name in names if name})


def count_commits(repo: Repo, base: str) -> int:
    """Return how many commits the worktree's branch has that base lacks."""
    output = run_git(repo.worktree, "rev-list", "--count", f"{base}..HEAD")
    return int(output)


def commit_edits(repo: Repo, skill: str, message_file: Path) -> str:
    """Commit the skill's edits to the worktree's branch.

    Returns:
        The new commit's short hash.
    """
    run_git(repo.worktree, "add", "--all", "--", repo.find_skill_dir(skill))
    run_git(repo.worktree, "commit", "--quiet", "--file", str(message_file))
    return run_git(repo.worktree, "rev-parse", "--short", "HEAD").strip()


def hold_fix(repo: Repo, skill: str, message_file: Path) -> str:
    """Move the skill's edits to a new held branch that cannot be pushed.

    The held commit's parent is the fix branch's head, so ``held
    retry`` can pick it onto the fix branch later. The fix branch and
    its files end as they were before the edits.

    Returns:
        The held branch, such as ``fix/skill-fixes-held-1``.
    """
    numbers = [number for number, _ in list_held_fixes(repo)]
    branch = f"{HELD_PREFIX}{max(numbers, default=0) + 1}"
    with tempfile.TemporaryDirectory() as folder:
        # A separate index builds the commit without moving the branch.
        index = {"GIT_INDEX_FILE": str(Path(folder) / "index")}
        run_git(repo.worktree, "read-tree", "HEAD", env=index)
        run_git(
            repo.worktree,
            "add",
            "--all",
            "--",
            repo.find_skill_dir(skill),
            env=index,
        )
        tree = run_git(repo.worktree, "write-tree", env=index).strip()
    commit = run_git(
        repo.worktree,
        "commit-tree",
        tree,
        "-p",
        "HEAD",
        "-F",
        str(message_file),
    ).strip()
    run_git(repo.worktree, "branch", branch, commit)
    run_git(repo.worktree, "config", f"branch.{branch}.pushRemote", NO_REMOTE)
    discard_edits(repo, skill)
    return branch


def discard_edits(repo: Repo, skill: str) -> None:
    """Put the skill's folder in the fix worktree back to the branch head."""
    folder = repo.find_skill_dir(skill)
    run_git(repo.worktree, "reset", "--quiet", "--", folder)
    run_git(repo.worktree, "checkout", "HEAD", "--", folder)
    run_git(repo.worktree, "clean", "-fdq", "--", folder)


def list_held_fixes(repo: Repo) -> list[tuple[int, str]]:
    """Return each held fix's number and skill, oldest first."""
    output = run_git(
        repo.checkout,
        "for-each-ref",
        "--format=%(refname:short)",
        f"refs/heads/{HELD_PREFIX}*",
    )
    held = []
    for branch in output.splitlines():
        number = branch.removeprefix(HELD_PREFIX)
        if number.isdigit():
            held.append((int(number), find_held_skill(repo, branch)))
    return sorted(held)


def find_held_skill(repo: Repo, branch: str) -> str:
    """Return the skill a held commit changes, read from its first path."""
    paths = run_git(
        repo.checkout,
        "diff-tree",
        "--no-commit-id",
        "--name-only",
        "-r",
        branch,
    ).splitlines()
    prefix = f"{repo.skills_dir}/"
    for path in paths:
        if path.startswith(prefix):
            return path[len(prefix) :].split("/")[0]
    raise ToolError(f"{branch} changes no skill")


def run_git(where: Path, *args: str, env: dict[str, str] | None = None) -> str:
    """Run git in a folder and return its output.

    Args:
        where: The folder to run in.
        args: The git command and its arguments.
        env: Variables to add to this process's environment.

    Returns:
        Git's standard output.

    Raises:
        ToolError: Git failed; the message holds git's last error line.
    """
    result = call_git(where, list(args), env)
    if result.returncode != 0:
        lines = result.stderr.strip().splitlines() or [
            f"exit {result.returncode}"
        ]
        raise ToolError(f"git {args[0]} failed: {lines[-1]}")
    return result.stdout


def try_git(where: Path, *args: str) -> bool:
    """Run git in a folder and return whether it succeeded."""
    return call_git(where, list(args)).returncode == 0


def call_git(
    where: Path, args: list[str], env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run git without a password prompt, timing out as a failure.

    Args:
        where: The folder to run in.
        args: The git command and its arguments.
        env: Variables to add to this process's environment.

    Returns:
        The finished process; a timeout returns exit code 1.
    """
    try:
        return subprocess.run(
            ["git", *args],
            cwd=where,
            capture_output=True,
            text=True,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0", **(env or {})},
            timeout=GIT_TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(args, 1, "", "git timed out")
    except FileNotFoundError as error:
        raise ToolError(f"cannot run git in {where}") from error
