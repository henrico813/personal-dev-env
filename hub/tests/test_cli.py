"""Tests that run the packed file ``build.py`` writes against a fake herdr."""

import json
import os
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

BUILD = Path(__file__).parents[1] / "build.py"

# Answers each call from the agent list in $FAKE/agents.json and the
# screens in $FAKE/screens. Without agents.json it fails as Herdr does.
FAKE_HERDR = """\
import json, os, sys
from pathlib import Path

fake = Path(os.environ["FAKE"])
args = sys.argv[1:]
if args[:1] == ["--machine"]:
    args = args[2:]


def refuse(code, message):
    print(json.dumps({"error": {"code": code, "message": message}}))
    sys.exit(1)


if not (fake / "agents.json").exists():
    refuse("server_unavailable", "herdr server is not running")
agents = json.loads((fake / "agents.json").read_text())

if args[:2] == ["agent", "list"]:
    print(json.dumps({"result": {"agents": agents}}))
elif args[:2] == ["agent", "read"]:
    screen = fake / "screens" / args[2]
    print(screen.read_text() if screen.exists() else "")
else:
    print(f"unknown command {args}", file=sys.stderr)
    sys.exit(2)
"""

# Stands in for the card command, so no test reads the real vault. It
# prints the notes in $FAKE/cards, or fails when $FAKE/card-fails exists.
FAKE_CARD = """\
import os, sys
from pathlib import Path

fake = Path(os.environ["FAKE"])
if (fake / "card-fails").exists():
    print("card: no default vault", file=sys.stderr)
    sys.exit(1)
cards = sorted((fake / "cards").glob("*.md"))
if sys.argv[1:] == ["list", "--stage", "active"]:
    for card in cards:
        print(f"{card.stem} active   build    A card")
    print(f"{len(cards)} cards")
elif sys.argv[1] == "show":
    print((fake / "cards" / f"{sys.argv[2]}.md").read_text(), end="")
else:
    sys.exit(2)
"""

Run = Callable[..., subprocess.CompletedProcess]


def make_agent(name: str, status: str, **fields: object) -> dict:
    """Build one agent as ``herdr agent list`` shows it.

    Args:
        name: The agent's name. Its pane ID is ``w1:<name>``.
        status: Herdr's state, such as ``idle``.
        **fields: Fields to add or replace, such as ``state_labels``.

    Returns:
        The agent.
    """
    return {
        "name": name,
        "agent_status": status,
        "pane_id": f"w1:{name}",
        **fields,
    }


