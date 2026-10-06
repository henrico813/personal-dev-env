"""Check planner commands embedded in the shared workflow skills.

OpenCode commands are thin wrappers that load these skills, so the planner
command text now lives in the shared skills. Go tests prove that the planner
works, but not that skill text still matches it; the manual eval spends model
tokens, so it rarely runs. This suite catches an
unquoted ``implementation[1].file_changes[1]`` selector that zsh rejects with
"no matches found", and skills that retain ``planner dod``,
``planner implementation``, or ``planner verification`` subcommands.
"""

import json
import os
import re
import subprocess
import tempfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
PLANNER_COMMAND = re.compile(r"`(planner\b[^`]*)`", re.DOTALL)
TARGET_COMMAND = re.compile(r"^planner (inspect|patch|check)\b")


def subcommand(command: str) -> str:
    """Return the second whitespace-separated command word.

    Args:
        command: Command text to split.

    Returns:
        The second word, or an empty string when absent.
    """
    words = command.split()
    return words[1] if len(words) > 1 else ""


def extract_commands(path: Path) -> list[str]:
    """Extract planner commands and collapse wrapped whitespace.

    Args:
        path: Skill file to read.

    Returns:
        Backtick-quoted planner commands with collapsed whitespace.
    """
    return [
        " ".join(match.group(1).split())
        for match in PLANNER_COMMAND.finditer(path.read_text())
    ]


def workflow_skill_files() -> list[Path]:
    """List the shared workflow skills that carry planner command text.

    The OpenCode commands are thin wrappers around these skills, so the
    skills are the prompts the agents copy planner commands from.

    Returns:
        Sorted skill paths under ``ai/skills/``.
    """
    return sorted((ROOT / "ai/skills").glob("*/SKILL.md"))


def prompt_kind(path: Path) -> str | None:
    """Identify the workflow kind for a planning skill.

    Args:
        path: Skill file to classify.

    Returns:
        ``"create"`` or ``"implement"``, or ``None`` for other skills.
    """
    if path.parts[-2:] == ("create-plan", "SKILL.md"):
        return "create"
    if path.parts[-2:] == ("implement-plan", "SKILL.md"):
        return "implement"
    return None


def prompt_id(path: Path) -> str:
    """Build the test ID for a skill path.

    Args:
        path: Absolute skill path inside the repository.

    Returns:
        A short shared-skill test ID.
    """
    return path.parent.name


