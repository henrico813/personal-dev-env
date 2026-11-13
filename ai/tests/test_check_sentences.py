"""Behavior tests for the sentence checker of the plain-direct-english skill.

Each test runs check_sentences.py on a draft, as an agent does before
it sends a message or opens a pull request.
"""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/plain-direct-english/scripts/check_sentences.py"


def check(args: list[str], draft: str = "") -> subprocess.CompletedProcess:
    """Run the checker with args, giving it draft on stdin.

    Args:
        args: The command-line arguments after the script.
        draft: The text on stdin.

    Returns:
        The finished script.
    """
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        input=draft,
        capture_output=True,
        text=True,
    )


class TestCheckSentences:
    """The checker reports AI filler in prose and nothing else."""

    def test_reports_each_rule_by_line(self) -> None:
        """Each filler word, banned word, hedge, offer and emoji is reported.

        The agent fixes the draft from these lines alone, so a missed
        rule or a wrong line number lets the user read the filler they
        asked never to see again.
        """
        draft = (
            "A robust fix that leverages the cache.\n"
            "It reads the canonical config.\n"
            "This should probably work, I think.\n"
            "Let me know if you want changes!\n"
            "Merged \u2705\n"
            "We ensure the build works.\n"
            "Shipped \U0001f680\n"
        )

        result = check([], draft)

        assert result.returncode == 1
        assert result.stdout == (
            'line 1: filler word "robust"\n'
            'line 1: filler word "leverages"\n'
            'line 2: banned word "canonical"\n'
            'line 3: hedge "probably"\n'
            'line 3: hedge "I think"\n'
            'line 4: closing offer or praise "Let me know"\n'
            "line 5: emoji\n"
            'line 6: filler word "ensure"\n'
            "line 7: emoji\n"
        )

    def test_skips_code_and_word_parts(self) -> None:
        """Words in code blocks, in backticks or inside longer words pass.

        A command or a name may need one of these words, and
        "contractor" is not a banned word. False findings teach
        agents to ignore the checker.
        """
        draft = (
            "The contractor ran `make robust` once.\n"
            "```\n"
            "tool ensure --maybe\n"
            "```\n"
        )

        result = check([], draft)

        assert result.returncode == 0
        assert result.stdout == ""

    def test_missing_file_exits_two(self, tmp_path: Path) -> None:
        """A file that cannot be read exits 2 and names the file.

        Exit 0 would make a mistyped path look like a clean draft,
        and exit 1 like a draft with problems.
        """
        result = check([str(tmp_path / "gone.md")])

        assert result.returncode == 2
        assert "gone.md" in result.stderr
