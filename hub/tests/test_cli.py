"""Tests that run the packed file ``build.py`` writes against a fake herdr."""

import json
import os
import re
import sqlite3
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

BUILD = Path(__file__).parents[1] / "build.py"

# Answers each call from files in $FAKE, logs it to calls.jsonl, and
# fails as Herdr does without agents.json. agents.json holds one agent
# list per agent list call, the last one repeating, so a test can change
# a session's state over time.
FAKE_HERDR = """\
import json, os, sys
from pathlib import Path

fake = Path(os.environ["FAKE"])
args = sys.argv[1:]
with (fake / "calls.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\\n")
if args[:1] == ["--machine"]:
    args = args[2:]


def refuse(code, message):
    print(json.dumps({"error": {"code": code, "message": message}}))
    sys.exit(1)


if not (fake / "agents.json").exists():
    refuse("server_unavailable", "herdr server is not running")
lists = json.loads((fake / "agents.json").read_text())
count_file = fake / "list-count"
count = int(count_file.read_text()) if count_file.exists() else 0
if args[:2] == ["agent", "list"]:
    count_file.write_text(str(count + 1))
agents = lists[min(count, len(lists) - 1)]
by_name = {agent["name"]: agent for agent in agents if agent.get("name")}
# agent get takes a pane ID too, as the hub's own pane is looked up.
by_name |= {agent["pane_id"]: agent for agent in agents}

if args[:2] == ["agent", "list"]:
    print(json.dumps({"result": {"agents": agents}}))
elif args[:2] == ["agent", "get"]:
    if args[2] not in by_name:
        refuse("agent_not_found", f"agent target {args[2]} not found")
    print(json.dumps({"result": {"agent": by_name[args[2]]}}))
elif args[:2] == ["agent", "read"]:
    screen = fake / "screens" / args[2]
    print(screen.read_text() if screen.exists() else "")
elif args[:2] == ["agent", "prompt"]:
    refusal = fake / "prompt-refusal.json"
    if refusal.exists():
        refuse(**json.loads(refusal.read_text()))
    print(json.dumps({"result": {}}))
elif args[:2] in (["pane", "report-metadata"], ["agent", "start"]):
    print(json.dumps({"result": {}}))
elif args[:2] == ["tab", "create"]:
    print(json.dumps({"result": {"root_pane": {"pane_id": "w1:new"}}}))
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
        **fields: Fields to add or replace, such as ``focused``.

    Returns:
        The agent, not focused and at state change 1 unless changed.
    """
    return {
        "name": name,
        "agent_status": status,
        "pane_id": f"w1:{name}",
        "focused": False,
        "state_change_seq": 1,
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
        The folder, which also holds the fakes' answers and call log.
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

    The hub's own pane is ``w1:kanban-hub``, and checks run every
    hundredth of a second so ``hub next`` tests end quickly. HOME is
    the fakes' folder, so no test reads the real OpenCode database.

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
            "HOME": str(fake_tools),
            "HUB_POLL_SECONDS": "0.01",
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


def set_agents(fake_tools: Path, *lists: list[dict]) -> None:
    """Make the fake herdr answer each agent list call with the next list.

    Args:
        fake_tools: The fake herdr's folder.
        *lists: One agent list per call. The last repeats.
    """
    (fake_tools / "agents.json").write_text(json.dumps(list(lists)))


def read_calls(fake_tools: Path) -> list[list[str]]:
    """Read every herdr call the fake logged.

    Args:
        fake_tools: The fake herdr's folder.

    Returns:
        The arguments of each call, in order.
    """
    log = fake_tools / "calls.jsonl"
    if not log.exists():
        return []
    return [json.loads(line) for line in log.read_text().splitlines()]


def read_prompts(fake_tools: Path) -> list[list[str]]:
    """Read the herdr agent prompt calls the fake logged.

    Args:
        fake_tools: The fake herdr's folder.

    Returns:
        The arguments of each prompt call, in order.
    """
    return [call for call in read_calls(fake_tools) if "prompt" in call]


def refuse_prompts(fake_tools: Path, code: str, message: str) -> None:
    """Make the fake herdr answer every prompt with a Herdr error.

    Args:
        fake_tools: The fake herdr's folder.
        code: Herdr's error code, such as ``agent_blocked``.
        message: Herdr's message.
    """
    refusal = {"code": code, "message": message}
    (fake_tools / "prompt-refusal.json").write_text(json.dumps(refusal))


class TestStatus:
    """``hub status``, the hub's one look at all its sessions."""

    def test_status_shows_session_rows_except_hub(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """Each named session gets one row with what the hub acts on.

        The hub reads this table instead of listing files and panes by
        hand, so a wrong column makes it miss a report, send twice, or
        ignore a session that waits on the user. Its own row would make
        it watch itself.
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
        (run_folder / "send-docs-fix.json").write_text("{}\n")

        result = hub("status", "--run", str(run_folder))

        assert result.returncode == 0
        sessions = result.stdout.split("\n\n")[0]
        rows = [line.split() for line in sessions.splitlines()]
        assert rows == [
            ["NAME", "STATE", "CTX", "CARD", "REPORT", "SEND", "LABEL"],
            [
                "demo-kb-007-impl",
                "idle",
                "98.3K",
                "DEMO-KB-007",
                "unread",
                "-",
                "needs",
                "you",
            ],
            ["docs-fix", "working", "-", "-", "-", "held", "-"],
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


class TestSend:
    """``hub send``, the one way the hub messages a session."""

    def test_send_prompts_free_session_at_once(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A free session gets the message once, word for word.

        A message changed by a shell would arrive garbled, and a second
        prompt would repeat it.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        message = 'Rebase on "main"; then reply with $HOME as typed.'

        result = hub("send", "--run", str(run_folder), "docs-fix", message)

        assert result.returncode == 0
        assert result.stdout == "sent docs-fix\n"
        prompts = read_prompts(fake_tools)
        assert len(prompts) == 1
        assert prompts[0][:4] == ["agent", "prompt", "docs-fix", message]
        assert list(run_folder.iterdir()) == []

    def test_send_keeps_message_when_prompt_stalls(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A stalled prompt leaves the message marked unsure.

        Herdr reports a stall after the text went in, so the session
        may have it. Without the mark the hub would send it again and
        the session would get it twice.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        refuse_prompts(fake_tools, "agent_prompt_stalled", "prompt stalled")

        result = hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")

        assert result.returncode == 1
        sending = run_folder / "send-docs-fix.sending"
        assert json.loads(sending.read_text())["message"] == "Rebase."

    @pytest.mark.parametrize(
        "agent",
        [
            pytest.param(make_agent("docs-fix", "working"), id="working"),
            pytest.param(
                make_agent("docs-fix", "idle", focused=True),
                id="user-in-pane",
            ),
        ],
    )
    def test_send_holds_message_when_session_busy(
        self, hub: Run, fake_tools: Path, run_folder: Path, agent: dict
    ) -> None:
        """A busy session's message waits in the run folder.

        A prompt mid-turn gets lost in that turn, and one in a focused
        pane mixes with what the user types.
        """
        set_agents(fake_tools, [agent])

        result = hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")

        assert result.returncode == 0
        assert result.stdout.startswith("held docs-fix")
        assert read_prompts(fake_tools) == []
        held = json.loads((run_folder / "send-docs-fix.json").read_text())
        assert held["message"] == "Rebase."

    @pytest.mark.parametrize("suffix", ["json", "sending"])
    def test_send_refuses_while_earlier_message_waits(
        self, hub: Run, fake_tools: Path, run_folder: Path, suffix: str
    ) -> None:
        """A second message to a session with one waiting exits 2.

        Two waiting messages would both go out once the session is
        free. A ``.sending`` file means the last send stopped part way,
        so the hub must check the pane before sending again.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        waiting = run_folder / f"send-docs-fix.{suffix}"
        waiting.write_text("earlier\n")

        result = hub("send", "--run", str(run_folder), "docs-fix", "Again.")

        assert result.returncode == 2
        assert waiting.read_text() == "earlier\n"
        assert read_prompts(fake_tools) == []


class TestNext:
    """``hub next``, which the hub keeps running to learn of news."""

    def test_next_prints_unread_report(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """An unread report ends the wait with the session's name.

        This line is what starts the hub's turn when a worker finishes.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        (run_folder / "report-docs-fix.md").write_text("PR opened.\n")
        (run_folder / "report-old.read").write_text("Handled.\n")

        result = hub("next", "--run", str(run_folder))

        assert result.returncode == 0
        assert result.stdout == "report docs-fix\n"

    def test_next_prints_each_blocked_state_once(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A session that stays blocked wakes the hub only once.

        Printing the same blocked state again would return at once on
        every restart, so the hub would loop on it. A later block, with
        a new state change number, must wake the hub again.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "blocked")])
        first = hub("next", "--run", str(run_folder))
        again = hub("next", "--run", str(run_folder), "--timeout", "0.1")
        set_agents(
            fake_tools, [make_agent("docs-fix", "blocked", state_change_seq=2)]
        )
        later = hub("next", "--run", str(run_folder), "--timeout", "0.1")

        assert first.stdout == "blocked docs-fix\n"
        assert again.stdout == "timeout\n"
        assert later.stdout == "blocked docs-fix\n"

    def test_next_sends_held_message_once_free(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A held message goes out once its session's turn ends.

        Sending it mid-turn would lose it, and sending it on every
        check would repeat it.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "working")])
        hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")
        set_agents(
            fake_tools,
            [make_agent("docs-fix", "working")],
            [make_agent("docs-fix", "idle")],
        )

        result = hub("next", "--run", str(run_folder), "--timeout", "0.2")

        assert result.stdout == "timeout\n"
        prompts = read_prompts(fake_tools)
        assert len(prompts) == 1
        assert prompts[0][:4] == ["agent", "prompt", "docs-fix", "Rebase."]
        calls = read_calls(fake_tools)
        sent_at = calls.index(prompts[0])
        assert calls[:sent_at].count(["agent", "list"]) == 2
        assert list(run_folder.glob("send-*")) == []

    def test_next_sends_remote_message_through_machine(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A held message for another machine goes through that machine.

        Without ``--machine`` the message would go to a session of the
        same name on this machine, or nowhere.
        """
        set_agents(fake_tools, [make_agent("notes-fix", "working")])
        hub(
            "--machine",
            "laptop",
            "send",
            "--run",
            str(run_folder),
            "notes-fix",
            "Pull.",
        )
        set_agents(fake_tools, [make_agent("notes-fix", "idle")])

        result = hub("next", "--run", str(run_folder), "--timeout", "0.1")

        assert result.stdout == "timeout\n"
        prompts = read_prompts(fake_tools)
        assert len(prompts) == 1
        assert prompts[0][:6] == [
            "--machine",
            "laptop",
            "agent",
            "prompt",
            "notes-fix",
            "Pull.",
        ]

    @pytest.mark.parametrize(
        ("code", "line", "left"),
        [
            pytest.param(
                "agent_blocked",
                "failed docs-fix: Herdr said no\n",
                "send-docs-fix.failed",
                id="refused",
            ),
            pytest.param(
                "agent_prompt_stalled",
                "unsure docs-fix: Herdr said no\n",
                "send-docs-fix.sending",
                id="stalled",
            ),
        ],
    )
    def test_next_prints_send_problem_keeping_message(
        self,
        hub: Run,
        fake_tools: Path,
        run_folder: Path,
        code: str,
        line: str,
        left: str,
    ) -> None:
        """A held message that goes wrong wakes the hub and keeps its text.

        A refusal means the message did not arrive, so the hub sends it
        again. A stall comes after the text went in, so it is marked
        unsure and the hub checks the pane first instead of sending it
        twice.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "working")])
        hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        refuse_prompts(fake_tools, code, "Herdr said no")

        result = hub("next", "--run", str(run_folder))

        assert result.stdout == line
        kept = json.loads((run_folder / left).read_text())
        assert kept["message"] == "Rebase."
        assert not (run_folder / "send-docs-fix.json").exists()


class TestRead:
    """``hub read``, how the hub takes in a session's report."""

    def test_read_prints_archives_and_lists_unread(
        self, hub: Run, run_folder: Path
    ) -> None:
        """The report is printed and archived, and other unread ones named.

        The hub acts on what this prints, and a report left unarchived
        wakes ``hub next`` again, so the hub would handle it twice. The
        ``unread`` line lets it read the next report in the same step.
        """
        (run_folder / "report-docs-fix.md").write_text("PR opened.\n")
        (run_folder / "report-notes-fix.md").write_text("Rebased.\n")
        (run_folder / "report-old.1200.read").write_text("Handled.\n")

        result = hub("read", "--run", str(run_folder), "docs-fix")

        assert result.returncode == 0
        assert result.stdout == "PR opened.\nunread: notes-fix\n"
        archives = list(run_folder.glob("report-docs-fix.*"))
        assert len(archives) == 1
        assert re.fullmatch(r"report-docs-fix\.\d{4}\.read", archives[0].name)
        assert archives[0].read_text() == "PR opened.\n"

    def test_read_keeps_reports_from_same_minute(
        self, hub: Run, run_folder: Path
    ) -> None:
        """A second report read soon after the first gets its own archive.

        A session can write a new report within a minute of the last,
        such as a question right after a plan. Reusing the first archive
        name would replace it, and the first report would be lost.
        """
        report = run_folder / "report-docs-fix.md"
        report.write_text("Plan ready.\n")
        hub("read", "--run", str(run_folder), "docs-fix")
        report.write_text("Question.\n")

        hub("read", "--run", str(run_folder), "docs-fix")

        archives = run_folder.glob("report-docs-fix.*.read")
        texts = sorted(path.read_text() for path in archives)
        assert texts == ["Plan ready.\n", "Question.\n"]

    def test_read_logs_note_with_archive_time(
        self, hub: Run, run_folder: Path
    ) -> None:
        """The note goes to the end of notes.md with the read time.

        The user reads notes.md to learn what the hub did, and the time
        ties each line to its archived report.
        """
        (run_folder / "notes.md").write_text("- 1200 earlier line\n")
        (run_folder / "report-docs-fix.md").write_text("PR opened.\n")

        hub(
            "read",
            "--run",
            str(run_folder),
            "docs-fix",
            "--note",
            "docs-fix opened #9.",
        )

        archive = next(run_folder.glob("report-docs-fix.*.read"))
        stamp = archive.name.split(".")[1]
        assert (run_folder / "notes.md").read_text() == (
            f"- 1200 earlier line\n- {stamp} docs-fix opened #9.\n"
        )

    def test_read_missing_report_exits_without_note(
        self, hub: Run, run_folder: Path
    ) -> None:
        """A name with no unread report exits 2 and logs nothing.

        A mistyped name must not log a note for a report the hub never
        read, and the exit code tells the hub the read did not happen.
        """
        result = hub(
            "read", "--run", str(run_folder), "docs-fx", "--note", "Done."
        )

        assert result.returncode == 2
        assert result.stderr == "hub: no unread report from docs-fx\n"
        assert not (run_folder / "notes.md").exists()


class TestFlag:
    """``hub flag`` and ``hub unflag``, the sidebar mark for sessions."""

    @pytest.mark.parametrize(
        ("status", "labels"),
        [
            pytest.param(
                "idle",
                [
                    "--state-label",
                    "idle=needs you",
                    "--state-label",
                    "done=needs you",
                ],
                id="idle",
            ),
            pytest.param(
                "blocked",
                ["--state-label", "blocked=needs you"],
                id="blocked",
            ),
        ],
    )
    def test_flag_marks_pane_and_unflag_clears(
        self, hub: Run, fake_tools: Path, status: str, labels: list[str]
    ) -> None:
        """The label goes on the session's pane, and unflag removes it.

        Herdr refuses the pane ID after --source, so a wrong order would
        leave the sidebar without the mark. A blocked session is marked
        only while blocked, so the mark does not stay once the user has
        answered and it works on.
        """
        set_agents(fake_tools, [make_agent("docs-fix", status)])

        marked = hub("flag", "docs-fix")
        cleared = hub("unflag", "docs-fix")

        assert (marked.returncode, cleared.returncode) == (0, 0)
        pane = [
            "pane",
            "report-metadata",
            "w1:docs-fix",
            "--source",
            "pde-hub",
        ]
        assert [
            call for call in read_calls(fake_tools) if call[0] == "pane"
        ] == [
            [*pane, *labels],
            [*pane, "--clear-state-labels"],
        ]


def write_first_reply(fake_tools: Path, session: str, effort: str) -> None:
    """Log a session's first reply, then a later one at xhigh.

    Args:
        fake_tools: The fakes' folder, which is HOME in tests.
        session: The OpenCode session ID.
        effort: The first reply's reasoning effort, such as ``medium``.
    """
    folder = fake_tools / ".local" / "share" / "opencode"
    folder.mkdir(parents=True)
    reply = {"model": {"id": "claude-opus-5-5[1m]", "variant": effort}}
    with sqlite3.connect(folder / "opencode.db") as database:
        database.execute(
            "create table session_message"
            " (session_id text, type text, seq integer, data text)"
        )
        database.execute(
            "insert into session_message values (?, 'assistant', 6, ?)",
            (session, json.dumps(reply)),
        )
        # The user may switch the effort later, which proves nothing.
        later = {"model": {"id": "claude-opus-5-5[1m]", "variant": "xhigh"}}
        database.execute(
            "insert into session_message values (?, 'assistant', 9, ?)",
            (session, json.dumps(later)),
        )
    database.close()


class TestHandoff:
    """``hub handoff``, which replaces the hub with a fresh session."""

    MODEL = "claude-code/claude-opus-5-5[1m]#medium"

    @pytest.fixture
    def next_handoff(self, fake_tools: Path, run_folder: Path) -> Path:
        """The next hub's handoff file, with both hubs and the rules' model.

        Args:
            fake_tools: The fakes' folder.
            run_folder: The hub's run folder, which gets rules.md.

        Returns:
            The next hub's handoff file, holding the run's state.
        """
        set_agents(
            fake_tools,
            [
                make_agent(
                    "kanban-hub",
                    "working",
                    pane_id="w1:kanban-hub",
                    tab_id="w1:t1",
                    workspace_id="w1",
                    agent_session={"value": "ses_old"},
                ),
                make_agent(
                    "kanban-hub-2",
                    "working",
                    pane_id="w1:new",
                    agent_session={"value": "ses_new"},
                ),
            ],
        )
        (run_folder / "rules.md").write_text(
            "- old: start hubs with --model 'claude-code/old'.\n"
            f"- new: start hubs with `--model '{self.MODEL}'`.\n"
        )
        handoff = fake_tools / "hub-2.md"
        handoff.write_text("# Handoff: run the hub\n")
        return handoff

    def test_handoff_starts_rules_model_reporting_effort(
        self, hub: Run, fake_tools: Path, run_folder: Path, next_handoff: Path
    ) -> None:
        """The next hub starts on the newest rules model and reports effort.

        A hub once told the user its successor ran at medium effort
        when it ran at the default, because nothing passed or checked
        the effort. The printed effort is read from the reply OpenCode
        logged, so the hub can repeat it to the user.
        """
        write_first_reply(fake_tools, "ses_new", "medium")

        result = hub(
            "handoff",
            "--run",
            str(run_folder),
            "kanban-hub-2",
            str(next_handoff),
        )

        assert result.returncode == 0, result.stderr
        assert result.stdout.startswith(
            f"started kanban-hub-2 on {self.MODEL}; its first reply ran at"
            " medium\n"
        )
        start = next(
            call
            for call in read_calls(fake_tools)
            if call[:2] == ["agent", "start"]
        )
        assert start[-4:] == ["--", "--model", self.MODEL, "--auto"]

    def test_handoff_tells_next_hub_review_steps(
        self, hub: Run, fake_tools: Path, run_folder: Path, next_handoff: Path
    ) -> None:
        """The old hub's review is written and the next hub told of it.

        Every hub that hands off gets a session review. The reviewer
        finds the old hub by its session ID, and the next hub learns
        from its own handoff, the one its only prompt names, which tab
        to close and which brief starts the review.
        """
        write_first_reply(fake_tools, "ses_new", "medium")

        result = hub(
            "handoff",
            "--run",
            str(run_folder),
            "kanban-hub-2",
            str(next_handoff),
        )

        assert result.returncode == 0, result.stderr
        briefs = list((fake_tools / ".local/state/pde/handoffs").iterdir())
        assert len(briefs) == 1
        assert briefs[0].name.endswith("-kanban-hub-review.md")
        assert "--session ses_old" in briefs[0].read_text()
        handoff = next_handoff.read_text()
        assert handoff.startswith("# Handoff: run the hub\n")
        assert "herdr tab close w1:t1" in handoff
        assert str(briefs[0]) in handoff
        prompts = read_prompts(fake_tools)
        assert len(prompts) == 1
        assert str(next_handoff) in prompts[0][3]

    @pytest.mark.parametrize(
        ("missing", "reason"),
        [
            pytest.param("hub-2.md", "does not exist", id="handoff"),
            pytest.param(
                "run/rules.md", "has no --model '...' line", id="model"
            ),
        ],
    )
    def test_handoff_refuses_missing_input_before_starting(
        self,
        hub: Run,
        fake_tools: Path,
        run_folder: Path,
        next_handoff: Path,
        missing: str,
        reason: str,
    ) -> None:
        """A missing handoff file or rules model exits 2 with no tab started.

        Appending to a missing file would create it, and the next hub
        would start with a handoff that holds none of the run's state.
        A run folder without rules.md is likely, and starting
        anyway would leave a tab behind. Stopping before any herdr
        change leaves nothing to clean up.
        """
        broken = fake_tools / missing
        broken.unlink()

        result = hub(
            "handoff",
            "--run",
            str(run_folder),
            "kanban-hub-2",
            str(next_handoff),
        )

        assert result.returncode == 2
        assert result.stderr == f"hub: {broken} {reason}\n"
        assert ["tab", "create"] not in [
            call[:2] for call in read_calls(fake_tools)
        ]

    def test_handoff_says_started_when_prompt_fails(
        self, hub: Run, fake_tools: Path, run_folder: Path, next_handoff: Path
    ) -> None:
        """A Herdr failure after the start says the next hub runs.

        Without the word started the hub would read a plain Herdr error
        and run the command again, which opens a second hub and adds
        the old hub's steps to the handoff twice.
        """
        refuse_prompts(fake_tools, "agent_blocked", "Herdr said no")

        result = hub(
            "handoff",
            "--run",
            str(run_folder),
            "kanban-hub-2",
            str(next_handoff),
        )

        assert result.returncode == 2
        assert result.stderr == (
            "hub: started kanban-hub-2, but Herdr said no\n"
        )

    def test_handoff_fails_when_reply_effort_differs(
        self, hub: Run, fake_tools: Path, run_folder: Path, next_handoff: Path
    ) -> None:
        """A first reply at another effort exits 2 and names both efforts.

        This is the check that would have caught the wrong claim: the
        hub must not report a successful handoff it did not verify.
        """
        write_first_reply(fake_tools, "ses_new", "default")

        result = hub(
            "handoff",
            "--run",
            str(run_folder),
            "kanban-hub-2",
            str(next_handoff),
        )

        assert result.returncode == 2
        assert result.stdout == ""
        assert result.stderr == (
            "hub: started kanban-hub-2, but it runs at default effort,"
            " not medium\n"
        )


class TestWaiting:
    """``hub waiting``, the one message of what waits on the user."""

    DECISION = (
        "## New model overnight\n"
        "\n"
        "Deploy the new model tonight so you can test it in the morning?\n"
        "\n"
        "- A. Deploy from the branch, with the undo path ready.\n"
        "- B. Wait until you return.\n"
        "\n"
        "Recommended: A, since you want to test it in the morning.\n"
        "If no reply: A.\n"
    )

    def test_waiting_prints_reviews_then_decisions_whole(
        self, hub: Run, run_folder: Path
    ) -> None:
        """Pull requests come first, then each question in full.

        Hubs named open questions by number alone, and the user had to
        ask what they were. Printing every line of each entry means
        the message never refers to text the user cannot see.
        """
        (run_folder / "reviews.md").write_text("# PRs\n\n| #1 | Fix |\n")
        (run_folder / "decisions.md").write_text(self.DECISION)

        result = hub("waiting", "--run", str(run_folder))

        assert result.returncode == 0
        assert result.stdout == (
            "# PRs\n\n| #1 | Fix |\n"
            "\n"
            "Decision: New model overnight\n"
            "Deploy the new model tonight so you can test it in the morning?\n"
            "- A. Deploy from the branch, with the undo path ready.\n"
            "- B. Wait until you return.\n"
            "Recommended: A, since you want to test it in the morning.\n"
            "If no reply: A.\n"
        )

    def test_waiting_refuses_entry_missing_parts(
        self, hub: Run, run_folder: Path
    ) -> None:
        """An entry missing Recommended and If no reply lines exits 2.

        The user decides from this message alone, often away
        from the desk, so an entry that leaves out what happens with
        no reply cannot go out.
        """
        (run_folder / "decisions.md").write_text(
            "## Pause cards\n\nPause stale cards?\n\n- A. Yes\n- B. No\n"
        )

        result = hub("waiting", "--run", str(run_folder))

        assert result.returncode == 2
        assert result.stdout == ""
        assert result.stderr == (
            "hub: decisions.md entry 'Pause cards' lacks the Recommended:"
            " line, the If no reply: line\n"
        )