@pytest.fixture(scope="session")
def planner_environment(
    tmp_path_factory: pytest.TempPathFactory,
) -> dict[str, str]:
    """Build the planner and put it first on PATH.

    Args:
        tmp_path_factory: Factory for the isolated build directory.

    Returns:
        The process environment with the built planner first on PATH.

    Raises:
        AssertionError: If building the planner fails.
    """
    build_root = tmp_path_factory.mktemp("planner")
    planner = build_root / "planner"
    result = subprocess.run(
        [
            "go",
            "build",
            "-C",
            str(ROOT / "planner"),
            "-o",
            str(planner),
            "./main",
        ],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"go build failed:\n{result.stderr}"
    environment = os.environ.copy()
    environment["PATH"] = f"{build_root}{os.pathsep}{environment['PATH']}"
    return environment


PLAN_PROMPTS = [
    pytest.param(path, kind, id=prompt_id(path))
    for path in workflow_skill_files()
    if (kind := prompt_kind(path)) is not None
]

def run_checked(
    args: list[str], *, cwd: Path | None = None, env: dict[str, str]
) -> str:
    """Run a command and return its standard output.

    Args:
        args: Command and arguments to execute.
        cwd: Working directory for the command, if any.
        env: Environment for the subprocess.

    Returns:
        Captured standard output.

    Raises:
        AssertionError: If the command exits with a nonzero status.
    """
    result = subprocess.run(
        args, cwd=cwd, env=env, capture_output=True, text=True
    )
    assert result.returncode == 0, (
        f"command failed: {' '.join(args)}\n{result.stderr}"
    )
    return result.stdout


def new_fixture(directory: Path, env: dict[str, str]) -> None:
    """Create and commit a minimal Go repository.

    Args:
        directory: Directory for the fixture repository.
        env: Environment for the git commands.
    """
    directory.mkdir(parents=True, exist_ok=True)
    run_checked(["git", "init", "-q"], cwd=directory, env=env)
    (directory / "go.mod").write_text(
        "module example.com/prompt-eval\n\ngo 1.21\n"
    )
    (directory / "main.go").write_text("package main\n\nfunc main() {}\n")
    run_checked(["git", "add", "-A"], cwd=directory, env=env)
    run_checked(
        [
            "git",
            "-c",
            "user.name=eval",
            "-c",
            "user.email=eval@example.com",
            "commit",
            "-qm",
            "init",
        ],
        cwd=directory,
        env=env,
    )


def seed_plan(
    repo: Path, plan: Path, tmp_root: Path, env: dict[str, str]
) -> str:
    """Create a plan whose first change edits main.go.

    Args:
        repo: Fixture repository containing the source file.
        plan: Plan file to create and update.
        tmp_root: Directory for scratch files.
        env: Environment for the planner commands.

    Returns:
        The base commit SHA used for planner operations.

    Raises:
        AssertionError: If the planner scaffold lacks an expected placeholder.
    """
    run_checked(["planner", "new", str(plan)], env=env)
    scaffold = "`path/to/file`"
    contents = plan.read_text()
    assert scaffold in contents, (
        "planner new scaffold changed; update seed_plan's placeholder"
    )
    base = run_checked(["git", "rev-parse", "HEAD"], cwd=repo, env=env).strip()
    current_state = (
        "<current behavior with file:line refs -- required, non-empty, "
        "max 250 chars>"
    )
    assert current_state in contents, (
        "planner new scaffold changed; update seed_plan's current-state "
        "placeholder"
    )
    contents = contents.replace(scaffold, "`main.go`")
    plan.write_text(contents.replace(current_state, f"Base commit: {base}"))
    scratch = Path(tempfile.mkdtemp(dir=tmp_root, prefix="seed.")) / "source"
    output = run_checked(
        [
            "planner",
            "inspect",
            str(plan),
            "--target",
            "implementation[1].file_changes[1]",
            "--repo",
            str(repo),
            "--base-commit",
            base,
            "--before",
            "--code-out",
            str(scratch),
        ],
        env=env,
    )
    token = json.loads(output)["edit_expect"]
    scratch.write_text(scratch.read_text() + "\nvar seeded = true\n")
    run_checked(
        [
            "planner",
            "patch",
            str(plan),
            "--target",
            "implementation[1].file_changes[1]",
            "--expect",
            token,
            "--repo",
            str(repo),
            "--base-commit",
            base,
            "--after-file",
            str(scratch),
        ],
        env=env,
    )
    return base


@pytest.mark.parametrize(("skill", "kind"), PLAN_PROMPTS)
def test_skill_commands_run_in_zsh(
    skill: Path,
    kind: str,
    tmp_path: Path,
    planner_environment: dict[str, str],
) -> None:
    """Run skill planner commands under zsh.

    Only running commands in zsh exposes glob failures such as an unquoted
    ``implementation[1].file_changes[1]`` selector failing with "no matches
    found".
    """
    fixture = tmp_path / "fixture"
    (fixture / "plans").mkdir(parents=True)
    new_fixture(fixture, planner_environment)
    plan = fixture / "plans" / "prompt.md"
    base = seed_plan(fixture, plan, tmp_path, planner_environment)
    selector = "implementation[1].file_changes[1]"
    token = ""
    scratch = ""
    ran: set[str] = set()

    for original in extract_commands(skill):
        if not TARGET_COMMAND.match(original):
            continue
        if "--repo <repo>" not in original or (
            subcommand(original) != "check"
            and "--base-commit <commit>" not in original
        ):
            continue
        ran.add(subcommand(original))
        if original.startswith("planner inspect "):
            scratch = str(
                Path(tempfile.mkdtemp(dir=tmp_path, prefix="run.")) / "source"
            )
        command = original
        replacements = {
            "<plan.md>": str(plan),
            "<output.md>": str(plan),
            "<selector>": selector,
            "<repo>": str(fixture),
            "<commit>": base,
            "<new-scratch-file>": scratch,
            "<scratch-file>": scratch,
            "<edit_expect>": token,
        }
        for placeholder, value in replacements.items():
            command = command.replace(placeholder, value)
        if command.startswith("planner patch "):
            with Path(scratch).open("a") as source:
                source.write("\n// prompt-check edit\n")
        result = subprocess.run(
            ["zsh", "-c", command],
            env=planner_environment,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, (
            f"skill {skill} command failed: {command}\n{result.stderr}"
        )
        if command.startswith("planner inspect "):
            token = json.loads(result.stdout)["edit_expect"]

    assert {"inspect", "patch", "check"} <= ran, (
        f"skill {skill} command text no longer matched the expected "
        f"placeholders; ran {sorted(ran)}"
    )
