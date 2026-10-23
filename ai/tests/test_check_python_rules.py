"""Behavior tests for the Python rule checker."""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "ai/skills/code-documentation/scripts"
CHECKER = SCRIPTS / "check_python_rules.py"


def run_checker(repo: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run the checker on every Python file in ``repo``.

    Args:
        repo: The folder passed as ``--repo``, whose ``.py`` files are
            checked.
        *args: Extra checker options, such as ``--base``.

    Returns:
        The finished process, with output as text.
    """
    paths = sorted(str(p) for p in repo.iterdir() if p.suffix == ".py")
    return subprocess.run(
        [sys.executable, str(CHECKER), "--repo", str(repo), *args, *paths],
        capture_output=True,
        text=True,
    )


def list_found_rules(result: subprocess.CompletedProcess[str]) -> set[str]:
    """Return each finding as ``"<file>:<line> <name>: <rule>"``."""
    return {
        ": ".join(line.split(": ")[:2])
        for line in result.stdout.splitlines()
        if " rule: " not in line
    }


class TestNames:
    """Hold function names to the verb and seven-word rules."""

    def test_names_follow_verb_and_word_rules(self, tmp_path: Path) -> None:
        """Flag bad names, but never names a framework chose.

        Reviewers kept asking for verb-first names of seven words or
        fewer. Names such as ``setUp`` or a fixture named for the value
        it gives a test are not the author's choice, and a checker that
        fails on them teaches agents to ignore it.
        """
        (tmp_path / "shop.py").write_text(
            '''
import pytest


def total_price():
    """Add up the cart."""


def add_the_tax_and_the_fee_to_total():
    """Add both charges."""


def test_total_includes_the_tax_and_the_fee():
    """Check the total.

    It is what the customer pays.
    """


@pytest.fixture
def cart():
    """Give a test an empty cart."""


class Handler:
    """Answer HTTP requests."""

    def do_GET(self):
        """Send the page."""

    def setUp(self):
        """Start clean."""

    @property
    def size(self):
        """Count the items."""


def add_the_tax_and_fee_to_total():
    """Add both charges."""
'''
        )

        result = run_checker(tmp_path)

        assert list_found_rules(result) == {
            "shop.py:5 total_price: verb-first",
            "shop.py:9 add_the_tax_and_the_fee_to_total: seven-words",
            "shop.py:13 test_total_includes_the_tax_and_the_fee: seven-words",
        }
        assert result.returncode == 1

    def test_unknown_verb_reports_without_failing(
        self, tmp_path: Path
    ) -> None:
        """Report a name the verb list lacks, but pass the run.

        The checker's verb list cannot hold every English verb, and a
        real verb missing from it once failed a good name. Agents must
        not rename working code just to please the list.
        """
        (tmp_path / "shop.py").write_text(
            'def total_price():\n    """Add up the cart."""\n'
        )

        result = run_checker(tmp_path)

        assert "verb-first" in result.stdout
        assert "(report only)" in result.stdout
        assert result.returncode == 0


class TestDocstrings:
    """Hold docstrings to the skill's Google-style rules."""

    def test_docstring_rules_flag_each_break(self, tmp_path: Path) -> None:
        """Flag each docstring break and pass a good Google docstring.

        Reviewers found missing docstrings on tests and fixtures, tests
        without a why paragraph, and invented headings such as
        ``Why it matters:``. ``Args:`` and ``Returns:`` are Google
        sections, so they must still pass.
        """
        (tmp_path / "shop.py").write_text(
            '''
def send_order(order):
    """Send an order to the warehouse.

    For example:
        send_order(cart.checkout())

    Args:
        order: The order to send.

    Returns:
        The warehouse's order number.
    """


def cancel_order(order):
    pass


def refund_order(order):
    """Pay back the order
    and mark it refunded."""


def test_refund_pays_back_the_order():
    """Check refunds."""


def test_cancel_frees_the_stock():
    """Check cancelling.

    Why it matters:
        Stock would stay held.
    """


class Cart:
    """Cart holds the items."""
'''
        )

        result = run_checker(tmp_path)

        assert list_found_rules(result) == {
            "shop.py:16 cancel_order: docstring",
            "shop.py:20 refund_order: summary-line",
            "shop.py:25 test_refund_pays_back_the_order: test-why",
            "shop.py:29 test_cancel_frees_the_stock: section-heading",
            "shop.py:37 Cart: starts-with-name",
        }


class TestComments:
    """Hold ``#`` comments to two sentences without colons."""

    def test_comment_rules_skip_code_and_urls(self, tmp_path: Path) -> None:
        """Flag long comments and colons, but not code, URLs or tools.

        Reviewers marked many comments "too long" or "no colons".
        Colons in code spans, URLs and tool comments such as ``noqa``
        are not prose, and the lines of one comment count together.
        """
        (tmp_path / "shop.py").write_text(
            """
# Prices are in cents. Tax is added later.
# Discounts come first.
PRICE = 1
# Note: prices include tax.
TAX = 2
# Runs at 4:00, e.g. before close. Then stops.
CLOSE = 3
# See `dict[str, int]` and https://example.com/tax for the rates.
RATES = {}  # noqa: F401
"""
        )

        result = run_checker(tmp_path)

        assert list_found_rules(result) == {
            "shop.py:2 -: comment-length",
            "shop.py:5 -: comment-colon",
        }


class TestBase:
    """Check only code added or changed since ``--base``."""

    def test_base_limits_findings_to_new_code(self, tmp_path: Path) -> None:
        """Leave older code alone, but check new code and new files.

        The checker runs on old repositories full of code written before
        these rules, and failing on it would bury the new breaks. A
        changed body checks the docstring but not the name. A file
        missing at the base is checked in full, and a renamed file is
        compared with its old path.
        """
        old = tmp_path / "old.py"
        old.write_text(
            '''# Note: in cents.
PRICE = 1


def total():
    pass


def subtotal():
    return 0


def add_the_tax_and_the_fee_to_the_total():
    """Add both charges."""
'''
        )
        (tmp_path / "moved.py").write_text("def discount():\n    pass\n")
        git = ["git", "-C", str(tmp_path), "-c", "user.name=t"]
        git += ["-c", "user.email=t"]
        subprocess.run([*git, "init", "-q"], check=True)
        subprocess.run([*git, "add", "."], check=True)
        subprocess.run([*git, "commit", "-qm", "base"], check=True)
        old.write_text(
            '''# Note: in cents.
PRICE = 1


def total():
    pass


def subtotal():
    return 1


def add_the_tax_and_the_fee_to_the_total(cart):
    """Add both charges."""


def price():
    pass
'''
        )
        (tmp_path / "new.py").write_text("def tax():\n    pass\n")
        subprocess.run([*git, "mv", "moved.py", "renamed.py"], check=True)

        result = run_checker(tmp_path, "--base", "HEAD")

        assert list_found_rules(result) == {
            "new.py:1 tax: docstring",
            "new.py:1 tax: verb-first",
            "old.py:9 subtotal: docstring",
            "old.py:17 price: docstring",
            "old.py:17 price: verb-first",
        }
        assert 'docstring rule: "Give every function' in result.stdout
        assert result.returncode == 1


class TestInput:
    """Stop on files the checker cannot read."""

    def test_invalid_python_exits_with_two(self, tmp_path: Path) -> None:
        """Exit 2 when a file is not valid Python.

        Exit 1 means rule breaks, so a file the checker could not read
        must not look like one, or like a clean pass with exit 0.
        """
        (tmp_path / "shop.py").write_text("def total(:\n")

        result = run_checker(tmp_path)

        assert result.returncode == 2
        assert "shop.py" in result.stderr
