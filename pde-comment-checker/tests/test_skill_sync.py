"""Keep the checker in step with the code-documentation skill."""

import re
import subprocess
import sys
from pathlib import Path

from comment_checker.cli import RULES
from comment_checker.python_rules import RULES as PYTHON_RULES

ROOT = Path(__file__).resolve().parents[2]
SKILL = ROOT / "ai/skills/code-documentation/SKILL.md"


class TestSkillText:
    """Keep the checker quoting the skill, which owns each rule sentence."""

    def test_skill_quotes_checker_rules_and_options(
        self, packed_checker: Path
    ) -> None:
        """Fail when the skill and the checker drift apart.

        Each rule may only enforce a sentence the skill states; an
        earlier draft added checks with no rule behind them and was
        rejected. Agents also run the skill's command word for word, so
        an option the checker lacks stops them with a usage error.
        """
        skill = SKILL.read_text()
        rules = {name: rule.sentence for name, rule in RULES.items()}
        rules |= PYTHON_RULES
        shown = "".join(
            subprocess.run(
                [sys.executable, str(packed_checker), *command, "--help"],
                capture_output=True,
                text=True,
                check=True,
            ).stdout
            for command in ([], ["python-rules"])
        )

        flat = " ".join(skill.split())
        unquoted = [rule for rule, quote in rules.items() if quote not in flat]
        named = set(re.findall(r"--[a-z][a-z-]*", skill))
        accepted = set(re.findall(r"--[a-z][a-z-]*", shown))

        assert unquoted == []
        assert named
        assert named - accepted == set()
