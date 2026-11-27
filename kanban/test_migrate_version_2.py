"""Tests for moving cards to the version 2 layout, on fake cards."""

import os
import shlex
import subprocess
from pathlib import Path
from typing import Any

import card
import migrate_version_2
import pytest

REPO = Path(__file__).resolve().parents[1]
RULES_FILE = REPO / "chezmoi/dot_config/pde/names.toml"
# A version 1 card with text in its sections and a section added by hand.
V1_CARD = """---
tags:
  - "#Kanban"
type: card
stage: active
template_version: 1
kind: build
---

# DEMO-001 Retry uploads
---

## Context
---

- Uploads fail on a slow network.

## Acceptance Criteria
---

- [ ] Uploads retry.

## Open Questions
---

## Decisions
---

## Risks
---

- The bucket may be full.

## Log
---

- 2026-11-01 active: by user
"""

V2_CARD = """---
tags:
  - "#Kanban"
type: card
stage: active
template_version: 2
kind: build
---

# DEMO-001 Retry uploads
---

## Context
---

- Uploads fail on a slow network.

## Acceptance Criteria
---

- [ ] Uploads retry.

## Open Questions
---

## Decisions
---

## Risks
---

- The bucket may be full.

## Notes
---

## Log
---

- 2026-11-01 active: by user

## Appendix
---
"""

# A card whose Log comes before its Context.
OUT_OF_ORDER_CARD = """---
type: card
template_version: 1
---

# DEMO-003 Old notes
---

## Log
---

## Context
---
"""

# A card with two Notes sections.
REPEATED_CARD = """---
type: card
template_version: 2
---

# DEMO-004 Two notes
---

## Notes
---

## Notes
---
"""

# A card from before version 1, which migrate_sections.py has not moved.
OLD_CARD = """---
type: card
---

# DEMO-005 Older notes
---

## Why
---

## Outcome
---
"""


@pytest.fixture(autouse=True)
def rules_file(monkeypatch: pytest.MonkeyPatch) -> None:
    """Point card at the repository's rules file for card file names."""
    monkeypatch.setenv("PDE_NAMES_FILE", str(RULES_FILE))


