"""The Fix Rules a fix must pass before it becomes a commit.

Read ``check_fix`` first. The text checks it calls stay in
``skill_check.py``, ``msg lint`` and ``ci/scan``, so a skill author
and skill-fix run the same checks. A finding names a file and line,
never the text that broke the rule.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Literal, Union

from skill_fix.repos import (
    Repo,
    Settings,
    ToolError,
    list_changed_paths,
    read_scope,
    run_git,
)

Rule = Literal[
    "other-files",
    "route",
    "width",
    "repeats",
    "message",
    "scan",
    "work-term",
]
SKILL_CHECK = "ai/skills/skill-development/scripts/skill_check.py"
MSG = "ai/skills/git-messages/scripts/msg"
PROBLEM = re.compile(r"^(.+):(\d+): (error|repeat): (.*)$")
LINT_LINE = re.compile(r"^line (\d+): ")
SCAN_LINE = re.compile(r"^(\S+) text:(\d+)")
# The place of a finding in a file name, which may hold the term.
WITHHELD = "(path withheld)"
HUNK = re.compile(r"^@@ -\S+ \+(\d+)(?:,\d+)? @@")


@dataclass(frozen=True)
class Finding:
    """One broken rule and where, such as ``width`` at SKILL.md:88."""

    rule: Rule
    path: str
    line: int | None

    def format_place(self) -> str:
        """Return ``path:line``, or the path alone when there is no line."""
        if self.line is None:
            return self.path
        return f"{self.path}:{self.line}"


@dataclass(frozen=True)
class Pass:
    """The fix may be committed; ``reported`` holds its repeats."""

    reported: tuple[Finding, ...]


@dataclass(frozen=True)
class Refused:
    """The fix broke at least one rule, and nothing is committed."""

    findings: tuple[Finding, ...]


@dataclass(frozen=True)
class Hold:
    """The fix holds a work term, so it goes to a held branch."""

    findings: tuple[Finding, ...]


FixCheck = Union[Pass, Refused, Hold]


def check_fix(
    repo: Repo, skill: str, message_file: Path, settings: Settings
) -> FixCheck:
    """Check the fix worktree's uncommitted edits to one skill.

    Every rule but ``repeats`` refuses the fix, ``scan`` included. A
    work term holds it instead, once every other rule passes. The
    skill's edits are staged, which the commit or the hold then uses.

    Args:
        repo: The repo whose fix worktree holds the edits.
        skill: The skill the fix is for.
        message_file: The commit message.
        settings: This PC's settings, for the checker scripts in the
            personal checkout and the work-term list.

    Returns:
        Pass, Refused or Hold.

    Raises:
        ToolError: Nothing changed, or a checker could not run.
    """
    folder = repo.find_skill_dir(skill)
    paths = list_changed_paths(repo)
    if not paths:
        raise ToolError(f"no edits to commit in {folder}")
    others = [
        Finding("other-files", path, None)
        for path in paths
        if not path.startswith(f"{folder}/")
    ]
    if others:
        return Refused(tuple(others))
    run_git(repo.worktree, "add", "--all", "--", folder)
    findings = (
        check_route(repo, skill)
        + run_skill_check(repo, settings)
        + check_message(repo, message_file, settings)
    )
    failed = tuple(found for found in findings if found.rule != "repeats")
    if failed:
        return Refused(failed)
    if repo.name == "personal":
        found = find_scan_findings(repo, skill, message_file)
        if found:
            return Refused(tuple(found))
    if repo.term_check:
        terms = find_work_terms(repo, skill, message_file, settings)
        if terms:
            return Hold(tuple(terms))
    return Pass(tuple(found for found in findings if found.rule == "repeats"))


def check_route(repo: Repo, skill: str) -> list[Finding]:
    """Refuse an edit that changes the skill's scope tag away from its repo."""
    path = f"{repo.find_skill_dir(skill)}/SKILL.md"
    skill_md = repo.worktree / path
    if (
        skill_md.is_file()
        and read_scope(skill_md.read_text(encoding="utf-8")) == repo.name
    ):
        return []
    return [Finding("route", path, None)]


