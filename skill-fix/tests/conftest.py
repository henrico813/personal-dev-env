"""Temporary repos, a bare remote and stub tools for the skill-fix tests.

Every test runs the file ``build.py`` packs, with HOME in a temporary
folder. Each repo's remote is a bare repository that refuses
non-fast-forward pushes, so a forced push fails any test. ``gh``,
``pde-gh-write`` and ``ci/scan`` are stubs, so no test needs the
network or gitleaks.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import pytest

ROOT = Path(__file__).parents[2]
BUILD = Path(__file__).parents[1] / "build.py"
DEMO = """\
---
name: demo
description: Use when a test needs a skill to fix.
metadata:
  scope: personal
---

# Demo

Do the work in small steps.
Check the result before you report it.
"""
OTHER = """\
---
name: other
description: Use when a test needs a second skill.
metadata:
  scope: personal
---

# Other

Always read the whole plan before you change any code.
"""
MESSAGE = """\
fix(demo): check step {n} before going on

Missed: the agent moved on before it checked step {n}.
Intent: agents check step {n} before they start the next one.
"""
# Stands in for ci/scan, which needs gitleaks, matching PRIVATE_TERMS and
# the work-term list in text and refusing commits while $HOME/scan-refuses
# exists. $HOME/stub/text-mode makes text fail as "broken" (exit 2, a term
# on stderr) or "unplaced" (exit 1, no line).
SCAN_STUB = """\
#!/bin/sh
case $1 in
text)
    case $(cat "$HOME/stub/text-mode" 2>/dev/null) in
    broken)
        echo "scan: cannot read the private terms" >&2
        echo "jq: error: cannot iterate over string (exname)" >&2
        exit 2 ;;
    unplaced)
        echo "private-term (path withheld)"
        exit 1 ;;
    esac
    if [ -n "$PDE_WORK_TERMS_FILE" ]; then
        [ -r "$PDE_WORK_TERMS_FILE" ] || exit 2
    fi
    printf '%s\\n' "$PRIVATE_TERMS" |
        cat - ${PDE_WORK_TERMS_FILE:+"$PDE_WORK_TERMS_FILE"} |
        grep -v -e '^#' -e '^$' >"$HOME/stub/terms"
    grep -n -i -F -f "$HOME/stub/terms" "$2" |
        sed 's/:.*//; s/^/private-term text:/' | grep . && exit 1
    exit 0 ;;
commits)
    [ -e "$HOME/scan-refuses" ] || exit 0
    echo "private-term (path withheld)"
    exit 1 ;;
esac
exit 2
"""
# Prints the PR saved for the branch after --head, else no PR. Fails
# as a GitHub outage would while $HOME/stub/gh-fails exists.
GH_STUB = """\
#!/bin/sh
if [ -e "$HOME/stub/gh-fails" ]; then
    echo "HTTP 502: Bad Gateway" >&2
    exit 1
