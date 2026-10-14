"""Behavior tests for the comment checker in the code-documentation skill."""

import runpy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "ai/skills/code-documentation/scripts/check_comments.py"
go_comments = runpy.run_path(str(CHECKER))["go_comments"]


class TestCommentNames:
    """Name each comment after the code below it."""

    def test_method_comment_takes_the_method_name(self) -> None:
        """Name a method's comment ``Recv.Method``, not ``func``.

        A method declaration starts with its receiver, so the plain
        declaration pattern would name its comment ``func``. Filler
        counts only in a sentence about the code, such as one that
        starts with its name, so "Total facilitates ..." would pass.
        """
        source = "// Total adds up the prices.\nfunc (c *Cart) Total() int {\n"

        comments = go_comments(source, "cart.go")

        assert [c.name for c in comments] == ["Cart.Total"]