def run_skill_check(repo: Repo, settings: Settings) -> list[Finding]:
    """Return the width errors and repeats on lines the fix adds.

    ``skill_check.py`` compares the worktree with its branch head, so
    old wide lines never block a fix. Its other errors are left to
    the fix PR's checks.

    Raises:
        ToolError: The script could not run.
    """
    script = find_checker(settings, SKILL_CHECK)
    result = subprocess.run(
        [
            sys.executable,
            "-I",
            str(script),
            "--skills",
            str(repo.worktree / repo.skills_dir),
            "--base",
            "HEAD",
        ],
        cwd=repo.worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode not in (0, 1):
        raise ToolError("skill_check.py could not run")
    findings = []
    for line in result.stdout.splitlines():
        match = PROBLEM.match(line)
        if not match:
            continue
        path, number, kind, message = match.groups()
        if kind == "repeat":
            findings.append(Finding("repeats", path, int(number)))
        elif "wrap at" in message:
            findings.append(Finding("width", path, int(number)))
    return findings


def check_message(
    repo: Repo, message_file: Path, settings: Settings
) -> list[Finding]:
    """Refuse a title ``msg lint`` fails, or a body without its two lines.

    The body needs ``Missed:`` and ``Intent:`` lines, so an agent can
    redo the fix from them after a conflict.

    Raises:
        ToolError: ``msg lint`` could not run.
    """
    script = find_checker(settings, MSG)
    result = subprocess.run(
        [sys.executable, "-I", str(script), "lint", str(message_file)],
        cwd=repo.worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode not in (0, 1):
        raise ToolError("msg lint could not run")
    findings = []
    for line in result.stdout.splitlines():
        match = LINT_LINE.match(line)
        if match:
            findings.append(Finding("message", "message", int(match.group(1))))
    lines = message_file.read_text(encoding="utf-8").splitlines()
    for word in ("Missed:", "Intent:"):
        if not any(line.startswith(word) for line in lines[1:]):
            findings.append(Finding("message", f"no {word} line", None))
    return findings


def find_scan_findings(
    repo: Repo, skill: str, message_file: Path
) -> list[Finding]:
    """Return where the added text, file names or message fail ``ci/scan``.

    This is sync's scan before each push, without the work terms. A
    fix that committed with such text would stop every later sync at
    ``scan``, so it is refused here instead.

    Raises:
        ToolError: ``ci/scan`` could not run.
    """
    found = scan_fix_text(repo, skill, message_file, None)
    return [Finding("scan", path, number) for _, path, number in found]


def find_work_terms(
    repo: Repo, skill: str, message_file: Path, settings: Settings
) -> list[Finding]:
    """Return where the added lines or the message hold a work term.

    ``ci/scan text`` reads only the work-term list here, so a
    ``private-term`` finding means a work term.

    Raises:
        ToolError: ``ci/scan`` could not run, such as when the list
            is missing.
    """
    found = scan_fix_text(repo, skill, message_file, settings.work_terms_file)
    return [
        Finding("work-term", path, number)
        for rule, path, number in found
        if rule == "private-term"
    ]


def scan_fix_text(
    repo: Repo, skill: str, message_file: Path, work_terms: Path | None
) -> list[tuple[str, str, int | None]]:
    """Run the worktree's ``ci/scan text`` on the added lines and message.

    The new file names are scanned too. The scan prints the line of
    its input, which maps back to a file and line. The place is
    withheld for a file whose name holds a finding, and for a finding
    the scan printed with no line.

    Args:
        repo: The repo whose fix worktree holds the staged fix.
        skill: The skill the fix is for.
        message_file: The commit message.
        work_terms: The work-term list to scan with alone, or None to
            scan with the private list and no work terms.

    Returns:
        Each finding's scan rule, such as ``private-term``, with its
        file and line.

    Raises:
        ToolError: ``ci/scan`` could not run.
    """
    added, lines = list_added_lines(repo, skill)
    places: list[tuple[str, int | None]] = list(added)
    names = list_new_paths(repo, skill)
    places += [(name, None) for name in names]
    lines += names
    message = message_file.read_text(encoding="utf-8").splitlines()
    places += [("message", number) for number in range(1, len(message) + 1)]
    lines += message
    with tempfile.TemporaryDirectory() as folder:
        text = Path(folder) / "text"
        text.write_text("\n".join(lines) + "\n", encoding="utf-8")
        none = Path(folder) / "none"
        env = dict(os.environ)
        if work_terms is None:
            # An empty work-term list leaves the private list on.
            none.write_text("", encoding="utf-8")
            env["PDE_WORK_TERMS_FILE"] = str(none)
        else:
            # A private list that does not exist leaves only the work terms.
            env.pop("PRIVATE_TERMS", None)
            env["PDE_PRIVATE_TERMS_FILE"] = str(none)
            env["PDE_WORK_TERMS_FILE"] = str(work_terms)
        result = subprocess.run(
            [str(repo.worktree / "ci/scan"), "text", str(text)],
            cwd=repo.worktree,
            capture_output=True,
            text=True,
            env=env,
        )
    if result.returncode not in (0, 1):
        raise ToolError(
            f"ci/scan could not check the fix: {describe_scan_failure(result)}"
        )
    found: list[tuple[str, str, int | None]] = []
    for line in result.stdout.splitlines():
        match = SCAN_LINE.match(line)
        if match:
            path, number = places[int(match.group(2)) - 1]
            found.append((match.group(1), path, number))
    # A finding in a file name withholds every place in that file.
    named = {path for _, path, number in found if number is None}
    found = [
        (rule, WITHHELD, None) if path in named else (rule, path, number)
        for rule, path, number in found
    ]
    if result.returncode == 1 and not found:
        found.append(("unplaced", WITHHELD, None))
    return found


def describe_scan_failure(result: subprocess.CompletedProcess[str]) -> str:
    """Return ``ci/scan``'s last own error line, or its exit code.

    Only lines that start with ``scan:`` are shown, since a tool it
    runs, such as jq, may quote a private term in its error.
    """
    own = [
        line
        for line in result.stderr.splitlines()
        if line.startswith("scan: ")
    ]
    return own[-1] if own else f"exit {result.returncode}"


def list_new_paths(repo: Repo, skill: str) -> list[str]:
    """Return the skill's staged new files, renamed ones included."""
    output = run_git(
        repo.worktree,
        "diff",
        "--cached",
        "--name-only",
        "-z",
        "--diff-filter=A",
        "--no-renames",
        "HEAD",
        "--",
        repo.find_skill_dir(skill),
    )
    # NUL-separated names come as they are, never quoted or escaped.
    return [name for name in output.split("\0") if name]


def list_added_lines(
    repo: Repo, skill: str
) -> tuple[list[tuple[str, int]], list[str]]:
    """Return each staged added line of the skill and its file and line."""
    # Unquoted paths, so a non-ASCII name matches its file.
    diff = run_git(
        repo.worktree,
        "-c",
        "core.quotePath=false",
        "diff",
        "--cached",
        "-U0",
        "--no-color",
        "--no-ext-diff",
        "HEAD",
        "--",
        repo.find_skill_dir(skill),
    )
    places: list[tuple[str, int]] = []
    lines: list[str] = []
    path, number = "", 0
    for line in diff.splitlines():
        hunk = HUNK.match(line)
        if line.startswith("+++ "):
            # Git ends the line with a tab when the path holds a space.
            path = line[6:].removesuffix("\t")
        elif hunk:
            number = int(hunk.group(1))
        elif line.startswith("+"):
            places.append((path, number))
            lines.append(line[1:])
            number += 1
    return places, lines


def find_checker(settings: Settings, relative: str) -> Path:
    """Return a checker script in the personal checkout.

    Raises:
        ToolError: The personal checkout is not known.
    """
    if settings.install_path is None:
        raise ToolError("install_path is not set; run pde-installer install")
    return settings.install_path / relative
