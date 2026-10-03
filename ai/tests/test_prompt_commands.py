"""Check planner commands embedded in the planning prompts.

Agents copy planner commands verbatim from the prompts. Go tests prove that
the planner works, but not that prompt text still matches it; the only other
check was a token-spending manual eval that nobody ran. This suite catches an
unquoted ``implementation[1].file_changes[1]`` selector that zsh rejects with
"no matches found", and prompts that retain ``planner dod``,
``planner implementation``, or ``planner verification`` subcommands.
"""

import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

import pytest


ROOT = Path(__file__).resolve().parents[2]
ALLOWED_SUBCOMMANDS = {"help", "new", "check", "inspect", "patch"}
PLANNER_COMMAND = re.compile(r"`(planner\b[^`]*)`", re.DOTALL)
TARGET_COMMAND = re.compile(r"^planner (inspect|patch|check)\b")


def extract_commands(path: Path) -> list[str]:
    """Extract and whitespace-normalize backtick-quoted planner commands."""
    return [
        " ".join(match.group(1).split())
        for match in PLANNER_COMMAND.finditer(path.read_text())
    ]


def prompt_files() -> list[Path]:
    return sorted((ROOT / "ai/opencode/commands").glob("*.md")) + sorted(
        (ROOT / "ai/codex/skills").glob("*/SKILL.md")
    )


def prompt_kind(path: Path) -> str | None:
    """Return the workflow kind for prompts that execute plan commands."""
    if path.name == "create_plan.md" or path.parts[-2:] == (
        "create-plan", "SKILL.md"
    ):
        return "create"
    if path.name == "implement_plan.md" or path.parts[-2:] == (
        "implement-plan", "SKILL.md"
    ):
        return "implement"
    return None


def prompt_id(path: Path) -> str:
    relative = path.relative_to(ROOT)
    if relative.parts[1] == "opencode":
        return f"opencode/{relative.name}"
    return f"codex/{relative.parts[-2]}"


@pytest.fixture(scope="session")
def planner_environment(tmp_path_factory: pytest.TempPathFactory) -> dict[str, str]:
    """Build the checkout's planner and put it first on PATH."""
    build_root = tmp_path_factory.mktemp("planner")
    planner = build_root / "planner"
    result = subprocess.run(
        ["go", "build", "-C", str(ROOT / "planner"), "-o", str(planner), "./main"],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"go build failed:\n{result.stderr}"
    environment = os.environ.copy()
    environment["PATH"] = f"{build_root}{os.pathsep}{environment['PATH']}"
    return environment


PROMPTS = [pytest.param(path, id=prompt_id(path)) for path in prompt_files()]
PLAN_PROMPTS = [
    pytest.param(path, id=prompt_id(path))
    for path in prompt_files()
    if prompt_kind(path)
]


@pytest.mark.parametrize("prompt", PROMPTS)
def test_prompt_uses_only_existing_subcommands(prompt: Path) -> None:
    # Guards removed planner dod/implementation/verification commands left in prompts.
    for command in extract_commands(prompt):
        words = command.split()
        subcommand = words[1] if len(words) > 1 else ""
        assert subcommand in ALLOWED_SUBCOMMANDS, (
            f"prompt {prompt} uses unknown planner subcommand "
            f"'{subcommand}': {command}"
        )


@pytest.mark.parametrize("prompt", PLAN_PROMPTS)
def test_planning_prompt_keeps_required_commands(prompt: Path) -> None:
    # Guards a prompt edit dropping planner check so agents stop validating plans.
    required = {
        "create": {"new", "inspect", "patch", "check"},
        "implement": {"inspect", "patch", "check"},
    }
    kind = prompt_kind(prompt)
    present = {
        words[1]
        for command in extract_commands(prompt)
        for words in [command.split()]
        if len(words) > 1
    }
    assert required[kind] <= present, (
        f"prompt {prompt} is missing planner commands "
        f"{sorted(required[kind] - present)}"
    )


def run_checked(
    args: list[str], *, cwd: Path | None = None, env: dict[str, str]
) -> str:
    result = subprocess.run(
        args, cwd=cwd, env=env, capture_output=True, text=True
    )
    assert result.returncode == 0, (
        f"command failed: {' '.join(args)}\n{result.stderr}"
    )
    return result.stdout


def new_fixture(directory: Path, env: dict[str, str]) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    run_checked(["git", "init", "-q"], cwd=directory, env=env)
    (directory / "go.mod").write_text("module example.com/prompt-eval\n\ngo 1.21\n")
    (directory / "main.go").write_text("package main\n\nfunc main() {}\n")
    run_checked(["git", "add", "-A"], cwd=directory, env=env)
    run_checked(
        [
            "git", "-c", "user.name=eval", "-c", "user.email=eval@example.com",
            "commit", "-qm", "init",
        ],
        cwd=directory,
        env=env,
    )


def seed_plan(
    repo: Path, plan: Path, tmp_root: Path, env: dict[str, str]
) -> str:
    run_checked(["planner", "new", str(plan)], env=env)
    plan.write_text(plan.read_text().replace("`path/to/file`", "`main.go`"))
    base = run_checked(["git", "rev-parse", "HEAD"], cwd=repo, env=env).strip()
    scratch = Path(tempfile.mkdtemp(dir=tmp_root, prefix="seed.")) / "source"
    output = run_checked(
        [
            "planner", "inspect", str(plan), "--target",
            "implementation[1].file_changes[1]", "--repo", str(repo),
            "--base-commit", base, "--before", "--code-out", str(scratch),
        ],
        env=env,
    )
    token = json.loads(output)["edit_expect"]
    scratch.write_text(scratch.read_text() + "\nvar seeded = true\n")
    run_checked(
        [
            "planner", "patch", str(plan), "--target",
            "implementation[1].file_changes[1]", "--expect", token,
            "--repo", str(repo), "--base-commit", base,
            "--after-file", str(scratch),
        ],
        env=env,
    )
    return base


@pytest.mark.parametrize("prompt", PLAN_PROMPTS)
def test_prompt_commands_run_in_zsh(
    prompt: Path, tmp_path: Path, planner_environment: dict[str, str]
) -> None:
    # Guards unquoted implementation[1].file_changes[1] failing with zsh's "no matches found".
    fixture = tmp_path / "fixture"
    (fixture / "plans").mkdir(parents=True)
    new_fixture(fixture, planner_environment)
    plan = fixture / "plans" / "prompt.md"
    base = seed_plan(fixture, plan, tmp_path, planner_environment)
    selector = "implementation[1].file_changes[1]"
    token = ""
    scratch = ""

    for original in extract_commands(prompt):
        if not TARGET_COMMAND.match(original):
            continue
        if "--repo <repo>" not in original or "--base-commit <commit>" not in original:
            continue
        if original.startswith("planner inspect "):
            scratch = str(Path(tempfile.mkdtemp(dir=tmp_path, prefix="run.")) / "source")
        command = original
        replacements = {
            "<plan.md>": str(plan), "<output.md>": str(plan),
            "<selector>": selector, "<repo>": str(fixture),
            "<commit>": base, "<new-scratch-file>": scratch,
            "<scratch-file>": scratch, "<edit_expect>": token,
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
            f"prompt {prompt} command failed: {command}\n{result.stderr}"
        )
        if command.startswith("planner inspect "):
            token = json.loads(result.stdout)["edit_expect"]
