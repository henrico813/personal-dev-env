"""Check that the shared workflow skills and their OpenCode wrappers line up.

The six workflow skills (create-plan, review-plan, implement-plan,
cleanup-plan, design-doc, research-codebase) live once under
``ai/skills/`` and install to ``~/.agents/skills/``. OpenCode no longer
embeds the workflow prompts; each command is a seven-line wrapper that
loads the matching skill and forwards its arguments, so the wrappers must
keep naming skills that exist.
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

COMMAND_SKILLS = {
    "cleanup_plan.md": "cleanup-plan",
    "create_plan.md": "create-plan",
    "design_doc.md": "design-doc",
    "implement_plan.md": "implement-plan",
    "research_codebase.md": "research-codebase",
    "review_plan.md": "review-plan",
}


def workflow_skill_names() -> list[str]:
    """List the shared workflow skill names in wrapper order.

    Returns:
        The six workflow skill names, one per OpenCode command wrapper.
    """
    return list(COMMAND_SKILLS.values())


def test_shared_workflow_skills_exist() -> None:
    """Require every shared workflow skill to ship its SKILL.md.

    Guards a skill moved or renamed out of ``ai/skills/`` so the mapping to
    ``~/.agents/skills/<name>/`` and the wrapper's skill load would point at
    nothing.
    """
    for name in workflow_skill_names():
        path = ROOT / "ai" / "skills" / name / "SKILL.md"
        assert path.is_file(), f"missing shared workflow skill: {path}"
        contents = path.read_text()
        assert f"name: {name}\n" in contents, (
            f"{path} frontmatter does not name itself {name}"
        )
        assert 'pde-workflow: "true"' in contents, (
            f"{path} lost its workflow-skill marker"
        )


def test_opencode_commands_wrap_shared_skills() -> None:
    """Require each command to be a short wrapper loading its skill.

    Guards a wrapper that regrows the old inline prompt instead of loading
    the shared skill, which would fork the workflow instructions between
    OpenCode and the rest of the tree.
    """
    for command, skill in COMMAND_SKILLS.items():
        path = ROOT / "ai" / "opencode" / "commands" / command
        lines = path.read_text().splitlines()
        assert len(lines) == 7, f"{path} is no longer a seven-line wrapper"
        assert lines[0] == "---" and lines[2] == "---", (
            f"{path} lost its description frontmatter"
        )
        assert f"Load the `{skill}` skill and follow it." in lines[4], (
            f"{path} does not load the {skill} skill"
        )
        assert lines[6] == "$ARGUMENTS", (
            f"{path} no longer forwards its command arguments"
        )