fi
saved="$HOME/stub/pr-$(printf %s "$4" | tr / _).json"
if [ -e "$saved" ]; then cat "$saved"; else echo '[]'; fi
"""
# Logs each request and its body's checksum, then exits with
# $HOME/stub/exit, or 3.
GH_WRITE_STUB = """\
#!/bin/sh
printf '%s %s\\n' "$*" "$(cksum <"${10}")" >>"$HOME/stub/requests"
code=$(cat "$HOME/stub/exit" 2>/dev/null || echo 3)
id=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
echo "pde-gh-write: waiting for approval, or run pde-pr-approve $id" >&2
[ "$code" = 3 ] && echo "pde-gh-write: still waiting for approval of $id" >&2
exit "$code"
"""
REQUEST_ID = "0123456789abcdef" * 4


@dataclass
class Reply:
    """One skill-fix run: its exit code and its JSON reply."""

    code: int
    body: dict


@dataclass
class Repos:
    """The paths a test works with.

    Attributes:
        home: The test's HOME.
        checkout: The user's personal checkout, remote named github.
        remote: The bare repository that stands in for GitHub.
        upstream: A second clone that pushes to main, as a merge does.
        worktree: Where skill-fix puts the personal fix worktree.
    """

    home: Path
    checkout: Path
    remote: Path
    upstream: Path
    worktree: Path


Run = Callable[..., Reply]


@pytest.fixture(scope="session")
def packed(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """The file build.py packs, built once per session.

    Running it instead of the source fails the suite when packing
    loses a module or the exit code.
    """
    output = tmp_path_factory.mktemp("build") / "skill-fix"
    subprocess.run([sys.executable, "-I", str(BUILD), str(output)], check=True)
    return output


@pytest.fixture
def repos(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Repos:
    """A PC labelled desk with the demo skill installed and in its checkout.

    The user's git config is ignored, so hooks or signing cannot change
    what a test sees, and so is any ``PRIVATE_TERMS`` in the shell.
    """
    home = tmp_path / "home"
    stub = home / "stub"
    stub.mkdir(parents=True)
    write_tool(stub / "gh", GH_STUB)
    write_tool(stub / "pde-gh-write", GH_WRITE_STUB)
    gitconfig = tmp_path / "gitconfig"
    gitconfig.write_text("[user]\n\tname = Dev\n\temail = dev@example.com\n")
    monkeypatch.setenv("HOME", str(home))
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", str(gitconfig))
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    monkeypatch.delenv("PRIVATE_TERMS", raising=False)
    monkeypatch.setenv("PATH", f"{stub}{os.pathsep}{os.environ['PATH']}")
    remote = tmp_path / "personal.git"
    upstream = tmp_path / "upstream"
    run_git(tmp_path, "init", "--quiet", "--bare", "-b", "main", str(remote))
    run_git(remote, "config", "receive.denyNonFastForwards", "true")
    run_git(tmp_path, "init", "--quiet", "-b", "main", str(upstream))
    write_personal_files(upstream)
    run_git(upstream, "add", ".")
    run_git(upstream, "commit", "--quiet", "-m", "Add skills")
    run_git(upstream, "remote", "add", "origin", str(remote))
    run_git(upstream, "push", "--quiet", "-u", "origin", "main")
    checkout = tmp_path / "checkout"
    run_git(
        tmp_path,
        "clone",
        "--quiet",
        "-o",
        "github",
        str(remote),
        str(checkout),
    )
    install_skill(home, "demo", DEMO)
    write_settings(home, install_path=str(checkout), skill_fix_label="desk")
    return Repos(
        home=home,
        checkout=checkout,
        remote=remote,
        upstream=upstream,
        worktree=home / ".local/share/skill-fix/personal",
    )


@pytest.fixture
def run(packed: Path, repos: Repos) -> Run:
    """Return a function that runs skill-fix and parses its reply."""

    def run_packed(*args: str) -> Reply:
        """Run the packed file with args and return its reply."""
        result = subprocess.run(
            [sys.executable, "-I", str(packed), *args],
            capture_output=True,
            text=True,
        )
        assert result.stderr == ""
        return Reply(result.returncode, json.loads(result.stdout))

    return run_packed


@pytest.fixture
def fix(run: Run, repos: Repos, tmp_path: Path) -> Callable[..., Reply]:
    """Return a function that makes one fix to demo and commits it.

    Each call adds one line to demo's SKILL.md in the fix worktree and
    commits it with a title naming that line.
    """
    count = 0

    def make_fix(
        line: str = "",
        message: str = MESSAGE,
        extra: str = "",
        extra_text: str = "Changed.\n",
    ) -> Reply:
        """Start, append a line to demo, and commit with message.

        Args:
            line: The line to add, else one naming the fix's number.
            message: The commit message, with {n} for that number.
            extra: A file in the worktree to change as well.
            extra_text: The text to write to that file.
        """
        nonlocal count
        count += 1
        started = run("start", "demo")
        assert started.code == 0, started.body
        if extra:
            write(repos.worktree / extra, extra_text)
        skill_md = Path(started.body["file"])
        added = line or f"Check step {count} first."
        skill_md.write_text(skill_md.read_text() + added + "\n")
        message_file = tmp_path / f"message-{count}"
        message_file.write_text(message.format(n=count))
        return run("commit", "demo", "--message-file", str(message_file))

    return make_fix


def write_personal_files(root: Path) -> None:
    """Write the skills, the checker scripts and the stub ci/scan."""
    write(root / "ai/skills/demo/SKILL.md", DEMO)
    write(root / "ai/skills/other/SKILL.md", OTHER)
    write(root / "ai/README.md", "# Skills\n")
    for name in ("skill-development", "git-messages"):
        header = f"---\nname: {name}\ndescription: Use when testing.\n"
        write(
            root / f"ai/skills/{name}/SKILL.md",
            f"{header}metadata:\n  scope: personal\n---\n\n# {name}\n",
        )
    scripts = "ai/skills/skill-development/scripts/skill_check.py"
    copy(ROOT / scripts, root / scripts)
    copy(
        ROOT / "ai/skills/git-messages/scripts/msg",
        root / "ai/skills/git-messages/scripts/msg",
    )
    write_tool(root / "ci/scan", SCAN_STUB)


def write_settings(home: Path, **settings: str) -> None:
    """Write config.json with these settings only."""
    write(home / ".config/pde/config.json", json.dumps(settings))


def install_skill(home: Path, name: str, text: str) -> None:
    """Write text as the installed SKILL.md of name."""
    write(home / ".agents/skills" / name / "SKILL.md", text)


def save_pr(home: Path, branch: str, **pr: object) -> None:
    """Make the stub gh report this PR for branch."""
    path = home / "stub" / f"pr-{branch.replace('/', '_')}.json"
    path.write_text(json.dumps([pr]))


def read_requests(home: Path) -> list[str]:
    """Return each PR request the stub pde-gh-write saw, one per line."""
    path = home / "stub/requests"
    if not path.exists():
        return []
    return path.read_text().splitlines()


def run_git(where: Path, *args: str) -> str:
    """Run git in where, fail the test if it fails, and return its output."""
    result = subprocess.run(
        ["git", *args], cwd=where, capture_output=True, text=True
    )
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def write(path: Path, text: str) -> None:
    """Write text to path, making its folder."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def write_tool(path: Path, text: str) -> None:
    """Write an executable script to path."""
    write(path, text)
    path.chmod(0o755)


def copy(source: Path, target: Path) -> None:
    """Copy one file, making the target's folder."""
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy(source, target)
