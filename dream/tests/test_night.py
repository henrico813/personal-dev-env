"""Tests that run whole nights with in-memory fake ports."""

import datetime
import json
from pathlib import Path

import pytest

from dream.config import Config, ConfigError, load_config
from dream.night import run_night
from dream.ports import Answer, Cost, SessionStats, SessionSummary, WorkItem

CARD_TEXT = (
    "export crashes when the board is empty."
    " the file is always called export.csv"
)
GOOD_A = "\n".join(
    json.dumps(row)
    for row in [
        {
            "kind": "criterion",
            "text": "An empty export works.",
            "based_on": "export crashes when the board is empty",
        },
        {
            "kind": "criterion",
            "text": "Names differ.",
            "based_on": "the file is always called export.csv",
        },
        {
            "kind": "question",
            "text": "Which name?",
            "based_on": "the file is always called export.csv",
        },
    ]
)
GOOD_B = json.dumps(
    {
        "fact": "F1",
        "finding": "Shell output was re-read 3,908,032 tokens.",
        "fix": "Use head.",
    }
)


def item(key: str, created: str, has_criteria: bool = False) -> WorkItem:
    """A draft card with the sample text."""
    return WorkItem(
        key,
        key,
        key,
        datetime.date.fromisoformat(created),
        CARD_TEXT,
        has_criteria,
    )


def session(sid: str, cloud: int, idle: int = 120) -> SessionSummary:
    """A finished large session."""
    return SessionSummary(sid, sid, cloud, cloud, idle)


class FakeItems:
    """Work items held in memory."""

    name = "fake-items"

    def __init__(self, items: list[WorkItem]):
        """Hold the items to list."""
        self.items = items

    def list_drafts(self) -> list[WorkItem]:
        """Return the held items."""
        return self.items


class FakeSessions:
    """Sessions held in memory, each with one large shell cost."""

    name = "fake-sessions"

    def __init__(self, summaries: list[SessionSummary]):
        """Hold the sessions to list."""
        self.summaries = summaries

    def list_large(
        self, since_hours: int, min_cloud_tokens: int
    ) -> list[SessionSummary]:
        """Return the held sessions."""
        return self.summaries

    def read_stats(self, summary: SessionSummary) -> SessionStats:
        """Return one shell cost re-read 3,908,032 tokens."""
        return SessionStats(
            summary, "m", (Cost("tool", "shell", 52993, 3908032, 0, 116),)
        )


class FakeModel:
    """A model that answers from a list, then repeats the last answer."""

    name = "fake-model"

    def __init__(self, *answers: str):
        """Hold the answers after the probe's OK; "" is a failed call."""
        self.answers = ["OK", *answers]
        self.prompts: list[str] = []

    def ask(self, prompt: str, timeout: int) -> Answer:
        """Return the next answer."""
        self.prompts.append(prompt)
        text = (
            self.answers.pop(0) if len(self.answers) > 1 else self.answers[0]
        )
        return Answer(text, 700, 1.0, "" if text else "http 500")


def night_config(
    tmp_path: Path, model: FakeModel, items=None, sessions=None
) -> Config:
    """A config over fake ports, staging under ``tmp_path``."""
    return Config(
        state_dir=tmp_path / "state",
        stop_starting_at=datetime.time(6, 0),
        work_items=FakeItems(items or []),
        sessions=FakeSessions(sessions or []),
        model=model,
        model_name="fake",
        items_per_night=3,
        sessions_per_night=10,
        min_cloud_tokens=1,
    )


def at(hour: int, minute: int = 0):
    """A clock fixed at ``hour``:``minute`` on the demo night."""
    return lambda: datetime.datetime(2026, 11, 14, hour, minute)


def records(config: Config) -> list[dict]:
    """The night's run records."""
    return [
        json.loads(line)
        for line in (config.state_dir / "runs.jsonl").read_text().splitlines()
    ]