@pytest.fixture
def projects(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A Projects folder with an empty Demo project folder.

    Backups go to a state folder beside it, outside Projects.
    """
    monkeypatch.setattr(card, "STATE_DIR", tmp_path / "state")
    (tmp_path / "Projects" / "Demo").mkdir(parents=True)
    return tmp_path / "Projects"


def write_note(projects: Path, name: str, text: str) -> Path:
    """Write a note into the Demo project folder and return its path."""
    path = projects / "Demo" / name
    path.write_text(text)
    return path


def read_tree(projects: Path) -> dict[str, bytes]:
    """Every file under Projects, by its path relative to Projects."""
    return {
        str(path.relative_to(projects)): path.read_bytes()
        for path in projects.rglob("*")
        if path.is_file()
    }


def write_two_old_cards(projects: Path) -> None:
    """Write two version 1 cards, DEMO-001 and DEMO-002."""
    write_note(projects, "DEMO-001 Retry uploads.md", V1_CARD)
    write_note(
        projects,
        "DEMO-002 Retry uploads.md",
        V1_CARD.replace("DEMO-001", "DEMO-002"),
    )


def read_undo_path(out: str) -> str:
    """Return the undo.sh path from the script's last output line."""
    return shlex.split(out.splitlines()[-1])[-1]


def edit_before_write(monkeypatch: pytest.MonkeyPatch, note: Path) -> None:
    """Edit one note just before the script writes it, as a user might.

    The real write_safely then refuses that write, since the note's
    modified time moved on.
    """
    real = card.write_safely

    def write(path: Path, *args: Any) -> bool:
        """Edit the watched note first, then call the real write_safely."""
        if path == note:
            with path.open("a") as handle:
                handle.write("edited\n")
            # An append in the same clock tick can leave the modified time
            # unchanged.
            stat = path.stat()
            os.utime(path, ns=(stat.st_atime_ns, stat.st_mtime_ns + 10**9))
        return real(path, *args)

    monkeypatch.setattr(card, "write_safely", write)


class TestMigrateText:
    """A card's text moves to version 2 with its words kept."""

    def test_migrate_text_adds_version_2_parts(self) -> None:
        """A version 1 card gets Notes, Appendix and version 2.

        The migration runs on cards people wrote by hand. Lost or moved
        words or a section in the wrong place would damage notes that
        sync copies everywhere.
        """
        migrated = migrate_version_2.migrate_text(V1_CARD)

        assert migrated == V2_CARD

    def test_migrate_text_leaves_version_2_card(self) -> None:
        """A complete version 2 card comes back byte for byte.

        The user reruns the script after a stop, so it meets cards it
        already moved. A doubled section or any other change there would
        rewrite and back up every card again.
        """
        assert migrate_version_2.migrate_text(V2_CARD) == V2_CARD


class TestDryRun:
    """Without --apply the script lists cards and writes nothing."""

    def test_dry_run_lists_cards_writes_nothing(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """The dry run names each card to migrate and changes no file.

        The user reviews this list before running --apply. A card
        already at version 2 must not be on it, and a write here would
        change the vault before the user agreed.
        """
        write_note(projects, "DEMO-001 Retry uploads.md", V1_CARD)
        write_note(projects, "DEMO-002 Retry uploads.md", V2_CARD)
        before = read_tree(projects)

        code = migrate_version_2.main([], projects)

        assert code == 0
        assert capsys.readouterr().out == (
            "migrate DEMO-001\n1 of 2 cards to migrate, 0 skipped\n"
        )
        assert read_tree(projects) == before

    def test_old_sections_card_skipped_exits_1(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """A card with the old Why and Outcome sections is skipped, exit 1.

        Adding sections to it now would give it two Acceptance Criteria
        sections once migrate_sections.py renames Outcome. The message
        tells the user to run that script first.
        """
        write_note(projects, "DEMO-001 Retry uploads.md", V1_CARD)
        write_note(projects, "DEMO-005 Older notes.md", OLD_CARD)

        code = migrate_version_2.main([], projects)

        assert code == 1
        assert capsys.readouterr().out == (
            "migrate DEMO-001\n"
            "skip DEMO-005: old Why or Outcome section;"
            " run migrate_sections.py first\n"
            "1 of 2 cards to migrate, 1 skipped\n"
        )

    def test_misordered_card_skipped_exits_1(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """A card with a section out of order or repeated is skipped, exit 1.

        Adding sections to such a card would put Notes or the Appendix
        in the wrong place. The user must see which card to fix by hand,
        and the other cards must still be listed.
        """
        write_note(projects, "DEMO-001 Retry uploads.md", V1_CARD)
        write_note(projects, "DEMO-003 Old notes.md", OUT_OF_ORDER_CARD)
        write_note(projects, "DEMO-004 Two notes.md", REPEATED_CARD)

        code = migrate_version_2.main([], projects)

        assert code == 1
        assert capsys.readouterr().out == (
            "migrate DEMO-001\n"
            "skip DEMO-003: sections out of order or repeated; fix by hand\n"
            "skip DEMO-004: sections out of order or repeated; fix by hand\n"
            "1 of 3 cards to migrate, 2 skipped\n"
        )


class TestApply:
    """--apply --user writes the cards, with a backup and undo.sh."""

    def test_apply_without_user_writes_nothing(self, projects: Path) -> None:
        """--apply alone exits 1 and changes no card.

        Only the user runs the migration on the vault, after reviewing
        the dry run. An agent that passes --apply by mistake must not
        change every card.
        """
        write_two_old_cards(projects)
        before = read_tree(projects)

        code = migrate_version_2.main(["--apply"], projects)

        assert code == 1
        assert read_tree(projects) == before

    def test_apply_writes_version_2_to_cards(self, projects: Path) -> None:
        """--apply --user writes the version 2 text to every old card.

        This is the write the user runs the script for. A card left at
        version 1 would have no Notes or Appendix section, though the run
        said it succeeded.
        """
        write_two_old_cards(projects)
        demo = projects / "Demo"

        code = migrate_version_2.main(["--apply", "--user"], projects)

        assert code == 0
        assert (demo / "DEMO-001 Retry uploads.md").read_text() == V2_CARD
        assert (demo / "DEMO-002 Retry uploads.md").read_text() == (
            V2_CARD.replace("DEMO-001", "DEMO-002")
        )

    def test_undo_script_restores_every_card(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """The printed undo.sh puts every written card back as it was.

        It is the user's way back from a migration they do not want, so
        it must restore each card's bytes and modified time from the
        backup.
        """
        write_two_old_cards(projects)
        cards = sorted((projects / "Demo").iterdir())
        for path in cards:
            # A past modified time, which undo must put back.
            os.utime(path, ns=(10**18, 10**18))
        before = read_tree(projects)
        migrate_version_2.main(["--apply", "--user"], projects)

        undo = read_undo_path(capsys.readouterr().out)
        subprocess.run(["sh", undo], check=True)

        assert read_tree(projects) == before
        assert [path.stat().st_mtime_ns for path in cards] == [10**18] * 2

    def test_rerun_keeps_first_undo_and_edits(
        self,
        projects: Path,
        monkeypatch: pytest.MonkeyPatch,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """A rerun after a stop keeps the first undo.sh and the user's edit.

        The script stops when the user saves a card during the run, and
        exits 1 so the user knows to run it again. Neither run nor either
        undo.sh may drop that save. If the rerun wrote over the first
        run's backup or undo.sh, the cards the first run wrote could no
        longer be put back.
        """
        write_two_old_cards(projects)
        before = read_tree(projects)
        second = "Demo/DEMO-002 Retry uploads.md"
        real_write = card.write_safely
        # The first run stops at the card edited during it.
        edit_before_write(monkeypatch, projects / second)
        first_code = migrate_version_2.main(["--apply", "--user"], projects)
        first_undo = read_undo_path(capsys.readouterr().out)
        monkeypatch.setattr(card, "write_safely", real_write)

        # The rerun writes the rest, then both undo scripts run.
        migrate_version_2.main(["--apply", "--user"], projects)
        second_undo = read_undo_path(capsys.readouterr().out)
        subprocess.run(["sh", second_undo], check=True)
        subprocess.run(["sh", first_undo], check=True)

        assert first_code == 1
        assert read_tree(projects) == {
            **before,
            second: before[second] + b"edited\n",
        }