@pytest.fixture(scope="session")
def packed(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Return the file ``build.py`` packs, built once per test session.

    Every test runs this file rather than the source, so a packing
    mistake, such as losing the exit code, fails the whole suite.

    Args:
        tmp_path_factory: Makes the folder the file is built in.

    Returns:
        The packed file.
    """
    output = tmp_path_factory.mktemp("build") / "hub"
    subprocess.run([sys.executable, "-I", str(BUILD), str(output)], check=True)
    return output


@pytest.fixture
def fake_tools(tmp_path: Path) -> Path:
    """Return a folder of fake herdr and card commands and their answers.

    Args:
        tmp_path: The test's own folder.

    Returns:
        The folder, which also holds the fakes' answers.
    """
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for name, script in (("herdr", FAKE_HERDR), ("card", FAKE_CARD)):
        tool = bin_dir / name
        tool.write_text(f"#!{sys.executable}\n{script}")
        tool.chmod(0o755)
    (tmp_path / "screens").mkdir()
    (tmp_path / "cards").mkdir()
    return tmp_path


@pytest.fixture
def run_folder(tmp_path: Path) -> Path:
    """Return an empty run folder for the hub.

    Args:
        tmp_path: The test's own folder.

    Returns:
        The run folder.
    """
    folder = tmp_path / "run"
    folder.mkdir()
    return folder


@pytest.fixture
def hub(packed: Path, fake_tools: Path) -> Run:
    """Return a function that runs hub with the fake herdr.

    The hub's own pane is ``w1:kanban-hub``.

    Args:
        packed: The packed hub.
        fake_tools: The fakes' folder, whose ``bin`` goes first on PATH.

    Returns:
        The function, which takes hub's arguments.
    """

    def run(*args: str) -> subprocess.CompletedProcess:
        """Run the packed hub with the fake herdr.

        Args:
            *args: The arguments after ``hub``.

        Returns:
            The finished process, with its output as text.
        """
        env = os.environ | {
            "FAKE": str(fake_tools),
            "PATH": f"{fake_tools / 'bin'}{os.pathsep}{os.environ['PATH']}",
            "HERDR_PANE_ID": "w1:kanban-hub",
        }
        return subprocess.run(
            [str(packed), *args],
            env=env,
            capture_output=True,
            text=True,
            timeout=30,
        )

    return run


def write_card(
    fake_tools: Path, key: str, sessions: list[str], log: str
) -> None:
    """Add an active card for the fake card command to show.

    Args:
        fake_tools: The fakes' folder.
        key: The card's key, such as ``DEMO-KB-001``.
        sessions: Its session IDs, oldest first.
        log: The lines of its Log section.
    """
    items = "".join(f'  - "opencode {session}"\n' for session in sessions)
    note = (
        f"---\nstage: active\nsessions:\n{items}topics: []\n---\n\n"
        f"# {key} A card\n---\n\n## Log\n---\n\n{log}"
    )
    (fake_tools / "cards" / f"{key}.md").write_text(note)


def set_agents(fake_tools: Path, agents: list[dict]) -> None:
    """Make the fake herdr list these agents.

    Args:
        fake_tools: The fake herdr's folder.
        agents: The agents, as ``herdr agent list`` shows them.
    """
    (fake_tools / "agents.json").write_text(json.dumps(agents))


class TestStatus:
    """``hub status``, the hub's one look at all its sessions."""

    def test_status_shows_session_rows_except_hub(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """Each named session gets one row with what the hub acts on.

        The hub reads this table instead of listing files and panes by
        hand, so a wrong column makes it miss a report or ignore a
        session that waits on the user. Its own row would make it watch
        itself.
        """
        set_agents(
            fake_tools,
            [
                make_agent("kanban-hub", "working", pane_id="w1:kanban-hub"),
                make_agent(
                    "demo-kb-007-impl",
                    "idle",
                    state_labels={"idle": "needs you"},
                ),
                make_agent("docs-fix", "working"),
                {"agent_status": "idle", "pane_id": "w2:p1"},
            ],
        )
        (fake_tools / "screens" / "demo-kb-007-impl").write_text(
            "  esc interrupt   12.0K (1%)\n  ~/repo   98.3K (10%)\n"
        )
        (run_folder / "report-demo-kb-007-impl.md").write_text("Done.\n")

        result = hub("status", "--run", str(run_folder))

        assert result.returncode == 0
        sessions = result.stdout.split("\n\n")[0]
        rows = [line.split() for line in sessions.splitlines()]
        assert rows == [
            ["NAME", "STATE", "CTX", "CARD", "REPORT", "LABEL"],
            [
                "demo-kb-007-impl",
                "idle",
                "98.3K",
                "DEMO-KB-007",
                "unread",
                "needs",
                "you",
            ],
            ["docs-fix", "working", "-", "-", "-", "-"],
        ]

    def test_card_rows_show_session_and_waiter(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """Each active card shows whether its session waits on the user.

        Sessions that never report to the hub sat idle for many minutes
        before anyone noticed. A blocked session, an idle one outside
        the hub, a pane marked "needs you" and an unanswered needs-you
        note all wait on the user. An idle worker that reports to the
        hub waits on the hub.
        A card shows its newest live session.
        """
        set_agents(
            fake_tools,
            [
                make_agent(
                    "demo-kb-001-impl",
                    "blocked",
                    agent_session={"value": "ses_a"},
                ),
                make_agent(
                    "demo-kb-002-impl",
                    "idle",
                    agent_session={"value": "ses_b"},
                ),
                make_agent(
                    "demo-kb-002-old",
                    "done",
                    agent_session={"value": "ses_old"},
                ),
                make_agent(
                    "demo-kb-003-impl",
                    "idle",
                    agent_session={"value": "ses_c"},
                ),
                make_agent(
                    "demo-kb-004-impl",
                    "working",
                    agent_session={"value": "ses_d"},
                ),
                make_agent(
                    "demo-kb-006-impl",
                    "idle",
                    agent_session={"value": "ses_f"},
                    state_labels={"idle": "needs you"},
                ),
            ],
        )
        write_card(fake_tools, "DEMO-KB-001", ["ses_a"], "")
        write_card(fake_tools, "DEMO-KB-002", ["ses_old", "ses_b"], "")
        write_card(
            fake_tools,
            "DEMO-KB-003",
            ["ses_c"],
            "- 2026-11-06 needs-you: which?\n- 2026-11-06 answered: yes\n",
        )
        write_card(
            fake_tools,
            "DEMO-KB-004",
            ["ses_d"],
            "- 2026-11-06 answered: yes\n- 2026-11-06 needs-you: which?\n",
        )
        write_card(fake_tools, "DEMO-KB-005", ["ses_gone"], "")
        write_card(fake_tools, "DEMO-KB-006", ["ses_f"], "")
        for name in ("demo-kb-003-impl", "demo-kb-006-impl"):
            (run_folder / f"report-{name}.read").write_text("PR open.\n")

        result = hub("status", "--run", str(run_folder))

        cards = result.stdout.split("\n\n")[1]
        assert [line.split() for line in cards.splitlines()] == [
            ["CARD", "SESSION", "STATE", "WAITS"],
            ["DEMO-KB-001", "demo-kb-001-impl", "blocked", "you"],
            ["DEMO-KB-002", "demo-kb-002-impl", "idle", "you"],
            ["DEMO-KB-003", "demo-kb-003-impl", "idle", "hub"],
            ["DEMO-KB-004", "demo-kb-004-impl", "working", "you"],
            ["DEMO-KB-005", "-", "-", "-"],
            ["DEMO-KB-006", "demo-kb-006-impl", "idle", "you"],
        ]

    def test_status_exits_with_herdr_error(
        self, hub: Run, run_folder: Path
    ) -> None:
        """A failed herdr call exits 1 with Herdr's own message.

        The hub trusts the table only when the exit code is 0, so a 0
        here would make it act on an empty session list. Herdr's message
        tells it why, such as a stopped Herdr server.
        """
        result = hub("status", "--run", str(run_folder))

        assert result.returncode == 1
        assert result.stderr == "hub: herdr server is not running\n"

    def test_status_keeps_sessions_when_card_fails(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A failing card command still leaves the session table.

        Machines without the vault have no working card command, and
        the hub must still see its sessions there.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        (fake_tools / "card-fails").touch()

        result = hub("status", "--run", str(run_folder))

        assert result.returncode == 0
        assert "docs-fix" in result.stdout
        assert result.stdout.endswith("cards: card: no default vault\n")