class TestRunNight:
    """``run_night`` from probe to morning report."""

    def test_run_night_reports_results_then_done(self, tmp_path: Path) -> None:
        """Passing and failing items both reach a report ending in done.

        The user reads the morning report first; one bad item must not
        hide the rest or stop the night.
        """
        model = FakeModel(GOOD_A, "not json", "not json")
        config = night_config(
            tmp_path,
            model,
            items=[item("OLD-1", "2026-01-01"), item("NEW-2", "2026-02-01")],
        )

        assert run_night(config, "A", at(2, 30)) == 0

        report = (config.state_dir / "latest.md").read_text()
        assert report.endswith("done\n")
        assert "OLD-1:" in report and "proposals/OLD-1.md" in report
        assert "NEW-2: fail not-json" in report
        assert [
            (r["item"], r["result"], r["tries"]) for r in records(config)
        ] == [("OLD-1", "pass", 1), ("NEW-2", "fail", 2)]

    def test_night_skips_items_when_probe_fails(self, tmp_path: Path) -> None:
        """A dead model server ends the night after the probe, with a reason.

        Without the probe a dead server would fail every item one by one.
        """
        model = FakeModel()
        model.answers = [""]
        config = night_config(
            tmp_path, model, items=[item("OLD-1", "2026-01-01")]
        )

        run_night(config, None, at(2, 30))

        assert (
            "skipped: http 500" in (config.state_dir / "latest.md").read_text()
        )
        assert len(model.prompts) == 1

    def test_night_starts_no_item_after_deadline(self, tmp_path: Path) -> None:
        """At 06:00 the night stops starting items.

        The deadline keeps the model server free for the user's first
        sessions.
        """
        times = iter(
            [datetime.datetime(2026, 11, 14, 2, 30)]
            + [datetime.datetime(2026, 11, 14, 6, 1)] * 5
        )
        model = FakeModel(GOOD_A)
        config = night_config(
            tmp_path, model, items=[item("OLD-1", "2026-01-01")]
        )

        run_night(config, None, lambda: next(times))

        assert len(model.prompts) == 1
        assert (
            "form A stopped at 06:00"
            in (config.state_dir / "latest.md").read_text()
        )

    def test_night_writes_only_under_state_dir(self, tmp_path: Path) -> None:
        """Every file a night writes is in the staging folder.

        "Nothing reaches the card or issue" is the design's main promise.
        """
        model = FakeModel(GOOD_A, GOOD_B)
        config = night_config(
            tmp_path,
            model,
            items=[item("OLD-1", "2026-01-01")],
            sessions=[session("ses_a", 5)],
        )

        run_night(config, None, at(2, 30))

        written = sorted(
            str(p.relative_to(tmp_path))
            for p in tmp_path.rglob("*")
            if p.is_file()
        )
        assert written == [
            "state/digests/ses_a.facts.json",
            "state/digests/ses_a.jsonl",
            "state/latest.md",
            "state/proposals/OLD-1.md",
            "state/runs.jsonl",
        ]

    def test_night_keeps_existing_proposal(self, tmp_path: Path) -> None:
        """A second night never overwrites or redoes a staged proposal.

        The user may be reading or have judged it.
        """
        model = FakeModel(GOOD_A)
        config = night_config(
            tmp_path, model, items=[item("OLD-1", "2026-01-01")]
        )
        proposal = config.state_dir / "proposals" / "OLD-1.md"
        proposal.parent.mkdir(parents=True)
        proposal.write_text("judged: kept\n")

        run_night(config, "A", at(2, 30))

        assert proposal.read_text() == "judged: kept\n"
        assert len(model.prompts) == 1

    def test_sessions_rank_by_cloud_tokens(self, tmp_path: Path) -> None:
        """Form B takes the most cloud tokens first and skips running ones.

        Ranking by all tokens would spend the night on sessions that ran
        on the free model and cost nothing.
        """
        model = FakeModel(GOOD_B)
        sessions = [
            session("small", 10),
            session("big", 30),
            session("running", 99, idle=5),
        ]
        config = night_config(tmp_path, model, sessions=sessions)

        run_night(config, "B", at(2, 30))

        assert [r["item"] for r in records(config)] == ["big", "small"]


@pytest.mark.parametrize(
    ("change", "message"),
    [
        pytest.param({"sesions": {}}, "unknown key sesions", id="typo"),
        pytest.param(
            {"model": {"adapter": "cloud", "base_url": "u", "name": "n"}},
            "unknown adapter model.adapter",
            id="adapter",
        ),
        pytest.param(
            {"work_items": {"adapter": "pde-cards", "per_night": 9}},
            "per_night must be 1 to 3",
            id="cap",
        ),
    ],
)
def test_config_refuses_unusable_values(
    tmp_path: Path, change: dict, message: str
) -> None:
    """A typo, unknown adapter or raised cap fails with the key named.

    A typo that silently turned a form off would look like a quiet
    night; the caps are user decisions.
    """
    config = json.loads(
        (Path(__file__).parents[1] / "config.example.json").read_text()
    )
    config.update(change)
    path = tmp_path / "config.json"
    path.write_text(json.dumps(config))

    with pytest.raises(ConfigError, match=message):
        load_config(path)
