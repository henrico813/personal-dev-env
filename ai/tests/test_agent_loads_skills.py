"""Verify OpenCode commands load their requested skills."""

import json
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
COMMAND_SKILLS = [
    ("create_plan", "create-plan"),
    ("review_plan", "review-plan"),
    ("implement_plan", "implement-plan"),
    ("cleanup_plan", "cleanup-plan"),
    ("design_doc", "design-doc"),
    ("research_codebase", "research-codebase"),
]


class Agent:
    """Run an OpenCode command in the temporary project."""

    def __init__(self, project: Path) -> None:
        self.project = project

    def run(self, command: str, skill: str) -> list[dict[str, object]]:
        message = (
            f"/{command}\n"
            f"Do not perform the task. Only load the {skill} skill and "
            "reply with its first Markdown heading."
        )
        result = subprocess.run(
            [
                "opencode",
                "run",
                "--standalone",
                "--auto",
                "--model",
                "goog/qwen3.8",
                "--format",
                "json",
                message,
            ],
            cwd=self.project,
            capture_output=True,
            text=True,
            timeout=180,
        )
        assert result.returncode == 0, (
            f"OpenCode command {command} failed:\n{result.stderr}"
        )
        return [json.loads(line) for line in result.stdout.splitlines()]


@pytest.fixture(scope="session")
def git_project(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Create a Git project containing the command and skill files."""
    project = tmp_path_factory.mktemp("opencode-skills")
    commands = project / ".opencode" / "commands"
    skills = project / ".agents" / "skills"
    commands.mkdir(parents=True)
    skills.mkdir(parents=True)

    for command, _ in COMMAND_SKILLS:
        shutil.copy(ROOT / "ai/opencode/commands" / f"{command}.md", commands)
    for _, skill in COMMAND_SKILLS:
        source = ROOT / "ai/skills" / skill
        shutil.copytree(source, skills / skill)

    subprocess.run(["git", "init", "-q"], cwd=project, check=True)
    return project


@pytest.mark.local_ai
class TestAgentLoadsSkills:
    """Verify commands load their requested skills."""

    @pytest.mark.parametrize("command,skill", COMMAND_SKILLS)
    def test_wrapper_loads_expected_skill(
        self, git_project: Path, command: str, skill: str
    ) -> None:
        agent = Agent(git_project)
        for _ in range(3):
            events = agent.run(command, skill)
            if any(
                event.get("type") == "tool_use"
                and event.get("part", {}).get("tool") == "skill"
                and event.get("part", {})
                .get("state", {})
                .get("input", {})
                .get("id")
                == skill
                for event in events
            ):
                return

        event_context = [
            (event.get("type"), event.get("part", {}).get("tool"))
            for event in events
        ]
        assert False, (
            f"OpenCode command {command} did not load skill {skill} after "
            f"three attempts; final event context: {event_context}"
        )
