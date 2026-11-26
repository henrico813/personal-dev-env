"""Tests for the card command, run against a made-up Projects folder."""

import json
import os
import re
import shlex
import subprocess
import tomllib
from datetime import date
from pathlib import Path
from typing import Any

import card
import pytest

KEY = "PDEV-KB-001"
# The key card new gives the first card in an empty project.
NEW_KEY = "PDEV-001"
TODAY = "2026-11-07"
REPO = Path(__file__).resolve().parents[1]
RULES_FILE = REPO / "chezmoi/dot_config/pde/names.toml"
# Names every reader of the rules file must make and read alike.
CASES = tomllib.loads((REPO / "testdata/names/cases.toml").read_text())
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

## Context
---

Cards have no tool yet.

## Acceptance Criteria
---

- [ ] The user makes cards from the shell.

## Open Questions
---

## Decisions
---

## Log
---

- 2026-11-06 draft: created by user
"""
# A plan made for card PDEV-005, which shares its key.
PLAN = """---
tags:
  - "#Ticket"
type: issue
status: open
---

# Plan the retries
"""
# A file for card new --from, with one heading in lower case.
CARD_FILE = """---
title: Retry failed uploads
kind: research
---

## Context
- Uploads fail on a slow link.

## acceptance criteria
- [ ] A failed upload retries.
"""
# A card as card wrote it before the version 2 layout, with a Risks
# section the user added by hand.
V1_CARD = f"""---
tags:
  - "#Kanban"
type: card
stage: active
stage_date: 2026-11-01
template_version: 1
project: DevEnv
kind: build
date_created: 2026-11-01
plans: []
prs: []
sessions: []
topics:
  - "[[DevEnv]]"
---

# {KEY} Fix some work
---

## Context
---

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
# A version 2 card with every section filled and one Agent Notes part
# for each section, in section order.
FULL_CARD = f"""---
tags:
  - "#Kanban"
type: card
stage: active
stage_date: 2026-11-01
template_version: 2
project: DevEnv
kind: build
date_created: 2026-11-01
plans: []
prs: []
sessions: []
topics:
  - "[[DevEnv]]"
---

# {KEY} Fix some work
---

## Context
---

- Uploads fail on a slow link.

## Acceptance Criteria
---

- [ ] Uploads retry.

## Open Questions
---

- How many retries?

## Decisions
---

- Retry three times.

## Notes
---

Ask about the cold bucket.

## Log
---

- 2026-11-01 active: by user

## Appendix
---

### Agent Notes on Context

- The upload code is in sync.py.

### Agent Notes on Acceptance Criteria

- A test can fake the slow link.

### Agent Notes on Open Questions

- The hub retries twice today.

### Agent Notes on Decisions

- Three matches the hub.

### Agent Notes on Notes

- The cold bucket is in the archive project.

### Agent Notes on Log

- Started on 2026-11-01.
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


@pytest.fixture(autouse=True)
def rules_file(monkeypatch: pytest.MonkeyPatch) -> None:
    """Point card at the repository's rules file, which chezmoi installs.

    Tests then check the rules users get, and never read a rules file
    installed on the computer running them.
    """
    monkeypatch.setenv("PDE_NAMES_FILE", str(RULES_FILE))


def run_card(projects: Path, *argv: str) -> int:
    """Run the card command and return its exit code."""
    return card.main(list(argv), projects)


def write_card(
    projects: Path,
    key: str,
    stage: str,
    criteria: str = "",
    stage_date: str = "2026-11-06",
    title: str = "Fix some work",
) -> Path:
    """Write a card note at a stage, with optional criteria and title."""
    text = card.render_card(
        key=key,
        title=title,
        project="DevEnv",
        kind="build",
        today="2026-11-01",
        stage=stage,
        log_entry=f"{stage}: by user",
    )
    text = card.set_field(text, "stage_date", stage_date)
    if criteria:
        text = text.replace(
            "## Open Questions",
            f"{criteria}\n\n## Open Questions",
        )
    path = projects / "DevEnv" / f"{key} {title}.md"
    path.write_text(text)
    return path


def read_log(text: str) -> str:
    """Return a card's Log text, where card adds each new line."""
    return card.read_section(text, "Log") or ""


