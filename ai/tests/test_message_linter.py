"""Behavior tests for the repository-independent message tool."""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
MSG = ROOT / "ai/skills/git-messages/scripts/msg"


def run_msg(tmp_path: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run msg against an isolated message file."""
    return subprocess.run(
        [sys.executable, str(MSG), *args, str(tmp_path / "message")],
        capture_output=True,
        text=True,
    )


def test_lint_accepts_issue_suffix(tmp_path: Path) -> None:
    """Keep GitHub issue suffixes out of the title limit.

    GitHub appends the issue number to squash titles, so a valid title
must remain valid when that suffix is present.
    """
    (tmp_path / "message").write_text("fix: " + "x" * 45 + " (#123)\n\n")
    assert run_msg(tmp_path, "lint").returncode == 0


def test_lint_reports_body_overage(tmp_path: Path) -> None:
    """Report the exact body line and overage.

    Authors need one precise correction instead of recounting the file.
    """
    (tmp_path / "message").write_text("fix: check body\n\n" + "x " * 36 + "x\n")
    result = run_msg(tmp_path, "lint")
    assert result.returncode == 1
    assert "line 3: 73 chars, max 72 (over by 1)" in result.stdout


def test_lint_allows_unicode_and_fenced_code(tmp_path: Path) -> None:
    """Count Unicode characters and exempt fenced code.

    Shell byte counts reject valid non-ASCII text and long code examples.
    """
    (tmp_path / "message").write_text(
        "fix: use café\n\n```\n" + "x" * 100 + "\n```\n"
    )
    assert run_msg(tmp_path, "lint").returncode == 0


def test_fmt_preserves_consecutive_bullets(tmp_path: Path) -> None:
    """Keep each list item and empty marker unchanged.

    Changes lists are edited incrementally, so merging or rewriting
bullets loses scope and creates needless diffs.
    """
    path = tmp_path / "message"
    path.write_text("fix: keep lists\n\n- add linter\n- add skill\n- \n")
    assert run_msg(tmp_path, "fmt").returncode == 0
    assert path.read_text() == "fix: keep lists\n\n- add linter\n- add skill\n- \n"


def test_fmt_preserves_indented_code(tmp_path: Path) -> None:
    """Leave indented code outside lists untouched.

    Code examples use indentation that should not be reflowed as prose.
    """
    path = tmp_path / "message"
    path.write_text("fix: keep code\n\n    command --long value\n")
    assert run_msg(tmp_path, "fmt").returncode == 0
    assert path.read_text() == "fix: keep code\n\n    command --long value\n"


def test_fmt_is_idempotent_and_lintable(tmp_path: Path) -> None:
    """Wrap prose once and leave later runs unchanged.

    Repeated skill use must not create needless message churn.
    """
    path = tmp_path / "message"
    path.write_text("fix: wrap prose\n\nThis paragraph has enough words to require wrapping when the formatter runs.\n")
    assert run_msg(tmp_path, "fmt").returncode == 0
    first = path.read_text()
    assert run_msg(tmp_path, "fmt").returncode == 0
    assert path.read_text() == first
    assert run_msg(tmp_path, "lint").returncode == 0


def test_hook_accepts_generated_subject(tmp_path: Path) -> None:
    """Allow generated Git subjects only in hook mode.

    Fixup and merge commits are created by Git and must not block normal
history maintenance.
    """
    (tmp_path / "message").write_text("fixup! " + "x" * 60 + "\n\n")
    assert run_msg(tmp_path, "lint", "--hook").returncode == 0
    assert run_msg(tmp_path, "lint").returncode == 1


def test_hook_accepts_plain_title(tmp_path: Path) -> None:
    """Allow plain commit subjects in hook mode.

    Other repositories and test fixtures may use short subjects without a
    conventional-commit prefix.
    """
    (tmp_path / "message").write_text("init\n")
    assert run_msg(tmp_path, "lint", "--hook").returncode == 0


def test_plain_title_requires_format_without_hook(tmp_path: Path) -> None:
    """Keep title format rules outside hook mode.

    Full message checks still guide authors toward consistent commit text.
    """
    (tmp_path / "message").write_text("init\n")
    assert run_msg(tmp_path, "lint").returncode == 1


def test_hook_rejects_long_title(tmp_path: Path) -> None:
    """Reject hook titles longer than fifty characters.

    Length limits keep commit subjects readable without requiring a prefix.
    """
    (tmp_path / "message").write_text("x" * 51 + "\n")
    result = run_msg(tmp_path, "lint", "--hook")
    assert result.returncode == 1
    assert "line 1: 51 chars, max 50 (over by 1)" in result.stdout


def test_hook_rejects_long_body_line(tmp_path: Path) -> None:
    """Reject hook body lines longer than seventy-two characters.

    Body limits prevent long lines from hiding important commit context.
    """
    (tmp_path / "message").write_text("init\n\n" + "x " * 36 + "x\n")
    result = run_msg(tmp_path, "lint", "--hook")
    assert result.returncode == 1
    assert "line 3: 73 chars, max 72 (over by 1)" in result.stdout


def test_hook_ignores_comments_and_scissors(tmp_path: Path) -> None:
    """Ignore editor comments and verbose commit diffs in hook mode.

    Git runs commit-msg before removing comments and may append a diff.
    """
    path = tmp_path / "message"
    path.write_text(
        "fix: keep hook usable\n\n"
        "# " + "x" * 100 + "\n"
        "# ------------------------ >8 ------------------------\n"
        "not a message body " + "x" * 100 + "\n"
    )
    assert run_msg(tmp_path, "lint", "--hook").returncode == 0


def test_squash_keeps_overview_only(tmp_path: Path) -> None:
    """Print only the title and Overview for squash commits.

    The commit body should preserve the reviewed reason and change without
    copying sections meant to remain visible on GitHub.
    """
    path = tmp_path / "message"
    path.write_text(
        "fix: keep squash text\n\n"
        "## Overview\n\n"
        "Keep the reviewed reason and change.\n\n"
        "## Changes\n\n- detail\n\n## Testing\n\n- check\n"
    )
    result = run_msg(tmp_path, "squash")
    assert result.returncode == 0
    assert result.stdout == "fix: keep squash text\n\nKeep the reviewed reason and change.\n"
    path.write_text(result.stdout)
    assert run_msg(tmp_path, "lint").returncode == 0


def test_squash_requires_overview(tmp_path: Path) -> None:
    """Reject squash messages without a non-empty Overview.

    A missing summary would create a commit body that cannot explain the
    change to readers of the default branch.
    """
    (tmp_path / "message").write_text("fix: missing summary\n\n## Changes\n\n- detail\n")
    result = run_msg(tmp_path, "squash")
    assert result.returncode == 1
    assert "non-empty Overview" in result.stderr


@pytest.mark.local_ai
def test_local_model_writes_lintable_message(tmp_path: Path) -> None:
    """Check a model writing through the same skill a worker receives.

    The local model must use the repository version of git-messages, not
an older installed copy, before its message reaches the linter.
    """
    last_lint = ""
    last_tools: list[str] = []
    prompt = (
        "Load the git-messages skill. Write a commit message for adding a "
        "--hook option to a message linter. Save it with your write tool to "
        "the file `message` in the current directory. Then run the skill's "
        "fmt and lint commands on it."
    )
    for attempt in range(3):
        project = tmp_path / f"project-{attempt}"
        skills = project / ".agents/skills"
        skills.mkdir(parents=True)
        shutil.copytree(ROOT / "ai/skills/git-messages", skills / "git-messages")
        subprocess.run(["git", "init", "-q"], cwd=project, check=True)
        environment = os.environ.copy()
        environment["PWD"] = str(project)
        result = subprocess.run(
            [
                "opencode", "run", "--standalone", "--auto", "--model", "goog/qwen3.8",
                "--format", "json", prompt,
            ],
            cwd=project,
            env=environment,
            capture_output=True,
            text=True,
            timeout=180,
        )
        events = []
        for line in result.stdout.splitlines():
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                continue
        last_tools = [
            event.get("part", {}).get("tool", "")
            for event in events
            if event.get("part", {}).get("tool")
        ]
        message = project / "message"
        if result.returncode != 0:
            last_lint = result.stderr
            continue
        if not message.is_file():
            last_lint = "message not written"
            continue
        checks = []
        for command in ("fmt", "lint"):
            check = subprocess.run(
                [sys.executable, str(MSG), command, str(message)],
                capture_output=True,
                text=True,
            )
            checks.append(check)
            if command == "lint":
                last_lint = check.stdout + check.stderr
        if all(check.returncode == 0 for check in checks):
            return
    tools = ", ".join(last_tools) or "none"
    assert False, (
        f"local model failed after three attempts: {last_lint or 'message not written'}; "
        f"tools: {tools}"
    )
