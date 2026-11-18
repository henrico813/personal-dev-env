"""Build a fix PR's body from the fix commits on its branch.

Read ``build_pr_body`` first. ``skill-fix commit`` refuses a fix whose
message lacks a ``Missed:`` and an ``Intent:`` line, so each fix gets
one Changes line that says what changed and why. Sync rebuilds the
body of a forge fix PR on each run, so the body lists every fix.
"""

from __future__ import annotations

import re
import textwrap

from skill_fix import repos
from skill_fix.repos import Repo

FIX_LINE = re.compile(r"(Missed|Intent):\s*(.*)")
# git log ends each message with FILES and starts each commit with
# COMMIT, so a message's own text never splits a commit.
COMMIT = "\x1e"
FILES = "\x1f"
WIDTH = 72
OVERVIEW = """\
## Overview

Each commit in this PR fixes one agent skill after a correction. Each
fix adds a correction case to the skill's evals folder, a JSON file
that checks the new rule.
"""
TESTING = """\
## Testing

skill-fix checked each fix before it committed it: `msg lint` on the
message, its Missed and Intent lines, and its new correction case. No
check here shows an agent following a new rule; the correction cases
do that when they run.

```
Checks
pass  skill-fix rules on each fix
{scan}
not run  correction cases: skill-fix runs no model
not run  try in a session: a fix takes effect after merge and update
```
"""
SCANNED = "pass  ci/scan commits before each push"
NOT_SCANNED = "not run  ci/scan: only the personal repo has it"


def build_pr_body(repo: Repo) -> str:
    """Return the fix PR's body, with one Changes line per fix commit.

    Merge commits, which sync makes to clear a conflict with main, get
    no line.
    """
    changes = [
        describe_fix(message, files, repo.skills_dir)
        for message, files in list_fixes(repo)
    ]
    scan = SCANNED if repo.name == "personal" else NOT_SCANNED
    return "\n".join(
        [
            OVERVIEW,
            "## Changes\n",
            *changes,
            "",
            TESTING.format(scan=scan),
        ]
    )


def list_fixes(repo: Repo) -> list[tuple[str, list[str]]]:
    """Return each fix commit's message and changed paths, oldest first."""
    output = repos.run_git(
        repo.worktree,
        "log",
        "--reverse",
        "--no-merges",
        "--name-only",
        f"--format={COMMIT}%B{FILES}",
        f"{repo.remote}/main..HEAD",
    )
    fixes = []
    for commit in output.split(COMMIT)[1:]:
        message, _, paths = commit.partition(FILES)
        fixes.append((message, [p for p in paths.splitlines() if p]))
    return fixes


def describe_fix(message: str, paths: list[str], skills_dir: str) -> str:
    """Return one Changes line: the skill, then the Intent, then the Missed.

    A line reads like ``- tool: agents check step 1 first. Before, the
    agent moved on.`` A message without an Intent line, such as one
    written by hand, uses its title's summary instead.
    """
    title = message.strip().splitlines()[0]
    found = read_fix_lines(message)
    intent = found.get("Intent") or title.partition(": ")[2] or title
    line = f"{find_skill(paths, skills_dir)}: {intent.rstrip('.')}."
    if "Missed" in found:
        line += f" Before, {found['Missed'].rstrip('.')}."
    return textwrap.fill(
        line, WIDTH, initial_indent="- ", subsequent_indent="  "
    )


def read_fix_lines(message: str) -> dict[str, str]:
    """Return the Missed and Intent text of a message, joined onto one line.

    Each runs from its label to the next blank line or label.
    """
    found: dict[str, str] = {}
    label = None
    for line in message.splitlines():
        match = FIX_LINE.fullmatch(line.strip())
        if match:
            label = match[1]
            found[label] = match[2]
        elif label is not None and line.strip():
            found[label] += " " + line.strip()
        else:
            label = None
    return found


def find_skill(paths: list[str], skills_dir: str) -> str:
    """Return the skill whose folder the paths are in, or "skills"."""
    prefix = f"{skills_dir}/"
    for path in paths:
        if path.startswith(prefix):
            return path[len(prefix) :].split("/")[0]
    return "skills"