class TestNew:
    """Making a card picks its key from the cards and plans of its code."""

    @pytest.mark.parametrize(
        ("files", "name"),
        [
            pytest.param(
                [
                    "DevEnv/PDEV-KB-010 Old card.md",
                    "Other/PDEV-412 A plan.md",
                    "Other/Archive/PDEV-415 Old plan.md",
                    "Other/HOME-900 A plan.md",
                ],
                "PDEV-416 Fix hub files drift.md",
                id="plan-in-any-folder",
            ),
            pytest.param(
                [
                    "DevEnv/PDEV-KB-450 Old card.md",
                    "DevEnv/PDEV-412 A plan.md",
                ],
                "PDEV-451 Fix hub files drift.md",
                id="old-card-highest",
            ),
        ],
    )
    def test_new_key_follows_highest_project_number(
        self,
        projects: Path,
        files: list[str],
        name: str,
    ) -> None:
        """A new card takes one more than any card or plan of its code.

        A plan made for a card shares the card's key, and plans sit in
        any project folder or archive. Missing a plan gives a new card
        the key of an unrelated plan. Missing an old card puts PDEV-300
        beside PDEV-KB-300. Other codes must not count.
        """
        for file in files:
            path = projects / file
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("")

        code = run_card(
            projects,
            "new",
            "DevEnv",
            "Fix hub files drift",
            "--by",
            "user",
        )

        assert code == 0
        assert (projects / "DevEnv" / name).exists()

    def test_new_writes_version_2_layout(self, projects: Path) -> None:
        """card new writes the seven sections in order, as version 2.

        Agents and the hub find a card's parts by these headings. A
        section out of order, or a Log line below the Appendix, would send
        them to the wrong text.
        """
        code = run_card(
            projects,
            "new",
            "DevEnv",
            "Retry failed uploads",
            "--by",
            "user",
        )

        assert code == 0
        path = projects / "DevEnv" / f"{NEW_KEY} Retry failed uploads.md"
        text = path.read_text()
        assert re.findall(r"^## (.*)$", text, re.M) == [
            "Context",
            "Acceptance Criteria",
            "Open Questions",
            "Decisions",
            "Notes",
            "Log",
            "Appendix",
        ]
        assert "template_version: 2\n" in text
        assert read_log(text).strip() == f"- {TODAY} draft: by user"

    def test_new_adopt_logs_adoption_at_stage(self, projects: Path) -> None:
        """Adopted work starts in its stage with an adoption Log line.

        Sessions already building for days come onto the board without
        walking through draft and todo. The Log must show it was an
        adoption the user asked for, and stage_date must date it so
        the in-review queue sorts it.
        """
        code = run_card(
            projects,
            "new",
            "DevEnv",
            "Old work",
            "--adopt",
            "in-review",
            "--user",
            "--why",
            "PR #450 open",
            "--by",
            "general-staff",
        )

        assert code == 0
        text = (projects / "DevEnv" / f"{NEW_KEY} Old work.md").read_text()
        assert f"stage: in-review\nstage_date: {TODAY}\n" in text
        assert read_log(text).endswith(
            f"- {TODAY} in-review: adopted by general-staff for the user."
            " PR #450 open\n",
        )

    def test_new_adopt_refuses_without_user(self, projects: Path) -> None:
        """Adopting work fails, and writes no card, without --user.

        Adoption skips the criteria check and the stage limits. If an
        agent could adopt on its own, work would reach active with no
        test of when it is done.
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
        assert not list((projects / "DevEnv").glob("PDEV-*"))

    def test_new_from_copies_fields_and_sections(
        self,
        projects: Path,
        tmp_path: Path,
    ) -> None:
        """A --from file gives the card its title, type, kind and sections.

        An agent or the user makes a card from one file instead of a
        card add per line. Text under the wrong heading or a lost kind
        would hand a worker the wrong brief, and a broken layout would
        leave a card that card check flags.
        """
        card_file = tmp_path / "card.md"
        card_file.write_text(CARD_FILE)

        code = run_card(
            projects,
            "new",
            "DevEnv",
            "--from",
            str(card_file),
            "--by",
            "user",
        )

        assert code == 0
        path = projects / "DevEnv" / f"{NEW_KEY} Retry failed uploads.md"
        text = path.read_text()
        assert "kind: research\n" in text
        assert card.read_section(text, "Context") == (
            "\n- Uploads fail on a slow link.\n"
        )
        assert card.read_section(text, card.CRITERIA) == (
            "\n- [ ] A failed upload retries.\n"
        )
        assert run_card(projects, "check") == 0

    @pytest.mark.parametrize(
        ("file_text", "args"),
        [
            pytest.param(
                CARD_FILE.replace("## Context", "## Criteria"),
                ["--from", "FILE"],
                id="unknown-heading",
            ),
            pytest.param(
                CARD_FILE.replace("## Context\n", ""),
                ["--from", "FILE"],
                id="text-before-heading",
            ),
            pytest.param(
                CARD_FILE + "\n## Context\n- Again.\n",
                ["--from", "FILE"],
                id="duplicate-section",
            ),
            pytest.param(
                CARD_FILE.replace("kind: research", "kind: chore"),
                ["--from", "FILE"],
                id="unknown-kind",
            ),
            pytest.param(
                CARD_FILE,
                ["Other title", "--from", "FILE"],
                id="title-and-file",
            ),
            pytest.param(
                CARD_FILE,
                ["--from", "FILE", "--kind", "build"],
                id="kind-and-file",
            ),
            pytest.param(None, ["--from", "FILE"], id="missing-file"),
            pytest.param(None, [], id="no-title-or-file"),
        ],
    )
    def test_new_from_refusal_writes_no_card(
        self,
        projects: Path,
        tmp_path: Path,
        file_text: str | None,
        args: list[str],
    ) -> None:
        """A bad --from file, or arguments that clash with it, write no card.

        A typo such as ## Criteria, or a section written twice, would
        otherwise drop that text from the card without notice, and the
        worker would build without it. A TITLE or --kind beside the file
        leaves unclear which one the user meant. FILE in args
        stands for the card file, which is not written when file_text is
        None.
        """
        card_file = tmp_path / "card.md"
        if file_text is not None:
            card_file.write_text(file_text)
        args = [str(card_file) if arg == "FILE" else arg for arg in args]

        code = run_card(projects, "new", "DevEnv", *args, "--by", "user")

        assert code == 1
        assert not list((projects / "DevEnv").glob("PDEV-*"))


class TestMove:
    """Moves follow the stage table, and only the user makes some moves."""

    @pytest.mark.parametrize(
        ("stage", "target", "flags", "criteria"),
        [
            pytest.param(
                "draft",
                "active",
                [],
                "- Done.",
                id="chain-unchecked",
            ),
            pytest.param(
                "draft",
                "todo",
                [],
                "- Done.",
                id="todo-without-checkbox",
            ),
            pytest.param(
                "draft",
                "todo",
                ["--user"],
                "- Done.",
                id="user-todo-without-checkbox",
            ),
            pytest.param(
                "done",
                "dropped",
                ["--user"],
                "- [ ] Done.",
                id="done-final",
            ),
            pytest.param(
                "draft",
                "dropped",
                [],
                "- [ ] Done.",
                id="agent-drop",
            ),
            pytest.param(
                "in-review",
                "blocked",
                [],
                "- [ ] Done.",
                id="blocked-without-why",
            ),
        ],
    )
    def test_move_refuses_and_leaves_card_unchanged(
        self,
        projects: Path,
        stage: str,
        target: str,
        flags: list[str],
        criteria: str,
    ) -> None:
        """A refused move exits 1 and leaves the note unchanged.

        A half-applied move would leave a card in a stage its checks reject.
        """
        path = write_card(projects, KEY, stage, criteria)
        before = path.read_text()

        code = run_card(
            projects,
            "move",
            KEY,
            target,
            *flags,
            "--by",
            "general-staff",
        )

        assert code == 1
        assert path.read_text() == before

    def test_move_releases_draft_and_logs_agent(self, projects: Path) -> None:
        """A draft moved to todo logs the agent who moved it.

        The user reads the Log to see which agent released a card.
        """
        path = write_card(projects, KEY, "draft", "- [ ] Done.")

        code = run_card(
            projects,
            "move",
            KEY,
            "todo",
            "--why",
            "Clear enough",
            "--by",
            "hub",
        )

        assert code == 0
        text = path.read_text()
        assert f"stage: todo\nstage_date: {TODAY}\n" in text
        assert read_log(text).endswith(
            f"- {TODAY} todo: by hub. Clear enough\n",
        )

    @pytest.mark.parametrize(
        ("source", "target", "steps"),
        [
            pytest.param("active", "done", ["in-review", "done"], id="accept"),
            pytest.param("in-review", "todo", ["active", "todo"], id="reopen"),
        ],
    )
    def test_move_logs_each_move_of_chain(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
        source: str,
        target: str,
        steps: list[str],
    ) -> None:
        """The user's move with no single step makes each step and logs it.

        A hub told to accept active work, or to send reviewed work back
        to todo, used to be refused and had to retry one stage at a
        time. The Log must still show every stage the card passed, and
        the reason belongs to the stage the user asked for.
        """
        path = write_card(projects, KEY, source, "- [ ] Done.")

        code = run_card(
            projects,
            "move",
            KEY,
            target,
            "--user",
            "--why",
            "Looks good",
            "--by",
            "hub",
        )

        assert code == 0
        text = path.read_text()
        assert f"stage: {target}\nstage_date: {TODAY}\n" in text
        assert read_log(text).endswith(
            f"- {TODAY} {steps[0]}: by hub for the user\n"
            f"- {TODAY} {steps[1]}: by hub for the user. Looks good\n",
        )
        assert capsys.readouterr().out == f"{KEY} {steps[0]}\n{KEY} {target}\n"

    @pytest.mark.parametrize(
        ("source", "target", "hint"),
        [
            pytest.param(
                "active",
                "done",
                "only the user moves in-review -> done."
                f" You can move to in-review: card move {KEY} in-review",
                id="accept-from-active",
            ),
            pytest.param(
                "in-review",
                "todo",
                "only the user moves in-review -> active, on the way to todo."
                " Pass --user only when the user said so.",
                id="reopen-in-review",
            ),
            pytest.param(
                "in-review",
                "done",
                "only the user moves in-review -> done."
                " Pass --user only when the user said so.",
                id="accept-from-in-review",
            ),
        ],
    )
    def test_move_refusal_names_the_next_move(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
        source: str,
        target: str,
        hint: str,
    ) -> None:
        """An agent's move that needs the user is refused with a hint.

        Accepting and reopening are the user's decisions, so the card
        must not change. The one-line hint names the move the agent can
        make, so it does not guess and get refused again.
        """
        path = write_card(projects, KEY, source, "- [ ] Done.")
        before = path.read_text()

        code = run_card(projects, "move", KEY, target, "--by", "hub")

        assert code == 1
        assert path.read_text() == before
        assert capsys.readouterr().err == f"card: {hint}\n"

    def test_move_pauses_active_card_without_criteria(
        self,
        projects: Path,
    ) -> None:
        """The staff agent can pause active work, even with no criteria.

        Paused work goes back to todo to wait for a free slot, and
        adopted work may have no criteria yet. A refusal would leave
        paused work counted against the active limit.
        """
        path = write_card(projects, KEY, "active")

        code = run_card(
            projects,
            "move",
            KEY,
            "todo",
            "--why",
            "paused, handoff kb-1.md",
            "--by",
            "general-staff",
        )

        assert code == 0
        assert "stage: todo\n" in path.read_text()

    def test_move_refuses_without_author(self, projects: Path) -> None:
        """A move with no --by and no Herdr agent name fails.

        The Log is the only record of who made a move. A guessed name
        could credit the user with a move an agent made.
        """
        path = write_card(projects, KEY, "active")
        before = path.read_text()

        code = run_card(projects, "move", KEY, "in-review")

        assert code == 1
        assert path.read_text() == before

    @pytest.mark.parametrize(
        ("source", "target", "limit"),
        [
            pytest.param("todo", "active", card.ACTIVE_LIMIT, id="start"),
            pytest.param(
                "active",
                "in-review",
                card.REVIEW_LIMIT,
                id="in-review",
            ),
            pytest.param("todo", "in-review", card.REVIEW_LIMIT, id="chain"),
        ],
    )
    @pytest.mark.parametrize(
        ("flags", "moved"),
        [
            pytest.param([], False, id="agent-refused"),
            pytest.param(["--user"], True, id="user-allowed"),
        ],
    )
    def test_move_into_full_stage_needs_user(
        self,
        projects: Path,
        source: str,
        target: str,
        limit: int,
        flags: list[str],
        moved: bool,
    ) -> None:
        """A full active or in-review stage takes a card only from the user.

        The limits keep the user's review queue short enough to clear.
        Without them the staff agent would keep starting workers and
        sending reviews. The user decides when a stage is full, so
        their move must work. A chain of moves is held to the same
        limits, or it would be a way around them.
        """
        for number in range(2, 2 + limit):
            write_card(projects, f"PDEV-KB-{number:03d}", target)
        path = write_card(projects, KEY, source, "- [ ] Done.")

        code = run_card(
            projects,
            "move",
            KEY,
            target,
            *flags,
            "--by",
            "general-staff",
        )

        assert code == (0 if moved else 1)
        stage = target if moved else source
        assert f"stage: {stage}\n" in path.read_text()

    def test_review_limit_skips_blocked_cards(self, projects: Path) -> None:
        """Blocked cards leave the in-review places free.

        Pull requests that wait on another team's reviewers used to
        fill review, so the user's own plans and questions could not
        reach them. Counting blocked cards would bring that back.
        """
        for number in range(2, 2 + card.REVIEW_LIMIT):
            write_card(projects, f"PDEV-KB-{number:03d}", "blocked")
        path = write_card(projects, KEY, "active")

        code = run_card(
            projects,
            "move",
            KEY,
            "in-review",
            "--by",
            "general-staff",
        )

        assert code == 0
        assert "stage: in-review\n" in path.read_text()


@pytest.fixture
def fake_herdr(projects: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A herdr that reports agent worker-1 in session opencode ses_abc."""
    # Needs projects first, since projects clears HERDR_PANE_ID.
    monkeypatch.setenv("HERDR_PANE_ID", "w4X:p2")
    herdr_answer = (
        '{"result": {"agent": {"name": "worker-1", "agent": "opencode",'
        ' "agent_session": {"value": "ses_abc"}}}}'
    )
    monkeypatch.setattr(
        card.subprocess,
        "run",
        lambda command, **kwargs: subprocess.CompletedProcess(
            command,
            0,
            herdr_answer,
            "",
        ),
    )


