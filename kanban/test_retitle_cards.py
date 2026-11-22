"""Tests for giving cards the work types and titles of a list."""

import shlex
import subprocess
from pathlib import Path

import card
import pytest
import retitle_cards

# The first card has no work type yet, like most vault cards, and gets
# a type and a new title. The second gets only a new type.
FIRST_CARD = """# Card retitle list

- ABC-001 (draft) feat: Retry uploads
  -> Retry failed uploads twice
"""
LIST = f"{FIRST_CARD}- ABC-002 (todo) docs: Write the setup guide\n"


@pytest.fixture
def projects(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A Projects folder whose Demo project holds the two cards of LIST."""
    monkeypatch.setattr(card, "STATE_DIR", tmp_path / "state")
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    folder = tmp_path / "Projects" / "Demo"
    folder.mkdir(parents=True)
    (folder / "AGENTS.md").write_text("Project code: `ABC`\n")
    projects = tmp_path / "Projects"
    for title, work_type in [
        ("Retry uploads", "chore"),
        ("Write the setup guide", "feat"),
    ]:
        argv = ["new", "Demo", title, "--type", work_type, "--by", "user"]
        assert card.main(argv, projects) == 0
    first = next(folder.glob("ABC-001 *.md"))
    first.write_text(first.read_text().replace("work_type: chore\n", ""))
    return projects


def write_list(tmp_path: Path, text: str) -> Path:
    """Write a list file outside the Projects folder and return its path."""
    path = tmp_path / "list.md"
    path.write_text(text)
    return path


def read_notes(projects: Path) -> dict[str, str]:
    """Return each note's text in the Demo project by file name."""
    return {path.name: path.read_text() for path in projects.glob("Demo/*")}


def test_main_without_apply_only_prints_changes(
    projects: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """Without --apply, retitle_cards prints each change and writes none.

    The user reads the dry run before any card on the synced vault
    changes, so a dry run that wrote would skip that review.
    """
    before = read_notes(projects)
    capsys.readouterr()

    code = retitle_cards.main(
        [str(write_list(tmp_path, LIST)), "--projects", str(projects)]
    )

    assert code == 0
    assert read_notes(projects) == before
    assert capsys.readouterr().out == (
        "ABC-001 type feat\n"
        "ABC-001 title Retry failed uploads twice\n"
        "ABC-002 type docs\n"
        "2 cards to change\n"
    )


def test_apply_sets_cards_and_undo_restores(
    projects: Path, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """--apply sets each type and title, and its undo.sh puts them back.

    The run changes many cards on a network drive at once. Without a
    working undo, one wrong list would need each card fixed by hand.
    The first card has no type, so the run must set the type before
    the title, which card set refuses on an untyped card.
    """
    before = read_notes(projects)
    capsys.readouterr()
    list_file = write_list(tmp_path, LIST)

    code = retitle_cards.main(
        [str(list_file), "--projects", str(projects), "--apply"]
    )

    assert code == 0
    first = card.find_card(projects, "ABC-001")
    assert (first.work_type, first.title) == (
        "feat",
        "Retry failed uploads twice",
    )
    assert card.find_card(projects, "ABC-002").work_type == "docs"
    undo = shlex.split(capsys.readouterr().out.splitlines()[-1])[-1]
    subprocess.run(["sh", undo], check=True)
    assert read_notes(projects) == before


def test_main_apply_failure_still_prints_undo(
    projects: Path,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A write error on the second card still prints the backup and undo.

    The first card has already changed by then. Without the undo line,
    the user would hunt for the backup folder to put it back.
    """
    before = read_notes(projects)
    capsys.readouterr()
    real_main = card.main

    def fail_second_card(argv: list[str], projects: Path) -> int:
        """Run card, but fail on ABC-002 as a dropped drive would."""
        if argv[1] == "ABC-002":
            raise PermissionError(13, "Permission denied")
        return real_main(argv, projects)

    monkeypatch.setattr(card, "main", fail_second_card)
    list_file = write_list(tmp_path, LIST)

    code = retitle_cards.main(
        [str(list_file), "--projects", str(projects), "--apply"]
    )

    out, err = capsys.readouterr()
    assert code == 1
    assert card.find_card(projects, "ABC-001").work_type == "feat"
    assert "retitle_cards: stopped at ABC-002\n" in err
    undo = shlex.split(out.splitlines()[-1])[-1]
    assert f"backup: {Path(undo).parent}\n" in out
    subprocess.run(["sh", undo], check=True)
    assert read_notes(projects) == before


@pytest.mark.parametrize(
    ("lines", "problem"),
    [
        pytest.param(
            "- ABC-002 (todo) docs: Write the setup guide\n"
            "  -> fix the guide.\n",
            "ABC-002 T6 form: starts with a lower-case letter,"
            " ends with a period",
            id="failing-title",
        ),
        pytest.param(
            "- ABC-009 (draft) fix: Fix the guide\n",
            "no card ABC-009",
            id="unknown-key",
        ),
        pytest.param(
            "- ABC-002 (todo) bug: Write the setup guide\n",
            'ABC-002 "bug" is not a work type',
            id="unknown-type",
        ),
        pytest.param(
            "- ABC-002 (in review) docs: Write the setup guide\n",
            "line 5 is not a card line:"
            " - ABC-002 (in review) docs: Write the setup guide",
            id="not-card-line",
        ),
        pytest.param(
            "\n  -> Write the setup guide\n",
            "line 6: no card line right above",
            id="loose-arrow",
        ),
        pytest.param(
            "  -> Retry uploads twice\n",
            "line 5: a second -> for ABC-001",
            id="second-arrow",
        ),
        pytest.param(
            "- abc-001 (draft) fix: Retry uploads\n",
            "line 5: abc-001 is listed twice",
            id="key-twice",
        ),
    ],
)
def test_main_refuses_list_when_one_fails(
    projects: Path,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    lines: str,
    problem: str,
) -> None:
    """One bad line, title or key refuses the list before any card changes.

    A run that stopped halfway would leave some cards retitled and some
    not. An arrow line read for the wrong card would give one card
    another card's title among a hundred plausible lines.
    """
    before = read_notes(projects)
    capsys.readouterr()
    list_file = write_list(tmp_path, f"{FIRST_CARD}{lines}")

    code = retitle_cards.main(
        [str(list_file), "--projects", str(projects), "--apply"]
    )

    assert code == 1
    assert read_notes(projects) == before
    assert f"{problem}\n" in capsys.readouterr().err
