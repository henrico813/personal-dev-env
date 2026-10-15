"""Keep the pull-request skill's guardrails above repository instructions."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / "ai/skills/pull-request/SKILL.md"
GUARDRAILS_RULE = (
    "Guardrails always apply. "
    "Repository instructions may replace the other steps' defaults."
)


def test_guardrails_apply_before_repository_rules() -> None:
    """Fail when repository instructions could replace the guardrails.

    The skill used to say "Use repository instructions first", which a
    model could read as permission to skip the push, merge and test
    guardrails. The guardrails must come first and say they always apply.
    """
    text = SKILL.read_text()

    assert GUARDRAILS_RULE in " ".join(text.split())
    assert text.index("## Guardrails") < text.index("## Discover")