@pytest.mark.usefixtures("fake_herdr")
class TestSessions:
    """A card records the Herdr sessions that worked on it."""

    def test_link_session_records_entry_once(self, projects: Path) -> None:
        """Linking a session twice gives one sessions entry and one Log line.

        The staff agent finds a card's workers through sessions after a
        restart, and agents may link the same session again. A doubled
        entry or Log line would make one worker look like two.
        """
        path = write_card(projects, KEY, "active")

        first = run_card(projects, "link", KEY, "--session")
        second = run_card(projects, "link", KEY, "--session")

        assert (first, second) == (0, 0)
        text = path.read_text()
        assert 'prs: []\nsessions:\n  - "opencode ses_abc"\ntopics:' in text
        assert text.count("session: worker-1 (opencode ses_abc)") == 1
        assert read_log(text).endswith(
            f"- {TODAY} session: worker-1 (opencode ses_abc)\n",
        )

    @pytest.mark.parametrize(
        ("stage", "argv", "stage_line"),
        [
            pytest.param(
                "todo",
                ["move", NEW_KEY, "active"],
                "active: by user",
                id="start",
            ),
            pytest.param(
                "",
                [
                    "new",
                    "DevEnv",
                    "Fix some work",
                    "--adopt",
                    "active",
                    "--user",
                ],
                "active: adopted by user for the user",
                id="adopt",
            ),
        ],
    )
    def test_start_with_session_records_entry(
        self,
        projects: Path,
        stage: str,
        argv: list[str],
        stage_line: str,
    ) -> None:
        """Starting or adopting a card with --session records the worker.

        The staff agent starts each worker with card move KEY active
        --session, and the user adopts running work the same way. If
        the session were dropped, the card would not say which worker
        holds it.
        """
        if stage:
            write_card(projects, NEW_KEY, stage, "- [ ] Done.")

        code = run_card(projects, *argv, "--session", "--by", "user")

        assert code == 0
        path = projects / "DevEnv" / f"{NEW_KEY} Fix some work.md"
        text = path.read_text()
        assert 'sessions:\n  - "opencode ses_abc"\n' in text
        assert read_log(text).endswith(
            f"- {TODAY} {stage_line}\n"
            f"- {TODAY} session: worker-1 (opencode ses_abc)\n",
        )

    def test_link_refuses_session_on_other_card(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """A session another card holds is refused, naming that card.

        The hub shows a worker's state on the card that holds its
        session, so a session on two cards would show one worker on the
        wrong card. A worker that serves a second card is told to log it
        with card note instead.
        """
        write_card(projects, "PDEV-KB-001", "active")
        run_card(projects, "link", "PDEV-KB-001", "--session")
        other = write_card(projects, "PDEV-KB-002", "active")
        before = other.read_text()
        capsys.readouterr()

        code = run_card(projects, "link", "PDEV-KB-002", "--session")

        assert code == 1
        err = capsys.readouterr().err
        assert "session ID already on PDEV-KB-001" in err
        assert "run card note PDEV-KB-002 TEXT" in err
        assert other.read_text() == before

    def test_link_allows_session_card_already_holds(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """Relinking a session this card holds passes, even if another has it.

        The hub relinks a worker's own card when it parks the worker. A
        refusal would fail every park of a worker whose session another
        card also holds.
        """
        for key in ("PDEV-KB-001", "PDEV-KB-002"):
            path = write_card(projects, key, "active")
            text = card.set_field(
                path.read_text(),
                "sessions",
                ["opencode ses_abc"],
            )
            path.write_text(text)
        before = path.read_text()

        code = run_card(projects, "link", "PDEV-KB-002", "--session")

        assert code == 0
        assert capsys.readouterr().out == (
            "PDEV-KB-002 sessions: opencode ses_abc (already)\n"
        )
        assert path.read_text() == before

    def test_check_reports_session_on_two_cards(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card check names both cards that hold one session.

        Hand edits, card move --session and card new --adopt --session
        can put a session on two cards. card check is where the user
        finds it.
        """
        for key in ("PDEV-KB-001", "PDEV-KB-002"):
            path = write_card(projects, key, "active")
            text = card.set_field(
                path.read_text(),
                "sessions",
                ["opencode ses_abc"],
            )
            path.write_text(text)

        code = run_card(projects, "check")

        assert code == 1
        assert capsys.readouterr().out == (
            "PDEV-KB-002 session also on PDEV-KB-001: opencode ses_abc\n"
            "1 problems in 2 cards\n"
        )

    def test_check_json_keeps_whole_problem_lines(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card check --json gives each problem uncut, and still exits 1.

        The text output cuts lines to 60 characters, which drops the end
        of a long session ID. A script reading the JSON needs the whole
        ID to find the session, and the exit code to know a problem exists.
        """
        session = "opencode ses_e4dc27c20ffeJkm3JZ8dwLh7rW"
        for key in ("PDEV-KB-001", "PDEV-KB-002"):
            path = write_card(projects, key, "active")
            path.write_text(
                card.set_field(path.read_text(), "sessions", [session]),
            )

        code = run_card(projects, "check", "--json")

        assert code == 1
        assert json.loads(capsys.readouterr().out) == {
            "cards": 2,
            "problems": [
                f"PDEV-KB-002 session also on PDEV-KB-001: {session}",
            ],
        }


class TestEdit:
    """One numbered item can be replaced, removed, ticked or unticked."""

    @pytest.mark.parametrize(
        ("item_args", "criteria", "note"),
        [
            pytest.param(
                ["2", "- [ ] Second, sharper."],
                (
                    "- [x] First.\n- [ ] Second, sharper.\n"
                    "  more words\n- [ ] Third.\n"
                ),
                'replaced Acceptance Criteria item 2, was "Second."',
                id="replace",
            ),
            pytest.param(
                ["2", "--remove"],
                "- [x] First.\n- [ ] Third.\n",
                'removed Acceptance Criteria item 2, "Second."',
                id="remove",
            ),
            pytest.param(
                ["2", "--tick"],
                "- [x] First.\n- [x] Second.\n  more words\n- [ ] Third.\n",
                'ticked Acceptance Criteria item 2, "Second."',
                id="tick",
            ),
            pytest.param(
                ["1", "--untick"],
                "- [ ] First.\n- [ ] Second.\n  more words\n- [ ] Third.\n",
                'unticked Acceptance Criteria item 1, "First."',
                id="untick",
            ),
        ],
    )
    def test_edit_changes_item_n_and_logs(
        self,
        projects: Path,
        item_args: list[str],
        criteria: str,
        note: str,
    ) -> None:
        """Editing one item changes that item only and logs it.

        Agents fix vague criteria and tick met ones by the numbers card
        show prints. An off-by-one would change the wrong criterion, a
        lost checkbox would hide it from the todo check, a tick that
        dropped the words would lose the criterion, and the Log line is
        the only record of the old words.
        """
        path = write_card(projects, KEY, "todo", "- [ ] First.")
        path.write_text(
            path.read_text().replace(
                "- [ ] First.\n",
                "- [x] First.\n- [ ] Second.\n  more words\n- [ ] Third.\n",
            ),
        )

        code = run_card(
            projects,
            "edit",
            KEY,
            "Acceptance Criteria",
            *item_args,
            "--by",
            "kanban-rollout-2",
        )

        assert code == 0
        text = path.read_text()
        assert f"---\n\n{criteria}\n## Open Questions" in text
        assert read_log(text).endswith(
            f"- {TODAY} note (kanban-rollout-2): {note}\n",
        )

    def test_show_section_numbers_items_for_edit(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """The numbers card show prints are the ones card edit takes.

        Agents read the number from card show and pass it to card edit.
        If the two counted differently, an agent would rewrite the
        wrong criterion on a real card.
        """
        path = write_card(projects, KEY, "todo", "- [ ] First.")
        path.write_text(
            path.read_text().replace(
                "- [ ] First.\n",
                "- [ ] First.\n  more words\n- [ ] Second.\n",
            ),
        )

        code = run_card(projects, "show", KEY, "--section", card.CRITERIA)

        assert code == 0
        assert capsys.readouterr().out == (
            f"## {card.CRITERIA}\n"
            "1. - [ ] First.\n  more words\n2. - [ ] Second.\n"
        )


def write_questions(projects: Path, items: str) -> Path:
    """Write an active card whose Open Questions section holds ``items``."""
    path = write_card(projects, KEY, "active")
    path.write_text(
        path.read_text().replace(
            "## Open Questions\n---\n",
            f"## Open Questions\n---\n\n{items}",
        ),
    )
    return path


class TestOpenQuestions:
    """Open Questions items keep their Q numbers until they are answered."""

    def test_add_numbers_and_dates_new_question(self, projects: Path) -> None:
        """card add gives a question the next number and today's date.

        The user answers by Q number, so two items with one number, or
        a hand-typed item whose number shifts when a question is added,
        would answer the wrong question.
        """
        path = write_questions(
            projects,
            "- Q2 asked 2026-11-01: Old?\n- Typed by hand?\n",
        )

        code = run_card(
            projects,
            "add",
            KEY,
            "Open Questions",
            "New?\n  - A. Yes\n  Recommended: A",
        )

        assert code == 0
        assert card.read_section(path.read_text(), "Open Questions") == (
            "\n- Q2 asked 2026-11-01: Old?\n"
            "- Q3: Typed by hand?\n"
            f"- Q4 asked {TODAY}: New?\n"
            "  - A. Yes\n"
            "  Recommended: A\n"
        )

    @pytest.mark.parametrize(
        ("item", "change", "left"),
        [
            pytest.param(
                "Q2",
                ["--remove"],
                "- Q3: Typed by hand?\n- Q1 asked 2026-11-01: First?\n",
                id="remove",
            ),
            pytest.param(
                "Q3",
                ["--remove"],
                (
                    "- Q1 asked 2026-11-01: First?\n"
                    "- Q2 asked 2026-11-02: Second?\n"
                ),
                id="remove-hand-typed",
            ),
            pytest.param(
                "Q2",
                ["Sharper?"],
                (
                    "- Q3: Typed by hand?\n"
                    "- Q1 asked 2026-11-01: First?\n"
                    "- Q2 asked 2026-11-02: Sharper?\n"
                ),
                id="replace",
            ),
        ],
    )
    def test_edit_by_q_number_keeps_numbers(
        self,
        projects: Path,
        item: str,
        change: list[str],
        left: str,
    ) -> None:
        """card edit by Q number changes that item, and no other's number.

        hub answer removes a question by its Q number, not its place. Each
        case would hit the wrong question if card read Q2 as the second
        item, renumbered the hand-typed Q3, could not find Q3, or dropped
        the item's ``Q<n>`` head.
        """
        path = write_questions(
            projects,
            "- Typed by hand?\n"
            "- Q1 asked 2026-11-01: First?\n"
            "- Q2 asked 2026-11-02: Second?\n",
        )

        code = run_card(
            projects,
            "edit",
            KEY,
            "Open Questions",
            item,
            *change,
            "--by",
            "hub",
        )

        assert code == 0
        section = card.read_section(path.read_text(), "Open Questions")
        assert section == f"\n{left}"

    def test_add_skips_numbers_of_answered_questions(
        self,
        projects: Path,
    ) -> None:
        """card add never gives a new question an answered question's number.

        hub answer moves the answered Q2 to Decisions with its head. A
        new or hand-typed question that took Q2 again would let a late
        reply such as ``KEY Q2A``, meant for the old one, answer it.
        """
        path = write_questions(
            projects,
            "- Q1 asked 2026-11-01: First?\n- Typed by hand?\n",
        )
        run_card(
            projects,
            "add",
            KEY,
            "Decisions",
            "Q2 asked 2026-11-02: Second?\n  Answer: A",
        )

        code = run_card(projects, "add", KEY, "Open Questions", "Third?")

        assert code == 0
        assert card.read_section(path.read_text(), "Open Questions") == (
            "\n"
            "- Q1 asked 2026-11-01: First?\n"
            "- Q3: Typed by hand?\n"
            f"- Q4 asked {TODAY}: Third?\n"
        )

    def test_edit_refuses_unknown_q_number(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card edit Q5 fails, and changes nothing, when no item is Q5.

        A second hub answer for one question must not remove the item
        that now sits at position 5 or any other question.
        """
        items = "- Q1: a\n- Q2: b\n- Q3: c\n- Q4: d\n- Q6: e\n"
        path = write_questions(projects, items)
        before = path.read_text()

        code = run_card(
            projects,
            "edit",
            KEY,
            "Open Questions",
            "Q5",
            "--remove",
            "--by",
            "hub",
        )

        assert code == 1
        assert "has no Q5" in capsys.readouterr().err
        assert path.read_text() == before


class TestShow:
    """card show finds one card by its key."""

    def test_show_finds_card_beside_its_plan(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card show finds a card whose plan shares its key.

        Every card command finds its card this way. If the plan counted
        as a second holder of the key, the hub and agents could not show
        or move the card once its plan exists.
        """
        write_card(projects, "PDEV-005", "draft", title="Build the retries")
        (projects / "DevEnv" / "PDEV-005 Plan the retries.md").write_text(PLAN)

        code = run_card(projects, "show", "pdev-005")

        assert code == 0
        assert "# PDEV-005 Build the retries\n" in capsys.readouterr().out

    def test_show_finds_card_by_old_key(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card show finds a rekeyed card by the key it had before.

        Log lines, notes outside Projects and agents' memories still name
        old keys after card rekey, and every card command finds its card
        the way show does.
        """
        path = write_card(projects, "PDEV-411", "draft")
        path.write_text(card.set_field(path.read_text(), "old_key", KEY))

        code = run_card(projects, "show", "pdev-kb-001")

        assert code == 0
        assert "# PDEV-411 Fix some work\n" in capsys.readouterr().out

    def test_show_json_gives_section_and_fields(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card show --json --section gives the card's fields and that text.

        An agent reads a card's criteria and stage in one call. Other
        sections in the output, or a missing stage, would make it read
        the card twice or act on the wrong stage.
        """
        write_card(projects, KEY, "active", criteria="- [ ] Uploads retry.")

        code = run_card(
            projects,
            "show",
            KEY,
            "--section",
            "Acceptance Criteria",
            "--json",
        )

        assert code == 0
        shown = json.loads(capsys.readouterr().out)
        assert (shown["key"], shown["stage"]) == (KEY, "active")
        assert shown["sections"] == {
            "Acceptance Criteria": "- [ ] Uploads retry.",
        }

    def test_show_json_numbers_every_question(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """Each Open Questions item gets a Q number, a date and its lines.

        The hub lists items by these numbers, and hub answer removes an
        item by its number. An item typed by hand, or with a Q number too
        long for the hub, takes one after the highest written number;
        reusing a written number would answer the wrong question. Later
        lines lose their two-space indent, and a bad date such as
        2026-13-45 reads as undated, since one the hub cannot read fails
        its whole card list.
        """
        path = write_card(projects, KEY, "active")
        path.write_text(
            path.read_text().replace(
                "## Open Questions\n---\n",
                "## Open Questions\n---\n\n"
                "- Typed in Obsidian?\n"
                "- Q3 asked 2026-11-20: Which bucket?\n"
                "  - A. The cold one\n"
                "  Recommended: A\n"
                "- Q5 asked 2026-13-45: Deploy tonight?\n"
                "- Q2: Stamped by hand?\n"
                "- Q12345678: Too big?\n",
            ),
        )
        capsys.readouterr()

        code = run_card(projects, "show", KEY, "--json")

        assert code == 0
        assert json.loads(capsys.readouterr().out)["open_questions"] == [
            {
                "number": 6,
                "asked": None,
                "text": "Typed in Obsidian?",
                "lines": [],
            },
            {
                "number": 3,
                "asked": "2026-11-20",
                "text": "Which bucket?",
                "lines": ["- A. The cold one", "Recommended: A"],
            },
            {
                "number": 5,
                "asked": None,
                "text": "Deploy tonight?",
                "lines": [],
            },
            {
                "number": 2,
                "asked": None,
                "text": "Stamped by hand?",
                "lines": [],
            },
            {
                "number": 7,
                "asked": None,
                "text": "Q12345678: Too big?",
                "lines": [],
            },
        ]


class TestReadParts:
    """card reads one part of a card, and its writes keep every part."""

    def test_full_card_reads_and_keeps_parts(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card reads each part of a full card, and its writes keep them.

        Agents read a card part by part, through outline, --section and
        --json. A wrong range or a part cut short hands them the wrong text.
        A write that drops an Agent Notes part, or puts a Log line in the
        Appendix, garbles an agent's notes.
        """
        path = projects / "DevEnv" / f"{KEY} Fix some work.md"
        path.write_text(FULL_CARD)

        # Read the outline, each section and the JSON before any write.
        run_card(projects, "outline", KEY)
        outline = capsys.readouterr().out
        sections = {}
        for name in card.SECTIONS:
            run_card(projects, "show", KEY, "--section", name)
            sections[name] = capsys.readouterr().out
        run_card(projects, "show", KEY, "--json")
        shown = json.loads(capsys.readouterr().out)
        # Add a criterion, edit a decision and add a Log note.
        run_card(
            projects,
            "add",
            KEY,
            "Acceptance Criteria",
            "Retries back off.",
        )
        run_card(
            projects,
            "edit",
            KEY,
            "Decisions",
            "1",
            "Retry four times.",
            "--by",
            "hub",
        )
        run_card(projects, "note", KEY, "bucket picked", "--by", "hub")

        assert outline == (
            "21-25     ## Context\n"
            "26-30     ## Acceptance Criteria\n"
            "31-35     ## Open Questions\n"
            "36-40     ## Decisions\n"
            "41-45     ## Notes\n"
            "46-50     ## Log\n"
            "51-76     ## Appendix\n"
            "54-57       ### Agent Notes on Context\n"
            "58-61       ### Agent Notes on Acceptance Criteria\n"
            "62-65       ### Agent Notes on Open Questions\n"
            "66-69       ### Agent Notes on Decisions\n"
            "70-73       ### Agent Notes on Notes\n"
            "74-76       ### Agent Notes on Log\n"
        )
        assert sections == {
            "Context": "## Context\n1. - Uploads fail on a slow link.\n",
            "Acceptance Criteria": (
                "## Acceptance Criteria\n1. - [ ] Uploads retry.\n"
            ),
            "Open Questions": "## Open Questions\n1. - How many retries?\n",
            "Decisions": "## Decisions\n1. - Retry three times.\n",
            "Notes": "## Notes\n1. Ask about the cold bucket.\n",
            "Log": "## Log\n1. - 2026-11-01 active: by user\n",
            "Appendix": (
                "## Appendix\n"
                "1. ### Agent Notes on Context\n\n"
                "2. - The upload code is in sync.py.\n\n"
                "3. ### Agent Notes on Acceptance Criteria\n\n"
                "4. - A test can fake the slow link.\n\n"
                "5. ### Agent Notes on Open Questions\n\n"
                "6. - The hub retries twice today.\n\n"
                "7. ### Agent Notes on Decisions\n\n"
                "8. - Three matches the hub.\n\n"
                "9. ### Agent Notes on Notes\n\n"
                "10. - The cold bucket is in the archive project.\n\n"
                "11. ### Agent Notes on Log\n\n"
                "12. - Started on 2026-11-01.\n"
            ),
        }
        assert shown["sections"] == {
            "Context": "- Uploads fail on a slow link.",
            "Acceptance Criteria": "- [ ] Uploads retry.",
            "Open Questions": "- How many retries?",
            "Decisions": "- Retry three times.",
            "Notes": "Ask about the cold bucket.",
            "Log": "- 2026-11-01 active: by user",
            "Appendix": (
                "### Agent Notes on Context\n\n"
                "- The upload code is in sync.py.\n\n"
                "### Agent Notes on Acceptance Criteria\n\n"
                "- A test can fake the slow link.\n\n"
                "### Agent Notes on Open Questions\n\n"
                "- The hub retries twice today.\n\n"
                "### Agent Notes on Decisions\n\n"
                "- Three matches the hub.\n\n"
                "### Agent Notes on Notes\n\n"
                "- The cold bucket is in the archive project.\n\n"
                "### Agent Notes on Log\n\n"
                "- Started on 2026-11-01."
            ),
        }
        assert path.read_text() == (
            FULL_CARD.replace(
                "- [ ] Uploads retry.\n",
                "- [ ] Uploads retry.\n- [ ] Retries back off.\n",
            )
            .replace("- Retry three times.\n", "- Retry four times.\n")
            .replace(
                "- 2026-11-01 active: by user\n",
                "- 2026-11-01 active: by user\n"
                f"- {TODAY} note (hub): replaced Decisions item 1,"
                ' was "Retry three times."\n'
                f"- {TODAY} note (hub): bucket picked\n",
            )
        )

    def test_show_section_prints_hand_made_section(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card show --section prints a hand-made section, in any case.

        Users add sections by hand that card does not list. If show took
        only its own names or their exact case, an agent would have to
        read the whole card.
        """
        (projects / "DevEnv" / f"{KEY} Fix some work.md").write_text(V1_CARD)

        code = run_card(projects, "show", KEY, "--section", "risks")

        assert code == 0
        assert capsys.readouterr().out == (
            "## Risks\n1. - The bucket may be full.\n"
        )

    def test_show_section_rejects_unknown_name(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card show --section refuses a section name it does not know.

        A typo must fail. An empty section would tell an agent the card
        has no criteria.
        """
        write_card(projects, KEY, "active")
        typo = "Acceptance Critera"

        code = run_card(projects, "show", KEY, "--section", typo)

        captured = capsys.readouterr()
        assert code == 1
        assert captured.out == ""
        assert captured.err == f"card: {KEY} has no {typo} section\n"


class TestAttach:
    """Attaching copies a file into the project and embeds it in Context."""

    def test_attach_copies_file_and_embeds_it(
        self,
        projects: Path,
        tmp_path: Path,
    ) -> None:
        """The file lands in attachments under the key, and Context shows it.

        The user attaches photos so a card holds what they saw. A wrong
        name or folder would leave the embed pointing at nothing, and a
        bad copy would show a broken image.
        """
        path = write_card(projects, KEY, "draft")
        photo = tmp_path / "photo.jpg"
        photo.write_bytes(b"made-up image bytes")

        code = run_card(projects, "attach", KEY, str(photo))

        assert code == 0
        attachment = projects / "DevEnv" / "attachments" / f"{KEY} photo.jpg"
        assert attachment.read_bytes() == b"made-up image bytes"
        context = card.read_section(path.read_text(), "Context")
        assert context.rstrip().endswith(f"- ![[{KEY} photo.jpg]]")

    @pytest.mark.parametrize(
        "name",
        [
            pytest.param("missing.jpg", id="missing-file"),
            pytest.param("", id="folder"),
        ],
    )
    def test_attach_without_file_leaves_vault_unchanged(
        self,
        projects: Path,
        tmp_path: Path,
        name: str,
    ) -> None:
        """A path with no file behind it fails before the vault changes.

        A typo in the path is easy to make. If card still made the
        attachments folder or an embed line, the card would show a
        broken image and the user would have to clean up by hand.
        """
        path = write_card(projects, KEY, "draft")
        before = path.read_text()

        code = run_card(projects, "attach", KEY, str(tmp_path / name))

        assert code == 1
        assert path.read_text() == before
        assert not (projects / "DevEnv" / "attachments").exists()

    def test_attach_refuses_to_overwrite_existing_attachment(
        self,
        projects: Path,
        tmp_path: Path,
    ) -> None:
        """A second file with the same name keeps the first one intact.

        Attachments are the only copy of some photos once the original
        is deleted. Writing over one would lose it, and the sync would
        copy the loss to every device.
        """
        path = write_card(projects, KEY, "draft")
        before = path.read_text()
        attachment = projects / "DevEnv" / "attachments" / f"{KEY} photo.jpg"
        attachment.parent.mkdir()
        attachment.write_bytes(b"first photo")
        photo = tmp_path / "photo.jpg"
        photo.write_bytes(b"second photo")

        code = run_card(projects, "attach", KEY, str(photo))

        assert code == 1
        assert attachment.read_bytes() == b"first photo"
        assert path.read_text() == before

    def test_attach_removes_copy_when_update_fails(
        self,
        projects: Path,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """When the card cannot be saved, only the new copy is removed.

        Without the cleanup, the copy would stay with no embed, and the
        retry would be refused as an overwrite. Removing the wrong file
        instead would delete the user's original.
        """
        write_card(projects, KEY, "draft")
        photo = tmp_path / "photo.jpg"
        photo.write_bytes(b"made-up image bytes")

        def refuse_update(*args: object) -> bool:
            """Fail as update_card does when the note keeps changing."""
            raise card.CardError("changed twice while writing")

        monkeypatch.setattr(card, "update_card", refuse_update)

        code = run_card(projects, "attach", KEY, str(photo))

        assert code == 1
        attachment = projects / "DevEnv" / "attachments" / f"{KEY} photo.jpg"
        assert not attachment.exists()
        assert photo.read_bytes() == b"made-up image bytes"


class TestList:
    """The list prints one line per card, for people and for scripts."""

    def test_list_tabs_keeps_whole_title(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """With --tabs, a long title stays whole and the project shows.

        The hub's board draws rows from these lines on screens of any
        width. A title cut to fit 60 characters stays cut on a wide
        screen, and a missing project leaves the row without its tag.
        """
        title = "Show subagent tokens and cost in the sidebar"
        run_card(projects, "new", "DevEnv", title, "--by", "user")
        capsys.readouterr()

        code = run_card(projects, "list", "--tabs")

        assert code == 0
        first_line = capsys.readouterr().out.splitlines()[0]
        assert first_line == f"{NEW_KEY}\tdraft\tbuild\tDevEnv\t{title}"

    def test_list_shows_both_shapes_without_plans(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card list shows old and new card keys, and not a plan's.

        Older cards keep their old keys, so the board must still show
        them. A plan made for a card has the same name shape; listed as
        a card, it would add a row for every plan.
        """
        write_card(projects, KEY, "draft")
        write_card(projects, "PDEV-005", "draft", title="Build the retries")
        (projects / "DevEnv" / "PDEV-005 Plan the retries.md").write_text(PLAN)

        code = run_card(projects, "list", "--tabs")

        assert code == 0
        assert capsys.readouterr().out == (
            "PDEV-005\tdraft\tbuild\tDevEnv\tBuild the retries\n"
            "PDEV-KB-001\tdraft\tbuild\tDevEnv\tFix some work\n"
            "2 cards\n"
        )

    def test_list_json_gives_whole_card_fields(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card list --json gives whole titles, sessions and question lists.

        The hub reads every card's state, and every open question, from
        this one call. A cut title, a lost session or a missing item
        would put the wrong state on the hub's board.
        """
        title = "Show subagent tokens and cost in the sidebar"
        path = write_card(projects, KEY, "active", title=title)
        path.write_text(
            card.set_field(path.read_text(), "sessions", ["opencode ses_abc"]),
        )
        capsys.readouterr()

        code = run_card(projects, "list", "--json")

        assert code == 0
        assert json.loads(capsys.readouterr().out) == [
            {
                "key": KEY,
                "title": title,
                "stage": "active",
                "kind": "build",
                "project": "DevEnv",
                "stage_date": "2026-11-06",
                "plans": [],
                "prs": [],
                "sessions": ["opencode ses_abc"],
                "open_questions": [],
            },
        ]


class TestBoard:
    """The board shows what waits on the user, oldest first."""

    def test_board_lists_in_review_oldest_first(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """In-review cards are listed by the day they entered in-review.

        The user clears the review queue from the top. Sorting by key
        or creation date would bury work that has waited longest.
        """
        write_card(
            projects,
            "PDEV-KB-001",
            "in-review",
            stage_date="2026-11-05",
        )
        write_card(
            projects,
            "PDEV-KB-002",
            "in-review",
            stage_date="2026-11-01",
        )

        code = run_card(projects, "board")

        assert code == 0
        out = capsys.readouterr().out
        assert out.index("PDEV-KB-002 2026-11-01") < out.index(
            "PDEV-KB-001 2026-11-05",
        )

    def test_board_prints_each_stage_limit(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """The board counts active and in-review against their own limits.

        The hub reads this line to tell the user a stage is over its
        limit. A combined count would hide a long review queue.
        """
        write_card(projects, "PDEV-KB-001", "active")
        write_card(projects, "PDEV-KB-002", "in-review")
        write_card(projects, "PDEV-KB-003", "in-review")

        code = run_card(projects, "board")

        assert code == 0
        assert "active 1 of 8, in-review 2 of 6\n" in capsys.readouterr().out

    def test_board_prints_blocked_lines_hub_reads(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """The board prints the blocked lines the hub parses.

        hub brief reads the count line and the list, longest wait first
        with days and the newest reason, and the user may read it out in
        a standup. Changed words drop the cards from the brief, and an
        old reason sends the user to chase the wrong team.
        """
        write_card(
            projects,
            "PDEV-KB-001",
            "in-review",
            stage_date="2026-11-02",
        )
        path = write_card(projects, "PDEV-KB-003", "in-review")
        for target, why in [
            ("blocked", ["--why", "docs team's review"]),
            ("in-review", []),
            ("blocked", ["--why", "platform team's review"]),
        ]:
            run_card(
                projects,
                "move",
                "PDEV-KB-003",
                target,
                *why,
                "--by",
                "hub",
            )
        path.write_text(
            card.set_field(path.read_text(), "stage_date", "2026-10-30"),
        )
        write_card(projects, "PDEV-KB-009", "blocked")
        capsys.readouterr()

        code = run_card(projects, "board")

        assert code == 0
        assert capsys.readouterr().out == (
            "draft 0  todo 0  active 0\n"
            "in-review 1  blocked 2\n"
            "done 0  dropped 0\n"
            "active 0 of 8, in-review 1 of 6\n"
            "blocked 2, oldest 8 days\n"
            "\n"
            "IN-REVIEW, oldest first (1)\n"
            "- PDEV-KB-001 2026-11-02\n"
            "  Fix some work\n"
            "\n"
            "BLOCKED, oldest first (2)\n"
            "- PDEV-KB-003 8 days\n"
            "  Fix some work\n"
            "  platform team's review\n"
            "- PDEV-KB-009 1 day\n"
            "  Fix some work\n"
        )

    def test_board_json_gives_limits_and_waits(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card board --json gives the limits and each blocked card's wait.

        A script can then read the board without the text lines'
        wording. A missing limit, wait or reason would hide an over-full
        stage or send the user to chase the wrong team.
        """
        write_card(projects, "PDEV-KB-003", "in-review")
        run_card(
            projects,
            "move",
            "PDEV-KB-003",
            "blocked",
            "--why",
            "platform team's review",
            "--by",
            "hub",
        )
        capsys.readouterr()

        code = run_card(projects, "board", "--json")

        assert code == 0
        board = json.loads(capsys.readouterr().out)
        assert board["limits"] == {"active": 8, "in-review": 6}
        assert board["counts"]["blocked"] == 1
        [blocked] = board["blocked"]
        assert (blocked["key"], blocked["days"], blocked["reason"]) == (
            "PDEV-KB-003",
            0,
            "platform team's review",
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
            projects,
            "move",
            KEY,
            "in-review",
            "--by",
            "general-staff",
        )

        assert code == 0
        assert "stage: in-review\n" in path.read_text()

    def test_update_card_keeps_concurrent_hand_edit(
        self,
        projects: Path,
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
                    text.replace(
                        "## Open Questions",
                        "Hand edit.\n\n## Open Questions",
                    ),
                )
                # Some file systems keep coarse times, so force a change.
                os.utime(path, ns=(1, 1))
            return text + "- added by card\n"

        card.update_card(card.load_card(path), change)

        text = path.read_text()
        assert "Hand edit." in text
        assert text.endswith("- added by card\n")

    def test_check_reports_card_note_without_type(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card check names an old-shape card note that lost its type.

        Every other command skips a note without ``type: card``, so a
        card whose front matter was deleted by hand drops off the board
        with no message. card check is where the user finds it.
        """
        (projects / "DevEnv" / f"{KEY} Lost type.md").write_text(
            f"# {KEY} Lost type\n",
        )

        code = run_card(projects, "check")

        assert code == 1
        assert capsys.readouterr().out == (
            f"{KEY} Lost type.md has a card name but no type: card\n"
            "1 problems in 0 cards\n"
        )

    def test_check_reports_new_card_missing_type(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card check names a new-shape card that lost its type line.

        A plan shares a new card's name shape, so the check tells them
        apart by the card's stage field. Other commands skip the broken
        card, so card check is where the user finds it, and plans must
        not show up as problems beside it.
        """
        folder = projects / "DevEnv"
        (folder / f"{NEW_KEY} Lost type.md").write_text(
            "---\nstage: todo\nkind: build\n---\n\n# Lost type\n",
        )
        (folder / f"{NEW_KEY} Plan the retries.md").write_text(PLAN)
        (folder / f"{NEW_KEY} Old plan.md").write_text("# Old plan\n")

        code = run_card(projects, "check")

        assert code == 1
        assert capsys.readouterr().out == (
            f"{NEW_KEY} Lost type.md has a card name but no type: card\n"
            "1 problems in 0 cards\n"
        )


class TestVersionOneCard:
    """Cards made before the version 2 layout keep working."""

    def test_check_passes_without_new_sections(self, projects: Path) -> None:
        """card check passes an old card that has no Notes or Appendix.

        Cards made before version 2 have neither section. If card check
        asked for them, it would flag every old card and hide the real
        problems.
        """
        (projects / "DevEnv" / f"{KEY} Fix some work.md").write_text(V1_CARD)

        code = run_card(projects, "check")

        assert code == 0

    def test_show_gives_old_cards_every_section(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card show gives an old card every section, empty when missing.

        Scripts read old cards through card show --json and --section.
        An error or a missing entry for Notes or the Appendix would
        break them on every old card.
        """
        (projects / "DevEnv" / f"{KEY} Fix some work.md").write_text(V1_CARD)

        section_code = run_card(projects, "show", KEY, "--section", "notes")
        section_out = capsys.readouterr().out
        json_code = run_card(projects, "show", KEY, "--json")
        shown = json.loads(capsys.readouterr().out)

        assert (section_code, section_out) == (0, "## Notes\n\n")
        assert json_code == 0
        assert shown["sections"] == {
            "Context": "",
            "Acceptance Criteria": "- [ ] Uploads retry.",
            "Open Questions": "",
            "Decisions": "",
            "Notes": "",
            "Log": "- 2026-11-01 active: by user",
            "Appendix": "",
            "Risks": "- The bucket may be full.",
        }

    def test_note_appends_exact_log_lines(self, projects: Path) -> None:
        """card note adds a dated Log line and keeps an old card whole.

        Agents and the hub record each step in these lines. The write
        must also keep the sections a user added by hand to an old card.
        """
        path = projects / "DevEnv" / f"{KEY} Fix some work.md"
        path.write_text(V1_CARD)

        run_card(projects, "note", KEY, "bucket picked", "--by", "hub")

        assert path.read_text() == V1_CARD + (
            f"- {TODAY} note (hub): bucket picked\n"
        )


class TestNamingRules:
    """card makes and reads names with the rules in the rules file."""

    @pytest.mark.parametrize(
        "case",
        [
            pytest.param(case, id=f"{case['rule']}-{case['name']}")
            for case in CASES["make"]
        ],
    )
    def test_make_name_round_trips_shared_cases(
        self,
        case: dict[str, Any],
    ) -> None:
        """Each rule makes the name the shared cases list and reads it back.

        The shared cases let each program that names plans, branches and
        agents check that its names match card's. card name refuses a
        made name its rule cannot read, so a rule that fails the round
        trip makes card name refuse every use of it.
        """
        rule = card.find_rule(case["rule"])

        name = card.make_name(rule, case["fields"])

        assert name == case["name"]
        assert card.parse_name(rule, name) is not None

    @pytest.mark.parametrize(
        "case",
        [
            pytest.param(case, id=f"{case['rule']}-{case['name']}")
            for case in CASES["parse"]
        ],
    )
    def test_parse_name_reads_shared_cases(self, case: dict[str, Any]) -> None:
        """Each rule reads the fields the shared cases list, or nothing.

        card finds cards by reading their file names. Reading a plan or
        research note as a card, or missing a card, would change the
        list, the board and the next key.
        """
        rule = card.find_rule(case["rule"])

        assert card.parse_name(rule, case["name"]) == case.get("fields")

    def test_rules_file_defaults_to_checkout_copy(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card reads its checkout's rules when PDE_NAMES_FILE is unset.

        card changes with git pull, before the installer copies the
        rules file. Reading the checkout's copy keeps card and the hub
        board working in between, with rules that match card's code.
        """
        monkeypatch.delenv("PDE_NAMES_FILE")
        monkeypatch.setenv("HOME", str(tmp_path))

        code = card.main(["name", "agent", "PDEV-KB-012", "build"])

        assert code == 0
        assert capsys.readouterr().out == "pdev-kb-012-build\n"

    def test_name_prints_branch_from_values(
        self,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card name fills fields in order, and the last takes the rest.

        A branch made from this output must hold every VALUE in its
        field. A VALUE dropped or put in the wrong field could still
        print a branch the rule reads, so the fit check would not catch
        it.
        """
        code = card.main(["name", "branch", "feat", "retry", "uploads"])

        assert code == 0
        assert capsys.readouterr().out == "feat/retry-uploads\n"

    def test_name_refuses_value_rule_cannot_read(
        self,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card name refuses a name its own rule would not read back.

        A name made from card's output is used as a branch or agent
        name. A wrong branch type that still printed a name would make a
        branch pde-check-branch refuses, with no error from card.
        """
        code = card.main(["name", "branch", "nope", "retry"])

        assert code == 1
        assert capsys.readouterr().err.startswith(
            "card: nope/retry does not fit the branch rule",
        )

    def test_template_embeds_rule_patterns(self) -> None:
        """The Obsidian template reads names with the rules file's patterns.

        Cards made in Obsidian and by card share one number series. A
        template with its own copy of a pattern could take a number a
        card already holds. JavaScript rejects a ``(?P<name>`` group, so
        a template that kept one could not make a card at all. An
        unfilled ``_JSON`` placeholder would also fail only in Obsidian.
        """
        text = card.render_template()

        assert "_JSON" not in text
        literals = re.findall(r'new RegExp\(("(?:[^"\\]|\\.)*")\)', text)
        patterns = [json.loads(literal) for literal in literals]
        assert not any("(?P<" in pattern for pattern in patterns)
        assert [
            re.compile(pattern.replace("(?<", "(?P<")) for pattern in patterns
        ] == [
            card.find_rule("card_file").parse,
            card.find_rule("plan_file").parse,
        ]


class TestTitleRules:
    """Titles read like commit titles, with no label or key."""

    @pytest.mark.parametrize(
        ("title", "rules"),
        [
            pytest.param(
                "Uploads retry after the network drops for a minute or two",
                ["T1"],
                id="long",
            ),
            pytest.param(
                "photo wall: sort the albums and suggest better captions",
                ["T1", "T2", "T4"],
                id="lower-case-label",
            ),
            pytest.param(
                "Shopkeep: AI tags use Ledger, tested",
                ["T2"],
                id="label-first",
            ),
            pytest.param(
                "Photo app fixes from the spring album review: short"
                " captions, plain album names",
                ["T1", "T4"],
                id="late-colon",
            ),
            pytest.param(
                "Make photo album captions readable on a tiny phone",
                [],
                id="50-characters",
            ),
            pytest.param(
                "Make photo album captions readable on a tiny phones",
                ["T1"],
                id="51-characters",
            ),
            pytest.param("Hubs read the approval state", [], id="noun-first"),
            pytest.param("Fix: board rows drift", ["T2"], id="verb-label"),
            pytest.param(
                "Fix the DEMO-KB-004 board rows",
                ["T3"],
                id="card-key",
            ),
            pytest.param(
                "Close DEMO-41 after the merge",
                ["T3"],
                id="plan-key",
            ),
            pytest.param(
                "Fix the board rows (DEMO-KB-004)",
                ["T3"],
                id="wrapped-key",
            ),
            pytest.param("Fix board rows.", ["T4"], id="period"),
            pytest.param("Fix the #board rows", ["T4"], id="link-character"),
        ],
    )
    def test_check_title_fails_each_broken_rule(
        self,
        title: str,
        rules: list[str],
    ) -> None:
        """check_title fails exactly the rules a title breaks.

        The bad titles have the shapes of real cards that showed as
        labels on the board. A missed rule lets such a title back, and a
        rule that fires on a good title refuses an agent's good card.
        """
        findings = card.check_title(title)

        assert [finding.rule for finding in findings] == rules

    def test_new_refuses_title_listing_every_finding(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card new writes no card for a failing title and lists each finding.

        An agent fixes its title in one try only when the refusal names
        every broken rule. A card written anyway would put the bad title
        on the board.
        """
        code = run_card(
            projects,
            "new",
            "DevEnv",
            "uploads: retry after the network drops for a minute or two",
            "--by",
            "general-staff",
        )

        assert code == 1
        assert not list((projects / "DevEnv").glob("PDEV-*"))
        assert capsys.readouterr().err == (
            "card: title refused:\n"
            "  T1 length: 58 characters, at most 50\n"
            '  T2 label: "uploads:"\n'
            "  T4 form: starts with a lower-case letter\n"
            "Write it as a commit title of at most 50 characters, such as"
            ' "Retry failed uploads".\n'
        )

    def test_new_accepts_noun_title_without_type(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """An agent's card new takes any first word and needs no type.

        It prints only the card's name, so an agent that reads the
        output as the name gets nothing else.
        """
        code = run_card(
            projects,
            "new",
            "DevEnv",
            "Hubs read the approval state",
            "--by",
            "general-staff",
        )

        assert code == 0
        assert capsys.readouterr().out == (
            "PDEV-001 Hubs read the approval state\n"
        )

    def test_set_title_refuses_failing_title_unchanged(
        self,
        projects: Path,
    ) -> None:
        """card set refuses a failing title and leaves the note as it was.

        A partial change, such as a renamed file with the old heading,
        would leave the card's name and title apart.
        """
        path = write_card(projects, KEY, "draft")
        before = path.read_text()

        code = run_card(projects, "set", KEY, "title", "fix board rows.")

        assert code == 1
        assert path.read_text() == before

    def test_check_lists_open_cards_failing_titles(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """card check lists each failing rule of an open card's title.

        Staff find the titles to fix from this list. A done or dropped
        card keeps its title, so listing it would bury the open ones.
        """
        write_card(projects, "PDEV-KB-001", "draft", title="Some work.")
        write_card(projects, "PDEV-KB-002", "done", title="Some work.")
        write_card(projects, "PDEV-KB-003", "active")

        code = run_card(projects, "check")

        assert code == 1
        assert capsys.readouterr().out == (
            "PDEV-KB-001 T4 form: ends with a period\n1 problems in 3 cards\n"
        )

    @pytest.mark.parametrize(
        ("stage", "target", "flags", "criteria"),
        [
            pytest.param(
                "active",
                "todo",
                ["--why", "paused"],
                "",
                id="pause",
            ),
            pytest.param("draft", "todo", [], "- [ ] Done.", id="release"),
            pytest.param("draft", "active", [], "- [ ] Done.", id="chain"),
        ],
    )
    def test_move_to_todo_refuses_failing_title(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
        stage: str,
        target: str,
        flags: list[str],
        criteria: str,
    ) -> None:
        """A move into or through todo refuses a failing title.

        Workers start todo cards, so a bad title must stop before todo.
        """
        path = write_card(projects, KEY, stage, criteria, title="Some work.")
        before = path.read_text()

        code = run_card(projects, "move", KEY, target, *flags, "--by", "hub")

        assert code == 1
        assert path.read_text() == before
        assert capsys.readouterr().err.startswith("card: title refused:\n")


@pytest.fixture
def herdr_agents(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A herdr on PATH whose agent list is the returned JSON file.

    The file starts with no agents. A real program, unlike a patched
    subprocess.run, leaves undo.sh free to run with sh.
    """
    agents = tmp_path / "agents.json"
    agents.write_text('{"result": {"agents": []}}')
    herdr = tmp_path / "bin" / "herdr"
    herdr.parent.mkdir()
    herdr.write_text(f"#!/bin/sh\ncat {shlex.quote(str(agents))}\n")
    herdr.chmod(0o755)
    path = f"{herdr.parent}{os.pathsep}{os.environ['PATH']}"
    monkeypatch.setenv("PATH", path)
    return agents


def write_rekey_vault(projects: Path) -> None:
    """Open, done and dropped old-key cards, PDEV-410 with its plan, links.

    PDEV-410 is the highest number, so the open cards take PDEV-411 and
    PDEV-412. The first card has session ses_abc; the second links the
    first.
    """
    first = write_card(projects, KEY, "draft")
    first.write_text(
        card.set_field(first.read_text(), "sessions", ["opencode ses_abc"]),
    )
    second = write_card(
        projects,
        "PDEV-KB-002",
        "todo",
        title="Retry failed uploads",
    )
    second.write_text(second.read_text() + "See [[PDEV-KB-001]].\n")
    write_card(projects, "PDEV-KB-003", "done", title="Old done work")
    write_card(projects, "PDEV-KB-004", "dropped", title="Dropped idea")
    write_card(projects, "PDEV-410", "active", title="Build the retries")
    (projects / "DevEnv" / "PDEV-410 Plan the retries.md").write_text(PLAN)
    (projects / "DevEnv" / "Links.md").write_text(
        "[[PDEV-KB-001]], [[PDEV-KB-002 Retry failed uploads|uploads]]"
        " and [[PDEV-KB-003 Old done work]]\n",
    )


def read_tree(projects: Path) -> dict[str, bytes]:
    """Every file under Projects, by its path relative to Projects."""
    return {
        str(path.relative_to(projects)): path.read_bytes()
        for path in projects.rglob("*")
        if path.is_file()
    }


def format_agent_list(*agents: dict[str, Any]) -> str:
    """The JSON that herdr agent list prints while these agents run."""
    return json.dumps({"result": {"agents": list(agents)}})


def edit_before_write(monkeypatch: pytest.MonkeyPatch, note: Path) -> None:
    """Make rekey find one note edited by the user on its first write.

    The real write_safely then refuses that write, since the note's
    modified time moved on. Later writes go through.
    """
    real = card.write_safely

    def write(path: Path, *args: Any) -> bool:
        """Append a line before the note's first write, then write it."""
        if path == note and not path.read_text().endswith("edited\n"):
            with path.open("a") as handle:
                handle.write("edited\n")
            # A write within one clock tick of the read can keep the time.
            stat = path.stat()
            os.utime(path, ns=(stat.st_atime_ns, stat.st_mtime_ns + 10**9))
        return real(path, *args)

    monkeypatch.setattr(card, "write_safely", write)


@pytest.mark.usefixtures("herdr_agents")
class TestRekey:
    """card rekey gives open cards with old keys new keys."""

    def test_dry_run_prints_keys_writes_nothing(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """Without --apply, rekey prints old and new keys and writes nothing.

        The user reviews this list before running --apply. Done and
        dropped cards and cards that already have new keys must not be on
        it, and each note whose links change is named.
        """
        write_rekey_vault(projects)
        before = read_tree(projects)

        code = run_card(projects, "rekey")

        assert code == 0
        assert capsys.readouterr().out == (
            "PDEV-KB-001 PDEV-411\n"
            "PDEV-KB-002 PDEV-412\n"
            "relink DevEnv/Links.md\n"
            "relink DevEnv/PDEV-KB-002 Retry failed uploads.md\n"
            "2 cards to rekey, 2 notes to relink\n"
        )
        assert read_tree(projects) == before

    def test_apply_renames_cards_keeping_old_key(self, projects: Path) -> None:
        """--apply renames open cards and writes old_key and the new heading.

        A card without old_key cannot be found by the key in its Log and in
        other notes.
        """
        write_rekey_vault(projects)
        devenv = projects / "DevEnv"

        code = run_card(projects, "rekey", "--apply", "--user")

        assert code == 0
        assert not (devenv / "PDEV-KB-001 Fix some work.md").exists()
        text = (devenv / "PDEV-411 Fix some work.md").read_text()
        assert "\nold_key: PDEV-KB-001\n" in text
        assert "\n# PDEV-411 Fix some work\n" in text
        assert (devenv / "PDEV-412 Retry failed uploads.md").exists()

    def test_apply_relinks_only_open_cards(self, projects: Path) -> None:
        """--apply points links to open cards at their new names.

        A link left on the old name breaks in Obsidian, also in a card that
        was renamed itself. Done cards keep their names, so links to them
        stay.
        """
        write_rekey_vault(projects)
        devenv = projects / "DevEnv"

        code = run_card(projects, "rekey", "--apply", "--user")

        assert code == 0
        assert (devenv / "Links.md").read_text() == (
            "[[PDEV-411]], [[PDEV-412 Retry failed uploads|uploads]]"
            " and [[PDEV-KB-003 Old done work]]\n"
        )
        retry = (devenv / "PDEV-412 Retry failed uploads.md").read_text()
        assert "See [[PDEV-411]].\n" in retry

    def test_undo_script_restores_every_note(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """The printed undo.sh puts every renamed and relinked note back.

        It is the user's way back from a rekey they do not want, so it
        must restore the notes that only had links changed too.
        """
        write_rekey_vault(projects)
        before = read_tree(projects)
        run_card(projects, "rekey", "--apply", "--user")

        undo = shlex.split(capsys.readouterr().out.splitlines()[-1])[-1]
        subprocess.run(["sh", undo], check=True)

        assert read_tree(projects) == before

    def test_undo_after_stop_keeps_later_edits(
        self,
        projects: Path,
        monkeypatch: pytest.MonkeyPatch,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """After a stop, undo puts back only the notes rekey wrote.

        The likeliest stop is the user editing a note during the run, and
        undo must not copy the old note over that edit.
        """
        write_rekey_vault(projects)
        before = read_tree(projects)
        retry = "DevEnv/PDEV-KB-002 Retry failed uploads.md"
        edit_before_write(monkeypatch, projects / retry)

        code = run_card(projects, "rekey", "--apply", "--user")
        undo = shlex.split(capsys.readouterr().out.splitlines()[-1])[-1]
        subprocess.run(["sh", undo], check=True)

        assert code == 1
        edited = {**before, retry: before[retry] + b"edited\n"}
        assert read_tree(projects) == edited

    def test_second_apply_finishes_stopped_run(
        self,
        projects: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """A second --apply after a stop renames the rest and fixes links.

        The first run renames PDEV-KB-001 and stops at PDEV-KB-002, then
        card new takes PDEV-412, so the second run gives PDEV-KB-002 the
        key PDEV-413. Each link must name the card it meant, never the new
        card, and PDEV-411 must not get yet another key.
        """
        write_rekey_vault(projects)
        devenv = projects / "DevEnv"
        retry = devenv / "PDEV-KB-002 Retry failed uploads.md"
        edit_before_write(monkeypatch, retry)
        first = run_card(projects, "rekey", "--apply", "--user")
        run_card(
            projects,
            "new",
            "DevEnv",
            "Add a retry button",
            "--by",
            "user",
        )

        code = run_card(projects, "rekey", "--apply", "--user")

        assert (first, code) == (1, 0)
        assert sorted(path.name for path in devenv.glob("PDEV-*")) == [
            "PDEV-410 Build the retries.md",
            "PDEV-410 Plan the retries.md",
            "PDEV-411 Fix some work.md",
            "PDEV-412 Add a retry button.md",
            "PDEV-413 Retry failed uploads.md",
            "PDEV-KB-003 Old done work.md",
            "PDEV-KB-004 Dropped idea.md",
        ]
        assert (devenv / "Links.md").read_text() == (
            "[[PDEV-411]], [[PDEV-413 Retry failed uploads|uploads]]"
            " and [[PDEV-KB-003 Old done work]]\n"
        )
        retry = devenv / "PDEV-413 Retry failed uploads.md"
        assert "See [[PDEV-411]].\n" in retry.read_text()

    def test_second_apply_finishes_stopped_relink(
        self,
        projects: Path,
        monkeypatch: pytest.MonkeyPatch,
    ) -> None:
        """A second --apply after a stop while relinking rewrites the links.

        The first run renames every card and stops at Links.md, its first
        link write, so the second run has no card left to rename. It must
        still rewrite the links, or they stay on old names while rekey
        reports success.
        """
        write_rekey_vault(projects)
        links = projects / "DevEnv" / "Links.md"
        edit_before_write(monkeypatch, links)
        first = run_card(projects, "rekey", "--apply", "--user")

        code = run_card(projects, "rekey", "--apply", "--user")

        assert (first, code) == (1, 0)
        assert links.read_text() == (
            "[[PDEV-411]], [[PDEV-412 Retry failed uploads|uploads]]"
            " and [[PDEV-KB-003 Old done work]]\n"
            "edited\n"
        )

    @pytest.mark.parametrize(
        ("argv", "herdr_output", "notes", "reason"),
        [
            pytest.param(
                ["--apply", "--user"],
                format_agent_list(
                    {
                        "name": "pdev-kb-002-build",
                        "agent": "opencode",
                        "agent_session": {"value": "ses_xyz"},
                    },
                ),
                {},
                "live workers on PDEV-KB-002 (pdev-kb-002-build)",
                id="worker-name",
            ),
            pytest.param(
                ["--apply", "--user"],
                format_agent_list(
                    {
                        "name": "worker-1",
                        "agent": "opencode",
                        "agent_session": {"value": "ses_abc"},
                    },
                ),
                {},
                "live workers on PDEV-KB-001 (worker-1)",
                id="worker-session",
            ),
            pytest.param(
                ["--apply", "--user"],
                '{"error": "no server"}',
                {},
                "live workers are unknown",
                id="herdr-failed",
            ),
            pytest.param(
                ["--apply", "--user"],
                format_agent_list(),
                {"Other/PDEV-KB-001 Kanban demo.md": HAND_EDITED},
                "two cards hold PDEV-KB-001",
                id="shared-key",
            ),
            pytest.param(
                ["--apply", "--user"],
                format_agent_list(),
                {
                    "Other/PDEV-400 Kanban demo.md": HAND_EDITED.replace(
                        "prs: []\n",
                        "prs: []\nold_key: PDEV-KB-002\n",
                    ),
                },
                "two cards hold PDEV-KB-002",
                id="old-key-reused",
            ),
            pytest.param(
                ["--apply", "--user"],
                format_agent_list(),
                {
                    "Other/PDEV-KB-005 Kanban demo.md": HAND_EDITED.replace(
                        "prs: []\n",
                        "prs: []\nold_key: PDEV-KB-005\n",
                    ),
                },
                "PDEV-KB-005 written but not renamed",
                id="stuck-card",
            ),
            pytest.param(
                ["--apply"],
                format_agent_list(),
                {},
                "--apply needs --user",
                id="no-user",
            ),
        ],
    )
    def test_refuses_and_changes_nothing(
        self,
        projects: Path,
        herdr_agents: Path,
        capsys: pytest.CaptureFixture[str],
        argv: list[str],
        herdr_output: str,
        notes: dict[str, str],
        reason: str,
    ) -> None:
        """Rekey refuses, with its reason, before it changes any note.

        A running worker on a card must end first, Herdr without an agent
        list may hide one, two cards with one key cannot both get the new
        one, a card written but not renamed would give its number to
        another card, and only the user runs --apply.
        """
        write_rekey_vault(projects)
        for name, text in notes.items():
            (projects / name).parent.mkdir(exist_ok=True)
            (projects / name).write_text(text)
        herdr_agents.write_text(herdr_output)
        before = read_tree(projects)

        code = run_card(projects, "rekey", *argv)

        assert code == 1
        assert reason in capsys.readouterr().err
        assert read_tree(projects) == before


MERGED_PR = "https://github.com/me/repo/pull/7"
OTHER_PR = "https://github.com/me/repo/pull/8"


@pytest.fixture
def pr_states(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """A gh on PATH that prints each PR's state from the returned file.

    Each line of the file is ``URL STATE``, and the file starts empty.
    For a URL not in the file, gh fails, as it does for a PR it cannot
    find.
    """
    states = tmp_path / "pr-states.txt"
    states.write_text("")
    gh = tmp_path / "gh-bin" / "gh"
    gh.parent.mkdir()
    script = """#!/bin/sh
awk -v url="$3" '$1 == url {print $2; found = 1} END {exit !found}' STATES \\
    || { echo "no pull request $3" >&2; exit 1; }
"""
    gh.write_text(script.replace("STATES", shlex.quote(str(states))))
    gh.chmod(0o755)
    path = f"{gh.parent}{os.pathsep}{os.environ['PATH']}"
    monkeypatch.setenv("PATH", path)
    return states


def link_prs(path: Path, *urls: str) -> None:
    """Set a card note's prs field to the URLs, as card link would."""
    path.write_text(card.set_field(path.read_text(), "prs", list(urls)))


@pytest.mark.usefixtures("pr_states")
class TestMerged:
    """card merged moves the cards of a merged PR to done, or logs why not."""

    def test_moves_card_through_review_to_done(
        self,
        projects: Path,
        pr_states: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """A merge moves an active card to done once all its PRs merged.

        The merge is the user's approval, so the Log shows the user's move
        through in-review and names the merged PR.
        """
        path = write_card(projects, KEY, "active", "- [x] Done.")
        link_prs(path, MERGED_PR, OTHER_PR)
        pr_states.write_text(f"{OTHER_PR} MERGED\n")

        code = run_card(projects, "merged", MERGED_PR)

        assert code == 0
        text = path.read_text()
        assert f"stage: done\nstage_date: {TODAY}\n" in text
        assert read_log(text).endswith(
            f"- {TODAY} in-review: by card merged for the user\n"
            f"- {TODAY} done: by card merged for the user."
            f" {MERGED_PR} merged\n",
        )
        out = capsys.readouterr().out
        assert out == f"{KEY} active -> in-review -> done\n"

    def test_matches_pr_in_any_spelling_only(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """A merge moves the card linking its PR and no card linking another.

        Done is final, so a card moved by another PR's merge would close
        unfinished work. The argument and the card each spell the PR
        another way, and must still match.
        """
        path = write_card(projects, KEY, "active", "- [x] Done.")
        link_prs(path, "http://GitHub.com/Me/Repo/pull/7/files")
        others = [
            write_card(projects, "PDEV-KB-002", "active", "- [x] Done."),
            write_card(projects, "PDEV-KB-003", "active", "- [x] Done."),
        ]
        link_prs(others[0], "https://github.com/me/repo/pull/70")
        link_prs(others[1], "https://github.com/me/other/pull/7")
        before = [other.read_text() for other in others]
        spelled = "HTTPS://www.github.com/me/REPO/pull/7#x"

        code = run_card(projects, "merged", spelled)

        assert code == 0
        assert "stage: done\n" in path.read_text()
        assert [other.read_text() for other in others] == before
        out = capsys.readouterr().out
        assert out == f"{KEY} active -> in-review -> done\n"

    @pytest.mark.parametrize(
        ("criteria", "states", "reason"),
        [
            pytest.param(
                "- [x] Done.",
                f"{OTHER_PR} OPEN\n",
                f"{OTHER_PR} is open",
                id="other-pr-open",
            ),
            pytest.param(
                "- [x] Done.",
                f"{OTHER_PR} CLOSED\n",
                f"{OTHER_PR} is closed",
                id="other-pr-closed",
            ),
            pytest.param(
                "- [x] Done.\n- [ ] Tested.",
                f"{OTHER_PR} MERGED\n",
                "1 unticked in Acceptance Criteria",
                id="criteria-unticked",
            ),
            pytest.param(
                "- [x] Done.\n- [/] Tested.\n* [ ] Read.",
                f"{OTHER_PR} MERGED\n",
                "2 unticked in Acceptance Criteria",
                id="criteria-other-marks",
            ),
        ],
    )
    def test_logs_why_card_stays(
        self,
        projects: Path,
        pr_states: Path,
        capsys: pytest.CaptureFixture[str],
        criteria: str,
        states: str,
        reason: str,
    ) -> None:
        """A card with work left keeps its stage, and its Log says what.

        Done is final, so moving such a card would close unfinished work
        with no further check. The note tells the user what still holds
        the card.
        """
        path = write_card(projects, KEY, "active", criteria)
        link_prs(path, MERGED_PR, OTHER_PR)
        pr_states.write_text(states)

        code = run_card(projects, "merged", MERGED_PR)

        assert code == 0
        text = path.read_text()
        assert "stage: active\n" in text
        assert read_log(text).endswith(
            f"- {TODAY} note (card merged): {MERGED_PR} merged,"
            f" but the card stays active: {reason}\n",
        )
        assert capsys.readouterr().out == f"{KEY} stays active: {reason}\n"

    def test_rerun_logs_why_card_stays_once(
        self,
        projects: Path,
        pr_states: Path,
    ) -> None:
        """A second run for the same merge leaves a held card unchanged.

        A user reruns the command after a gh failure or a timeout, and each
        rerun would otherwise add the same note to every held card.
        """
        path = write_card(projects, KEY, "active", "- [x] Done.")
        link_prs(path, MERGED_PR, OTHER_PR)
        pr_states.write_text(f"{OTHER_PR} OPEN\n")
        run_card(projects, "merged", MERGED_PR)
        before = path.read_text()

        code = run_card(projects, "merged", MERGED_PR)

        assert code == 0
        assert path.read_text() == before

    def test_moves_other_cards_when_gh_fails(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """One card's gh failure still lets the next card move, and exits 1.

        Nothing runs the command again on its own, so a card skipped after
        another card's failure would stay open until someone moves it by
        hand.
        """
        # KEY sorts first, so its failure comes before the other card.
        failing = write_card(projects, KEY, "active", "- [x] Done.")
        link_prs(failing, MERGED_PR, OTHER_PR)
        path = write_card(projects, "PDEV-KB-002", "active", "- [x] Done.")
        link_prs(path, MERGED_PR)
        before = failing.read_text()

        code = run_card(projects, "merged", MERGED_PR)

        assert code == 1
        assert failing.read_text() == before
        assert "stage: done\n" in path.read_text()
        out = capsys.readouterr().out
        assert out == "PDEV-KB-002 active -> in-review -> done\n"

    @pytest.mark.parametrize("stage", ["done", "dropped"])
    def test_skips_done_or_dropped_exits_zero(
        self,
        projects: Path,
        capsys: pytest.CaptureFixture[str],
        stage: str,
    ) -> None:
        """A merge that no open card links changes nothing and exits 0.

        A done card is final and a dropped one was given up, so a later
        merge must not write to either.
        """
        path = write_card(projects, KEY, stage, "- [x] Done.")
        link_prs(path, MERGED_PR)
        before = path.read_text()

        code = run_card(projects, "merged", MERGED_PR)

        assert code == 0
        assert path.read_text() == before
        assert capsys.readouterr().out == f"no open card links {MERGED_PR}\n"
