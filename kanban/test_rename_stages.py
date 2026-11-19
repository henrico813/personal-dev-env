"""Tests for renaming stage words on cards and checking the result."""

import os
import subprocess
from datetime import datetime
from pathlib import Path
from types import SimpleNamespace

import pytest

import card
import check_stage_rename
import rename_stages

PAIRS = ["--pair", "ready=todo", "--pair", "waiting-on-others=blocked"]
NAME = "PDEV-KB-001 Some work.md"

# A card in an old stage, with Log moves to two old stages. Its Context
# bullet and Log note are not moves, so they keep their words.
OLD_CARD = """---
stage: waiting-on-others
stage_date: 2026-11-02
---

# PDEV-KB-001 Some work
---

## Context
---

- 2026-11-01 ready: by hand

## Log
---

- 2026-11-01 ready: by user
- 2026-11-01 note (hub): ready to start
- 2026-11-01 active: by hub
- 2026-11-02 waiting-on-others: by hub. reviewers
"""

NEW_CARD = """---
stage: blocked
stage_date: 2026-11-02
---

# PDEV-KB-001 Some work
---

## Context
---

- 2026-11-01 ready: by hand

## Log
---

- 2026-11-01 todo: by user
- 2026-11-01 note (hub): ready to start
- 2026-11-01 active: by hub
- 2026-11-02 blocked: by hub. reviewers
"""


@pytest.fixture
def projects(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A Projects folder holding one card in an old stage."""
    monkeypatch.setattr(card, "STATE_DIR", tmp_path / "state")
    folder = tmp_path / "My Vault" / "Projects" / "DevEnv"
    folder.mkdir(parents=True)
    (folder / NAME).write_text(OLD_CARD)
    return tmp_path / "My Vault" / "Projects"


class TestRenameStages:
    """Old stage words on cards become new ones, after a backup."""

    def test_rename_changes_words_restore_undoes_it(
        self, projects: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """The stage and Log lines get new words; the restore line undoes it.

        The rename writes cards on a synced drive. A missed line leaves an
        old stage word behind, a renamed bullet that is not a move changes
        the user's notes, and a restore line that does not work leaves no
        way back.
        """
        path = projects / "DevEnv" / NAME

        code = rename_stages.main(["--projects", str(projects), *PAIRS])

        assert code == 0
        assert path.read_text() == NEW_CARD
        out = capsys.readouterr().out.splitlines()
        restore = next(line for line in out if line.startswith("restore: "))
        subprocess.run(
            restore.removeprefix("restore: "), shell=True, check=True
        )
        assert path.read_text() == OLD_CARD

    def test_rename_again_changes_nothing(
        self, projects: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """A second run writes no card and makes no second backup.

        The user may run it twice, such as after a sync. A second
        backup would hold renamed cards, so its restore line would not
        bring the old words back.
        """
        rename_stages.main(["--projects", str(projects), *PAIRS])
        capsys.readouterr()

        code = rename_stages.main(["--projects", str(projects), *PAIRS])

        assert code == 0
        assert capsys.readouterr().out == "0 cards hold an old stage word\n"
        assert len(list(card.STATE_DIR.iterdir())) == 1

    def test_rerun_same_second_fails_keeps_backup(
        self, projects: Path, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """A run in the same second as the last stops before it writes.

        Both runs would name the same backup folder. If the second wrote
        its copies there, the restore line would lose the first run's
        text of a card an old ``card`` moved back to an old stage.
        """
        path = projects / "DevEnv" / NAME
        moved_back = NEW_CARD.replace("stage: blocked", "stage: ready")
        clock = SimpleNamespace(now=lambda: datetime(2026, 11, 19, 12))
        monkeypatch.setattr(rename_stages, "datetime", clock)
        rename_stages.main(["--projects", str(projects), *PAIRS])
        path.write_text(moved_back)

        code = rename_stages.main(["--projects", str(projects), *PAIRS])

        assert code == 1
        assert path.read_text() == moved_back
        [backup] = card.STATE_DIR.iterdir()
        assert (backup / "DevEnv" / NAME).read_text() == OLD_CARD

    def test_concurrent_edit_exits_one_survives_restore(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """A card edited during the run keeps its edit after a restore.

        The restore line runs first when a real run fails. If the
        backup kept the skipped card's copy, the restore would wipe the
        edit that made the run skip it.
        """
        path = projects / "DevEnv" / NAME
        edit = "- 2026-11-03 note (user): edited during the run\n"
        write_safely = card.write_safely

        def edit_then_write(
            note: Path, key: str, text: str, mtime_ns: int
        ) -> bool:
            """Append an edit to the card, as a hand edit would, then write."""
            with note.open("a") as file:
                file.write(edit)
            # A coarse clock could leave the mtime as it was read.
            os.utime(note, ns=(mtime_ns + 1, mtime_ns + 1))
            return write_safely(note, key, text, mtime_ns)

        monkeypatch.setattr(card, "write_safely", edit_then_write)

        code = rename_stages.main(["--projects", str(projects), *PAIRS])

        assert code == 1
        out = capsys.readouterr().out.splitlines()
        restore = next(line for line in out if line.startswith("restore: "))
        subprocess.run(
            restore.removeprefix("restore: "), shell=True, check=True
        )
        assert path.read_text() == OLD_CARD + edit


class TestCheckStageRename:
    """The check passes only when stage words alone changed."""

    @pytest.mark.parametrize(
        ("renamed", "code"),
        [
            pytest.param(NEW_CARD, 0, id="only-stage-words"),
            pytest.param(OLD_CARD, 1, id="old-word-left"),
            pytest.param(
                NEW_CARD.replace("reviewers", "nobody"), 1, id="other-change"
            ),
            pytest.param(
                NEW_CARD.replace("ready: by hand", "todo: by hand"),
                1,
                id="bullet-outside-log",
            ),
            pytest.param(
                NEW_CARD + "- 2026-11-03 note (hub): later\n",
                1,
                id="added-line",
            ),
        ],
    )
    def test_check_fails_unless_only_words_renamed(
        self, tmp_path: Path, renamed: str, code: int
    ) -> None:
        """A card with an old word or another change fails the check.

        The check is what proves a real run touched nothing else. If it
        passed a changed card, damage to the user's notes would go
        unseen until sync had copied it everywhere.
        """
        backup = tmp_path / "backup" / "DevEnv"
        projects = tmp_path / "Projects" / "DevEnv"
        backup.mkdir(parents=True)
        projects.mkdir(parents=True)
        (backup / NAME).write_text(OLD_CARD)
        (projects / NAME).write_text(renamed)

        args = [str(backup.parent), str(projects.parent), *PAIRS]

        assert check_stage_rename.main(args) == code
