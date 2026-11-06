"""Tests for the card command, run against a made-up Projects folder."""

import os
from datetime import date
from pathlib import Path

import card
import pytest

KEY = "PDEV-KB-001"
TODAY = "2026-11-07"
# A card as the user might leave it after editing in Obsidian. The
# fields are reordered and quoted, plans is an inline list, and an
# extra field was added.
HAND_EDITED = """---
stage: 'active'
aliases:
  - kanban demo
tags:
  - "#Kanban"
type: card
kind: build
plans: ["[[PDEV-400 Old plan]]"]
prs: []
---

# PDEV-KB-001 Kanban demo
---

## Why
---

Cards have no tool yet.

## Outcome
---

The user makes cards from the shell.

## Open questions
---

## Decisions
---

## Log
---

- 2026-11-06 draft: created by user
"""


class FixedDate(date):
    """A date whose today() is always TODAY."""

    @classmethod
    def today(cls) -> "FixedDate":
        """Return TODAY."""
        return cls.fromisoformat(TODAY)


@pytest.fixture
def projects(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A Projects folder holding a DevEnv project with the code PDEV."""
    monkeypatch.setattr(card, "STATE_DIR", tmp_path / "state")
    monkeypatch.setattr(card, "date", FixedDate)
    monkeypatch.delenv("HERDR_PANE_ID", raising=False)
    folder = tmp_path / "Projects" / "DevEnv"
    folder.mkdir(parents=True)
    (folder / "AGENTS.md").write_text("Project code: `PDEV`\n")
    return tmp_path / "Projects"


def run_card(projects: Path, *argv: str) -> int:
    """Run the card command and return its exit code."""
    return card.main(list(argv), projects)


def write_card(
    projects: Path,
    key: str,
    stage: str,
    outcome: str = "",
    stage_date: str = "2026-11-06",
) -> Path:
    """Write a card note at a stage, with an optional Outcome."""
    text = card.render_card(
        key=key,
        title="Some work",
        project="DevEnv",
        kind="build",
        today="2026-11-01",
        stage=stage,
        log_entry=f"{stage}: by user",
    )
    text = card.set_field(text, "stage_date", stage_date)
    if outcome:
        text = text.replace(
            "## Open questions", f"{outcome}\n\n## Open questions"
        )
    path = projects / "DevEnv" / f"{key} Some work.md"
    path.write_text(text)
    return path


class TestNew:
    """Making a card picks its key from the cards already in the folder."""

    def test_new_takes_number_after_highest_card(self, projects: Path) -> None:
        """A new card takes the highest card number plus one.

        Plan notes share the project code, and cards deleted by hand
        leave gaps. If either changed the number, two cards could get
        one key, and links to the first would point at the second.
        """
        folder = projects / "DevEnv"
        (folder / "PDEV-KB-002 Old card.md").write_text("")
        (folder / "PDEV-KB-010 Newer card.md").write_text("")
        (folder / "PDEV-412 A plan.md").write_text("")

        code = run_card(
            projects, "new", "DevEnv", "Hub files drift", "--by", "user"
        )

        assert code == 0
        assert (folder / "PDEV-KB-011 Hub files drift.md").exists()

    def test_new_adopt_logs_adoption_at_stage(self, projects: Path) -> None:
        """Adopted work starts in its stage with an adoption Log line.

        Sessions already building for days come onto the board without
        walking through draft and ready. The Log must show it was an
        adoption the user asked for, and stage_date must date it so
        the review queue sorts it.
        """
        code = run_card(
            projects,
            "new",
            "DevEnv",
            "Old work",
            "--adopt",
            "review",
            "--user",
            "--why",
            "PR #450 open",
            "--by",
            "general-staff",
        )

        assert code == 0
        text = (projects / "DevEnv" / f"{KEY} Old work.md").read_text()
        assert f"stage: review\nstage_date: {TODAY}\n" in text
        assert text.endswith(
            f"- {TODAY} review: adopted by general-staff for the user."
            " PR #450 open\n"
        )

    def test_new_adopt_refuses_without_user(self, projects: Path) -> None:
        """Adopting work fails, and writes no card, without --user.

        Adoption skips the user's release to ready. If an agent could
        adopt on its own, work would reach active that the user never
        agreed to.
        """
        code = run_card(
            projects,
            "new",
            "DevEnv",
            "Old work",
            "--adopt",
            "active",
            "--by",
            "general-staff",
        )

        assert code == 1
        assert not list((projects / "DevEnv").glob("*-KB-*"))


class TestMove:
    """Moves follow the stage table, and only the user makes some moves."""

    @pytest.mark.parametrize(
        ("stage", "target", "flags", "outcome"),
        [
            pytest.param(
                "review", "accepted", [], "Done.", id="user-move-without-user"
            ),
            pytest.param(
                "draft", "active", ["--user"], "Done.", id="skips-ready"
            ),
            pytest.param(
                "draft", "ready", ["--user"], "", id="ready-without-outcome"
            ),
            pytest.param(
                "accepted", "dropped", ["--user"], "Done.", id="accepted-final"
            ),
        ],
    )
    def test_move_refuses_and_leaves_card_unchanged(
        self,
        projects: Path,
        stage: str,
        target: str,
        flags: list[str],
        outcome: str,
    ) -> None:
        """A refused move fails and does not touch the note.

        Accepting work and releasing a card are the user's decisions.
        An agent that could make them, or a card released with no
        Outcome, would take those decisions away from the user. A move
        missing from the table, such as skipping ready or reopening an
        accepted card, would let work bypass those decisions.
        """
        path = write_card(projects, KEY, stage, outcome)
        before = path.read_text()

        code = run_card(
            projects, "move", KEY, target, *flags, "--by", "general-staff"
        )

        assert code == 1
        assert path.read_text() == before

    def test_move_updates_stage_fields_and_log(self, projects: Path) -> None:
        """A move sets the stage and its date, and logs who made it.

        The staff agent rebuilds its picture of the work from the cards
        after a restart, so the stage and the Log must both change. The
        board and the Base sort by stage_date, which every move updates.
        """
        path = write_card(projects, KEY, "active")

        code = run_card(
            projects,
            "move",
            KEY,
            "review",
            "--why",
            "PR open",
            "--by",
            "general-staff",
        )

        assert code == 0
        text = path.read_text()
        assert f"stage: review\nstage_date: {TODAY}\n" in text
        assert text.endswith(f"- {TODAY} review: by general-staff. PR open\n")

    def test_move_refuses_without_author(self, projects: Path) -> None:
        """A move with no --by and no Herdr agent name fails.

        The Log is the only record of who made a move. A guessed name
        could credit the user with a move an agent made.
        """
        path = write_card(projects, KEY, "active")
        before = path.read_text()

        code = run_card(projects, "move", KEY, "review")

        assert code == 1
        assert path.read_text() == before

    @pytest.mark.parametrize(
        ("flags", "exit_code", "stage"),
        [
            pytest.param([], 1, "ready", id="agent-refused"),
            pytest.param(["--user"], 0, "active", id="user-allowed"),
        ],
    )
    def test_move_start_at_limit_needs_user(
        self, projects: Path, flags: list[str], exit_code: int, stage: str
    ) -> None:
        """At the work limit only the user can start another card.

        The limit keeps the user's review queue short enough to clear.
        Without it the staff agent would keep starting workers. The
        user decides when the limit is full, so their start must work.
        """
        for number in range(2, 2 + card.WORK_LIMIT):
            write_card(projects, f"PDEV-KB-{number:03d}", "review")
        path = write_card(projects, KEY, "ready", "Done.")

        code = run_card(
            projects, "move", KEY, "active", *flags, "--by", "general-staff"
        )

        assert code == exit_code
        assert f"stage: {stage}\n" in path.read_text()


class TestBoard:
    """The board shows what waits on the user, oldest first."""

    def test_board_lists_review_by_stage_date(
        self, projects: Path, capsys: pytest.CaptureFixture[str]
    ) -> None:
        """Review cards are listed by the day they entered review.

        The user clears the review queue from the top. Sorting by key
        or creation date would bury work that has waited longest.
        """
        write_card(projects, "PDEV-KB-001", "review", stage_date="2026-11-05")
        write_card(projects, "PDEV-KB-002", "review", stage_date="2026-11-01")

        code = run_card(projects, "board")

        assert code == 0
        out = capsys.readouterr().out
        assert out.index("PDEV-KB-002 2026-11-01") < out.index(
            "PDEV-KB-001 2026-11-05"
        )


class TestHandEditedCard:
    """Cards edited by hand in Obsidian keep working and keep the edits."""

    def test_link_keeps_hand_edits_to_card(self, projects: Path) -> None:
        """Linking a plan keeps the other fields exactly as written.

        The user edits cards in Obsidian. A tool that rewrote the
        whole frontmatter would drop their fields and reorder lines,
        and the sync would copy that to every device.
        """
        path = projects / "DevEnv" / f"{KEY} Kanban demo.md"
        path.write_text(HAND_EDITED)

        code = run_card(projects, "link", KEY, "PDEV-412 New plan")

        assert code == 0
        assert path.read_text() == HAND_EDITED.replace(
            'plans: ["[[PDEV-400 Old plan]]"]\n',
            'plans:\n  - "[[PDEV-400 Old plan]]"\n'
            '  - "[[PDEV-412 New plan]]"\n',
        )

    def test_move_reads_quoted_hand_edited_stage(self, projects: Path) -> None:
        """A stage the user quoted by hand still counts as that stage.

        Obsidian's property editor and hand edits can quote values. A
        tool that missed that would refuse every move of the card.
        """
        path = projects / "DevEnv" / f"{KEY} Kanban demo.md"
        path.write_text(HAND_EDITED)

        code = run_card(
            projects, "move", KEY, "review", "--by", "general-staff"
        )

        assert code == 0
        assert "stage: review\n" in path.read_text()

    def test_update_card_keeps_concurrent_hand_edit(
        self, projects: Path
    ) -> None:
        """A save in Obsidian while card works survives card's change.

        The user edits cards while agents run card. If card wrote over
        a save made in between, the user's edit would be lost, and the
        sync would copy the loss to every device.
        """
        path = write_card(projects, KEY, "draft")
        saved = False

        def change(text: str) -> str:
            """Add a line, after the user saves an edit the first time."""
            nonlocal saved
            if not saved:
                saved = True
                path.write_text(
                    text.replace("## Outcome", "Hand edit.\n\n## Outcome")
                )
                # Some file systems keep coarse times, so force a change.
                os.utime(path, ns=(1, 1))
            return text + "- added by card\n"

        card.update_card(card.load_card(path), change)

        text = path.read_text()
        assert "Hand edit." in text
        assert text.endswith("- added by card\n")
