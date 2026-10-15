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
KEY_PREFIX = "KEY-[0-9]+ "
PREFIXED_FORMAT_ERROR = (
    "line 1: title must match [<msg.titlePrefix>]<type>[(scope)][!]: <summary>"
)


def isolated_git(tmp_path: Path, global_config: str = os.devnull) -> dict[str, str]:
    """Return an environment for git that ignores the developer's config.

    Git reads only tmp_path's config and the given global file, so the
    developer's own settings, or a checkout above tmp_path, cannot change
    what msg enforces.

    Args:
        tmp_path: Test directory; git does not search above it.
        global_config: File git reads as global config.

    Returns:
        A copy of the process environment with git isolated.
    """
    return os.environ | {
        "GIT_CONFIG_GLOBAL": global_config,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CEILING_DIRECTORIES": str(tmp_path.parent),
    }


def run_msg(
    tmp_path: Path, *args: str, global_config: str = os.devnull
) -> subprocess.CompletedProcess[str]:
    """Run msg against an isolated message file.

    Args:
        tmp_path: Directory holding ``message``; msg runs from it.
        args: msg subcommand and options.
        global_config: File git reads as global config.

    Returns:
        The finished msg process with captured output.
    """
    return subprocess.run(
        [sys.executable, str(MSG), *args, str(tmp_path / "message")],
        cwd=tmp_path,
        env=isolated_git(tmp_path, global_config),
        capture_output=True,
        text=True,
    )


def init_repo(tmp_path: Path, title_prefix: str | None = None) -> None:
    """Make tmp_path a git checkout, optionally setting msg.titlePrefix.

    Args:
        tmp_path: Directory to initialize.
        title_prefix: Value for msg.titlePrefix, or None to leave it unset.
    """
    environment = isolated_git(tmp_path)
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, env=environment, check=True)
    if title_prefix is not None:
        subprocess.run(
            ["git", "config", "msg.titlePrefix", title_prefix],
            cwd=tmp_path,
            env=environment,
            check=True,
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


class TestTitlePrefix:
    """Let one checkout allow a ticket key before commit titles.

    Some repositories start titles with a key such as KEY-123. Each
    sets msg.titlePrefix in its own git config, so the key is optional,
    counts toward the title limit, and still needs a valid type after it.
    """

    @pytest.mark.parametrize(
        ("args", "title", "code", "output"),
        [
            pytest.param(
                ("lint",),
                "KEY-123 feat: " + "x" * 36 + " (#45)",
                0,
                [],
                id="keyed-squash-subject-at-limit",
            ),
            pytest.param(("lint",), "feat: add a check", 0, [], id="unkeyed-title"),
            pytest.param(
                ("lint",),
                "KEY-123 feat: " + "x" * 37 + " (#45)",
                1,
                ["line 1: 51 chars, max 50 (over by 1)"],
                id="key-counts-toward-limit",
            ),
            pytest.param(
                ("lint", "--hook"),
                "KEY-123 add a check",
                1,
                [PREFIXED_FORMAT_ERROR],
                id="hook-rejects-bad-type",
            ),
            pytest.param(
                ("lint", "--hook"),
                "fixup! KEY-123 add a check",
                0,
                [],
                id="hook-skips-generated-subject",
            ),
        ],
    )
    def test_prefix_allows_key_before_type(
        self,
        tmp_path: Path,
        args: tuple[str, ...],
        title: str,
        code: int,
        output: list[str],
    ) -> None:
        """Accept an optional key and keep the other title rules.

        Changelog tools may read only unkeyed subjects, so the key must
        stay optional. GitHub adds the (#N) suffix on merge, and the key counts
        toward the 50 characters. `git commit --fixup` runs the hook, so
        generated subjects must still pass in a keyed repository.
        """
        init_repo(tmp_path, KEY_PREFIX)
        (tmp_path / "message").write_text(title + "\n")

        result = run_msg(tmp_path, *args)

        assert (result.returncode, result.stdout.splitlines()) == (code, output)

    def test_invalid_prefix_names_setting_and_repo(self, tmp_path: Path) -> None:
        """Fail on a bad pattern and say which setting and repository.

        A typo in one checkout's config must not quietly turn the check
        off or into another rule. "KEY-[" is invalid alone but compiles
        once TITLE_RE follows it, which would hide the mistake.
        """
        init_repo(tmp_path, "KEY-[")
        (tmp_path / "message").write_text("feat: add a check\n")

        result = run_msg(tmp_path, "lint")

        assert result.returncode == 1
        assert result.stdout.startswith(
            f"msg.titlePrefix in {tmp_path}: invalid regex: "
        )

    def test_global_prefix_is_ignored(self, tmp_path: Path) -> None:
        """Ignore a prefix set outside the checkout's own config.

        A global value would loosen the title rule in every repository on
        the machine, and a bad pattern there would break lint in all of
        them. A repository without its own setting keeps today's format
        error.
        """
        global_config = tmp_path / "global.gitconfig"
        global_config.write_text(f'[msg]\n\ttitlePrefix = "{KEY_PREFIX}"\n')
        init_repo(tmp_path)
        (tmp_path / "message").write_text("KEY-123 feat: add a check\n")

        result = run_msg(tmp_path, "lint", global_config=str(global_config))

        assert result.returncode == 1
        assert result.stdout == (
            "line 1: title must match <type>[(scope)][!]: <summary>\n"
        )


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
