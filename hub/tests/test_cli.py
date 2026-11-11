"""Tests that run the packed file ``build.py`` writes against a fake herdr."""

import json
import os
import re
import socket
import sqlite3
import subprocess
import sys
import threading
import time
from collections.abc import Callable, Iterator
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
# shells.json holds the panes that run no agent, with their process info.
shells_file = fake / "shells.json"
shells = json.loads(shells_file.read_text()) if shells_file.exists() else []

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
elif args[:2] in (
    ["pane", "report-metadata"],
    ["agent", "start"],
    ["agent", "send-keys"],
    ["agent", "wait"],
):
    print(json.dumps({"result": {}}))
elif args[:2] == ["tab", "create"]:
    print(json.dumps({"result": {"root_pane": {"pane_id": "w1:new"}}}))
elif args[:2] == ["agent", "focus"]:
    print(json.dumps({"result": {}}))
elif args[:2] == ["workspace", "list"]:
    ids = json.loads((fake / "workspaces.json").read_text())
    workspaces = [{"workspace_id": id_} for id_ in ids]
    print(json.dumps({"result": {"workspaces": workspaces}}))
elif args[:2] == ["pane", "list"]:
    # Herdr's pane list holds no agent names, so leave them out here too.
    hidden = ("name", "process_info")
    panes = [
        {key: value for key, value in pane.items() if key not in hidden}
        for pane in [*agents, *shells]
    ]
    print(json.dumps({"result": {"panes": panes}}))
elif args[:2] == ["pane", "process-info"]:
    shell = next(shell for shell in shells if shell["pane_id"] == args[3])
    print(json.dumps({"result": {"process_info": shell["process_info"]}}))
elif args[:2] == ["tab", "close"]:
    print(json.dumps({"result": {}}))
else:
    print(f"unknown command {args}", file=sys.stderr)
    sys.exit(2)
"""

# Stands in for the card command, so no test reads the real vault. It
# prints the notes in $FAKE/cards, listing those whose stage line
# matches, or fails when $FAKE/card-fails exists.
FAKE_CARD = """\
import os, re, sys
from pathlib import Path

fake = Path(os.environ["FAKE"])
if (fake / "card-fails").exists():
    print("card: no default vault", file=sys.stderr)
    sys.exit(1)
cards = sorted((fake / "cards").glob("*.md"))
if sys.argv[1:3] == ["list", "--stage"]:
    for card in cards:
        stage = re.search(r"^stage: (\\S+)", card.read_text(), re.M)[1]
        if stage == sys.argv[3]:
            print(f"{card.stem} {stage:8} build    A card")
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
    the fakes' folder, so no test reads the real OpenCode database. The
    Herdr socket is a file there too, so no test reaches the user's
    real Herdr.

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
            "HERDR_SOCKET_PATH": str(fake_tools / "herdr.sock"),
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
    fake_tools: Path,
    key: str,
    sessions: list[str],
    log: str,
    stage: str = "active",
) -> None:
    """Add a card for the fake card command to show.

    Args:
        fake_tools: The fakes' folder.
        key: The card's key, such as ``DEMO-KB-001``.
        sessions: Its session IDs, oldest first.
        log: The lines of its Log section.
        stage: Its stage, such as ``accepted``.
    """
    items = "".join(f'  - "opencode {session}"\n' for session in sessions)
    note = (
        f"---\nstage: {stage}\nsessions:\n{items}topics: []\n---\n\n"
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


def set_workspaces(fake_tools: Path, *workspace_ids: str) -> None:
    """Make the fake herdr list these workspaces, top of the sidebar first.

    Args:
        fake_tools: The fake herdr's folder.
        *workspace_ids: The workspace IDs in sidebar order.
    """
    (fake_tools / "workspaces.json").write_text(json.dumps(workspace_ids))


def answer_one_request(
    fake_tools: Path, answer: dict
) -> tuple[threading.Thread, list[dict]]:
    """Answer one request on the fake Herdr socket, in the background.

    Args:
        fake_tools: The fakes' folder, which holds the socket.
        answer: The reply without its ``id``, such as ``{"result": {}}``.

    Returns:
        The thread, to join once hub ends, and a list that holds the
        request once it arrives.
    """
    server = socket.socket(socket.AF_UNIX)
    server.bind(str(fake_tools / "herdr.sock"))
    server.listen(1)
    server.settimeout(30)
    requests: list[dict] = []

    def serve() -> None:
        """Read one request line and send ``answer`` back with its id."""
        with server, server.accept()[0] as connection:
            request = json.loads(connection.makefile("rb").readline())
            requests.append(request)
            reply = {"id": request["id"], **answer}
            connection.sendall(json.dumps(reply).encode() + b"\n")

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    return thread, requests


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


def write_messages(
    fake_tools: Path, session: str, messages: list[tuple[str, int, dict]]
) -> None:
    """Log a session's messages in a fake OpenCode database.

    Args:
        fake_tools: The fakes' folder, which is HOME in tests.
        session: The OpenCode session ID.
        messages: Each message's type, last change in milliseconds
            since 1970, and data, oldest first.
    """
    folder = fake_tools / ".local" / "share" / "opencode"
    folder.mkdir(parents=True)
    with sqlite3.connect(folder / "opencode.db") as database:
        database.execute(
            "create table session_message (id text, session_id text,"
            " type text, seq integer, time_created integer,"
            " time_updated integer, data text)"
        )
        database.executemany(
            "insert into session_message values (?, ?, ?, ?, ?, ?, ?)",
            [
                (f"msg_{seq}", session, kind, seq, at, at, json.dumps(data))
                for seq, (kind, at, data) in enumerate(messages)
            ],
        )
    database.close()


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
                "-",
                "DEMO-KB-007",
                "unread",
                "-",
                "needs",
                "you",
            ],
            ["docs-fix", "working", "-", "-", "-", "held", "-"],
        ]

    def test_status_ctx_drops_double_counted_step(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """CTX leaves out a zero-token step and the step after it.

        The claude-code provider logs some steps with zero tokens and
        adds them to the next step. The footer showed a worker at 217.7K
        that really held 107.8K, so the hub would hand off a worker that
        still had room.
        """
        set_agents(
            fake_tools,
            [make_agent("docs-fix", "idle", agent_session={"value": "ses_a"})],
        )
        zero = {"input": 0, "cache": {"read": 0, "write": 0}}
        write_messages(
            fake_tools,
            "ses_a",
            [
                ("user", 1, {}),
                (
                    "assistant",
                    2,
                    {
                        "tokens": {
                            "input": 800,
                            "cache": {"read": 100_000, "write": 7_000},
                        }
                    },
                ),
                ("assistant", 3, {"tokens": zero}),
                (
                    "assistant",
                    4,
                    {"tokens": {"input": 0, "cache": {"read": 217_700}}},
                ),
            ],
        )

        result = hub("status", "--run", str(run_folder))

        sessions = result.stdout.split("\n\n")[0]
        assert sessions.splitlines()[1].split()[:3] == [
            "docs-fix",
            "idle",
            "108k",
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

    def test_send_marks_unsure_when_nothing_pasted(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A stall with no pasted text leaves the message marked unsure.

        Herdr reports a stall after the text went in, so the session
        may have it. Without the mark the hub would send it again and
        the session would get it twice. No key is pressed, since Enter
        would send whatever the user typed in the box.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "idle")])
        refuse_prompts(fake_tools, "agent_prompt_stalled", "prompt stalled")

        result = hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")

        assert result.returncode == 1
        sending = run_folder / "send-docs-fix.sending"
        assert json.loads(sending.read_text())["message"] == "Rebase."
        assert not [c for c in read_calls(fake_tools) if "send-keys" in c]

    def test_send_submits_stalled_paste_once(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A stalled prompt whose text sits pasted is sent with one Enter.

        In an auto run a worker sat done with "[Pasted ~1 lines]" in its
        input box, and the hub thought the message might have arrived.
        A second Enter could send the user's own typing.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "done")])
        refuse_prompts(fake_tools, "agent_prompt_stalled", "prompt stalled")
        (fake_tools / "screens" / "docs-fix").write_text(
            "  ┃  [Pasted ~1 lines]\n"
        )

        result = hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")

        assert result.returncode == 0
        assert result.stdout == "sent docs-fix\n"
        keys = [c for c in read_calls(fake_tools) if "send-keys" in c]
        assert keys == [["agent", "send-keys", "docs-fix", "Enter"]]
        assert list(run_folder.iterdir()) == []

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

    def test_next_skips_enter_for_focused_pane(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A stalled paste gets no Enter once the user is in the pane.

        The user may focus the pane during the stall and start typing.
        Enter would then send their typing with the message, so the
        message stays unsure for the hub to check instead.
        """
        set_agents(fake_tools, [make_agent("docs-fix", "working")])
        hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")
        set_agents(
            fake_tools,
            [make_agent("docs-fix", "idle")],
            [make_agent("docs-fix", "idle", focused=True)],
        )
        refuse_prompts(fake_tools, "agent_prompt_stalled", "prompt stalled")
        (fake_tools / "screens" / "docs-fix").write_text(
            "  ┃  [Pasted ~1 lines]\n"
        )

        result = hub("next", "--run", str(run_folder))

        assert result.stdout == "unsure docs-fix: prompt stalled\n"
        assert not [c for c in read_calls(fake_tools) if "send-keys" in c]
        assert (run_folder / "send-docs-fix.sending").exists()


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


class TestBrief:
    """``hub brief``, the one message of what waits on the user."""

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

    def test_brief_prints_reviews_then_decisions_whole(
        self, hub: Run, run_folder: Path
    ) -> None:
        """Pull requests come first, then each question in full.

        Hubs named open questions by number alone, and the user had to
        ask what they were. Printing every line of each entry means
        the message never refers to text the user cannot see.
        """
        (run_folder / "reviews.md").write_text("# PRs\n\n| #1 | Fix |\n")
        (run_folder / "decisions.md").write_text(self.DECISION)

        result = hub("brief", "--run", str(run_folder))

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

    def test_brief_refuses_entry_missing_parts(
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

        result = hub("brief", "--run", str(run_folder))

        assert result.returncode == 2
        assert result.stdout == ""
        assert result.stderr == (
            "hub: decisions.md entry 'Pause cards' lacks the Recommended:"
            " line, the If no reply: line\n"
        )


def write_run_card(fake_tools: Path, key: str, stage: str = "ready") -> None:
    """Add a card with a project, Context and Acceptance Criteria.

    Args:
        fake_tools: The fakes' folder.
        key: The card's key, such as ``DEMO-KB-001``.
        stage: The card's stage.
    """
    note = (
        f"---\nstage: {stage}\nproject: Demo\ntopics: []\n---\n\n"
        f"# {key} Retry failed uploads\n---\n\n"
        "## Context\n---\n\n- Uploads fail on a slow link.\n\n"
        "## Acceptance Criteria\n---\n\n- [ ] A failed upload retries.\n\n"
        "## Log\n---\n\n- 2026-11-10 ready: by the user\n"
    )
    (fake_tools / "cards" / f"{key}.md").write_text(note)


def write_run_list(run_folder: Path, run: str, rest: str = "") -> None:
    """Write run.md for auto run 2 with these lines under its headings.

    Args:
        run_folder: The auto run's folder.
        run: The lines under ``## Run``.
        rest: Any further headings and their lines.
    """
    (run_folder / "run.md").write_text(
        f"# Auto run 2\n\n## Run\n{run}\n{rest}"
    )


class TestAuto:
    """``hub auto``, the scripted steps of a run the user approved."""

    def test_ready_writes_each_ready_card_text(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """Only ready cards are described, with project and criteria.

        The local model sorts the cards from this file alone, so a
        card left out or cut short is sorted on a guess.
        """
        write_run_card(fake_tools, "DEMO-KB-001")
        write_run_card(fake_tools, "DEMO-KB-002", stage="draft")

        result = hub("auto", "ready", "--run", str(run_folder))

        assert result.returncode == 0
        assert result.stdout == (
            "## DEMO-KB-001 Retry failed uploads\n"
            "Project: Demo\n"
            "- Uploads fail on a slow link.\n"
            "- [ ] A failed upload retries.\n"
        )
        assert (run_folder / "ready.md").read_text() == result.stdout

    @pytest.mark.parametrize(
        ("run", "rest", "problem"),
        [
            pytest.param(
                "- DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: draft",
                "",
                "DEMO-KB-002 is not a ready card; only the user moves a"
                " card to ready",
                id="draft-card",
            ),
            pytest.param(
                "- DEMO-KB-001 demo: retry",
                "\n## Left out\n- DEMO-KB-001 demo: retry\n",
                "DEMO-KB-001 is listed more than once",
                id="listed-twice",
            ),
            pytest.param(
                "",
                "\n## Left out\n- DEMO-KB-001 demo: retry\n",
                "run.md lists no card under '## Run'",
                id="no-run-cards",
            ),
        ],
    )
    def test_check_names_list_problem(
        self,
        hub: Run,
        fake_tools: Path,
        run_folder: Path,
        run: str,
        rest: str,
        problem: str,
    ) -> None:
        """A list the user could not run as approved exits 2.

        Draft cards wait for the user's refinement, a card listed
        twice would get two workers, and a list with nothing under Run
        would start an empty run, so none may reach the briefing.
        """
        write_run_card(fake_tools, "DEMO-KB-001")
        write_run_card(fake_tools, "DEMO-KB-002", stage="draft")
        write_run_list(run_folder, run, rest)

        result = hub("auto", "check", "--run", str(run_folder))

        assert result.returncode == 2
        assert f"hub: {problem}\n" in result.stderr

    def test_check_names_missing_ready_card(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """A ready card the list leaves out exits 2.

        The briefing must account for every ready card, so the user
        sees what the hub chose not to run. A card under another
        heading counts as listed.
        """
        write_run_card(fake_tools, "DEMO-KB-001")
        write_run_card(fake_tools, "DEMO-KB-002")
        write_run_card(fake_tools, "DEMO-KB-003")
        write_run_list(
            run_folder,
            "- DEMO-KB-001 demo: retry",
            "\n## Needs your hands\n- DEMO-KB-002 demo: plug in a disk\n",
        )

        result = hub("auto", "check", "--run", str(run_folder))

        assert result.returncode == 2
        assert result.stderr == (
            "hub: ready card DEMO-KB-003 is missing from run.md\n"
        )

    @pytest.mark.parametrize(
        ("number", "reply"),
        [
            pytest.param("2", "go", id="go"),
            pytest.param("2", "approve run 3", id="other-phrase"),
            pytest.param("2", "approve run 2 without DEMO-KB-001", id="edit"),
            pytest.param("3", "approve run 3", id="other-run"),
        ],
    )
    def test_approve_refuses_reply_other_than_phrase(
        self,
        hub: Run,
        fake_tools: Path,
        run_folder: Path,
        number: str,
        reply: str,
    ) -> None:
        """Any reply but the phrase for this list approves nothing.

        Auto run 1 started on a "go" the user meant for another
        question, so only "approve run N", with N the number of the
        list in run.md, may start a run.
        """
        write_run_card(fake_tools, "DEMO-KB-001")
        write_run_list(run_folder, "- DEMO-KB-001 demo: retry")

        result = hub(
            "auto", "approve", "--run", str(run_folder), number, reply
        )

        assert result.returncode == 2
        assert not (run_folder / "approved.md").exists()

    def test_approve_saves_list_with_users_words(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """The approval keeps the user's reply beside the list.

        Later sessions check what the user approved against this
        file, in the user's own words. Case and a final period in the
        reply do not matter.
        """
        write_run_card(fake_tools, "DEMO-KB-001")
        write_run_list(run_folder, "- DEMO-KB-001 demo: retry")

        result = hub(
            "auto", "approve", "--run", str(run_folder), "2", "Approve run 2."
        )

        approved = (run_folder / "approved.md").read_text()
        assert result.returncode == 0
        assert "The user's words: Approve run 2.\n" in approved
        assert approved.endswith((run_folder / "run.md").read_text())

    @pytest.mark.parametrize(
        "words",
        [
            pytest.param("go ahead with DEMO-KB-001", id="card-missing"),
            pytest.param(
                "run DEMO-KB-001, DEMO-KB-002 and DEMO-KB-003",
                id="extra-card",
            ),
            pytest.param("you have my approval", id="no-card"),
        ],
    )
    def test_approve_given_refuses_other_card_set(
        self, hub: Run, fake_tools: Path, run_folder: Path, words: str
    ) -> None:
        """Words that do not name exactly the run's cards approve nothing.

        Words the user gave before the briefing are checked only by
        the cards they name. A card they did not name must not run,
        and a card they named but the list leaves out means the list
        is not what they approved.
        """
        for key in ("DEMO-KB-001", "DEMO-KB-002", "DEMO-KB-003"):
            write_run_card(fake_tools, key)
        write_run_list(
            run_folder,
            "- DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: resume",
            "\n## Needs your hands\n- DEMO-KB-003 demo: plug in a disk\n",
        )

        result = hub(
            "auto", "approve", "--run", str(run_folder), "--given", "2", words
        )

        assert result.returncode == 2
        assert not (run_folder / "approved.md").exists()

    def test_approve_given_saves_users_own_words(
        self, hub: Run, fake_tools: Path, run_folder: Path
    ) -> None:
        """Own words that name exactly the run's cards approve it.

        A user who approves a run before its briefing exists does so in
        their own words, and the hub records them rather than writing
        approved.md by hand. Keys count in lower case too.
        """
        write_run_card(fake_tools, "DEMO-KB-001")
        write_run_card(fake_tools, "DEMO-KB-002")
        write_run_list(
            run_folder, "- DEMO-KB-001 demo: retry\n- DEMO-KB-002 demo: resume"
        )
        words = "Run demo-kb-001 and DEMO-KB-002 while I am away."

        result = hub(
            "auto", "approve", "--run", str(run_folder), "--given", "2", words
        )

        approved = (run_folder / "approved.md").read_text()
        assert result.returncode == 0
        assert f"The user's words: {words}\n" in approved

    def test_now_writes_line_per_run_card(
        self, hub: Run, run_folder: Path
    ) -> None:
        """Each run card gets its status, or that it waits on the user.

        A front hub once answered "status update" with "nothing new"
        while four cards were done and a question waited, so the
        lines come from the files, not from the hub's memory.
        """
        (run_folder / "approved.md").write_text(
            "# Auto run 2 approved\n\nThe user's words: approve run 2\n\n"
            "# Auto run 2\n\n## Run\n"
            "- DEMO-KB-001 demo: retry\n"
            "- DEMO-KB-002 demo: resize\n"
            "- DEMO-KB-003 demo: export\n\n"
            "## Needs your hands\n- DEMO-KB-004 demo: plug in a disk\n"
        )
        (run_folder / "status-demo-kb-001-auto.md").write_text(
            "\nPR #12 opened; waiting for the user's review.\nMore.\n"
        )
        (run_folder / "decisions.md").write_text(
            "## DEMO-KB-002 command name\n\nWhich name?\n\n"
            "- A. resize\n- B. scale\n\n"
            "Recommended: A.\nIf no reply: A.\n"
        )

        result = hub("auto", "now", "--run", str(run_folder))

        assert result.returncode == 0
        assert result.stdout == (
            "Auto run 2\n"
            "DEMO-KB-001: PR #12 opened; waiting for the user's review.\n"
            "DEMO-KB-002 (needs you): not started\n"
            "DEMO-KB-003: not started\n"
            "Questions for you: 1\n"
            "- DEMO-KB-002 command name\n"
        )
        assert (run_folder / "brief-now.md").read_text() == result.stdout

    def test_now_refuses_unapproved_run(
        self, hub: Run, run_folder: Path
    ) -> None:
        """Without approved.md the run has no status and exits 2.

        The auto hub runs this first, so a run the user never
        approved stops before any worker starts.
        """
        write_run_list(run_folder, "- DEMO-KB-001 demo: retry")

        result = hub("auto", "now", "--run", str(run_folder))

        assert result.returncode == 2
        assert not (run_folder / "brief-now.md").exists()


class TestGo:
    """``hub go``, the jump to a hub that ctrl+b shift+h runs."""

    @pytest.mark.parametrize(
        ("focused", "shown"),
        [
            pytest.param(
                "demo-kb-001-impl", "kanban-hub-13", id="not-on-a-hub"
            ),
            pytest.param("kanban-hub-13", "auto-hub-1", id="next-hub"),
            pytest.param("auto-hub-1", "kanban-hub-13", id="wraps-to-first"),
        ],
    )
    def test_go_shows_hub_after_focused_one(
        self, hub: Run, fake_tools: Path, focused: str, shown: str
    ) -> None:
        """Each press shows the next hub in sidebar order, then wraps.

        The user presses one key to move between hubs, so a wrong order
        or a missing wrap would leave a hub they cannot reach by
        pressing again. Name order differs from sidebar order here, so
        a sort by name fails.
        """
        set_workspaces(fake_tools, "w1", "w2", "w3")
        set_agents(
            fake_tools,
            [
                make_agent(
                    name,
                    "idle",
                    workspace_id=workspace_id,
                    focused=name == focused,
                )
                for name, workspace_id in (
                    ("auto-hub-1", "w3"),
                    ("demo-kb-001-impl", "w1"),
                    ("kanban-hub-13", "w2"),
                )
            ],
        )

        result = hub("go")

        assert result.returncode == 0
        focus = [call for call in read_calls(fake_tools) if "focus" in call]
        assert focus == [["agent", "focus", shown]]


class TestGather:
    """``hub gather``, which puts every hub at the top of the sidebar."""

    def test_gather_moves_hub_workspaces_to_top(
        self, hub: Run, fake_tools: Path
    ) -> None:
        """Every hub workspace moves once, ahead of the first other one.

        The hub runs gather from its own pane at each start, so its own
        workspace must move too. Herdr refuses a move that names a
        workspace twice or anchors on a hub, which would leave the hubs
        down the sidebar.
        """
        set_workspaces(fake_tools, "w1", "w2", "w3")
        set_agents(
            fake_tools,
            [
                make_agent("auto-hub-1", "done", workspace_id="w3"),
                make_agent("auto-hub-2", "done", workspace_id="w3"),
                make_agent("demo-kb-001-impl", "idle", workspace_id="w2"),
                make_agent(
                    "kanban-hub-13",
                    "working",
                    workspace_id="w1",
                    pane_id="w1:kanban-hub",
                ),
            ],
        )
        thread, requests = answer_one_request(fake_tools, {"result": {}})

        result = hub("gather")
        thread.join(timeout=5)

        assert result.returncode == 0
        assert requests == [
            {
                "id": "hub",
                "method": "workspace.move_block",
                "params": {
                    "workspace_ids": ["w1", "w3"],
                    "before_workspace_id": "w2",
                },
            }
        ]

    def test_gather_exits_with_herdr_refusal(
        self, hub: Run, fake_tools: Path
    ) -> None:
        """A refused move exits 1 with Herdr's own message.

        The hub trusts the exit code, so a 0 here would tell it the
        hubs sit at the top when they do not.
        """
        set_workspaces(fake_tools, "w1", "w2")
        set_agents(
            fake_tools, [make_agent("auto-hub-1", "done", workspace_id="w2")]
        )
        refusal = {"code": "invalid_params", "message": "bad block"}
        thread, _ = answer_one_request(fake_tools, {"error": refusal})

        result = hub("gather")
        thread.join(timeout=5)

        assert result.returncode == 1
        assert result.stderr == "hub: bad block\n"


# A finished step that ran the shell tool, logged before the newest one.
SHELL_STEP = {
    "time": {"completed": 1},
    "content": [
        {"type": "text"},
        {"type": "tool", "name": "shell", "state": {"status": "completed"}},
    ],
}


class TestAlive:
    """``hub alive``, which tells the hub whether a session still works."""

    @pytest.mark.parametrize(
        ("kind", "minutes_ago", "data", "tool", "word"),
        [
            pytest.param(
                "idle",
                10,
                {"outcome": "succeeded"},
                "shell",
                "finished",
                id="turn-succeeded",
            ),
            pytest.param(
                "idle",
                0,
                {"outcome": "interrupted"},
                "shell",
                "stopped",
                id="turn-interrupted",
            ),
            pytest.param(
                "assistant",
                10,
                {
                    "content": [
                        {
                            "type": "tool",
                            "name": "subagent",
                            "state": {"status": "running"},
                        }
                    ]
                },
                "subagent",
                "working",
                id="quiet-tool-running",
            ),
            pytest.param(
                "user", 0, {}, "shell", "working", id="recent-message"
            ),
            pytest.param(
                "assistant",
                10,
                {"content": [{"type": "text"}]},
                "shell",
                "stopped",
                id="quiet-unfinished-step",
            ),
        ],
    )
    def test_alive_word_follows_newest_message(
        self,
        hub: Run,
        fake_tools: Path,
        kind: str,
        minutes_ago: int,
        data: dict,
        tool: str,
        word: str,
    ) -> None:
        """The last word says whether the session works, finished or stopped.

        The hub runs this before telling the user a session died. Old
        sessions killed in the middle of a step keep an unfinished step,
        and a parent that waits on a subagent changes nothing for
        minutes, so neither the step nor the time decides alone.
        """
        now_ms = int(time.time() * 1000)
        newest = now_ms - minutes_ago * 60_000
        write_messages(
            fake_tools,
            "ses_a",
            [
                ("assistant", now_ms - 20 * 60_000, SHELL_STEP),
                (kind, newest, data),
            ],
        )

        result = hub("alive", "ses_a")

        assert result.returncode == 0
        assert re.fullmatch(
            rf"2 messages, newest \d\d:\d\d:\d\d, last tool {tool}, {word}\n",
            result.stdout,
        )

    def test_alive_unknown_session_exits_with_error(
        self, hub: Run, fake_tools: Path
    ) -> None:
        """A session missing from the database exits 1 with a reason.

        A mistyped session ID must not read as a stopped session, or the
        hub would tell the user a working session died.
        """
        write_messages(fake_tools, "ses_a", [("user", 1, {})])

        result = hub("alive", "ses_b")

        assert result.returncode == 1
        assert result.stdout == ""
        assert result.stderr == (
            "hub: no OpenCode session ses_b on this machine\n"
        )


def make_worker(
    name: str, status: str, tab: str, folder: Path, **fields: object
) -> dict:
    """Build an agent alone in its tab, working from one folder.

    Args:
        name: The agent's name.
        status: Herdr's state, such as ``idle``.
        tab: The tab's ID, such as ``w1:t1``.
        folder: The agent's working folder.
        **fields: Fields to add or replace, such as ``focused``.

    Returns:
        The agent, with the fields ``herdr pane list`` also prints.
    """
    return make_agent(
        name,
        status,
        tab_id=tab,
        agent="opencode",
        cwd=str(folder),
        foreground_cwd=str(folder),
        **fields,
    )


def set_shells(fake_tools: Path, *shells: tuple[str, str, int, int]) -> None:
    """Add panes that run no agent.

    Args:
        fake_tools: The fake herdr's folder.
        *shells: Each pane's ID, tab, shell pid and foreground process
            group. A group equal to the shell pid means the shell sits
            at its prompt.
    """
    panes = [
        {
            "pane_id": pane,
            "tab_id": tab,
            "agent": None,
            "agent_status": "unknown",
            "focused": False,
            "process_info": {
                "foreground_process_group_id": group,
                "shell_pid": shell_pid,
            },
        }
        for pane, tab, shell_pid, group in shells
    ]
    (fake_tools / "shells.json").write_text(json.dumps(panes))


# Stands in for a pane's shell until its input closes. With an argument it
# starts a job in its own process group but the shell's session, as bash does.
FAKE_SHELL = """\
import os, subprocess, sys

jobs = [
    subprocess.Popen(["sleep", "60"], preexec_fn=os.setpgrp)
    for _ in sys.argv[1:]
]
print("ready", flush=True)
sys.stdin.read()
for job in jobs:
    job.kill()
"""


@pytest.fixture
def start_shell() -> Iterator[Callable[[bool], int]]:
    """Start fake shells as real processes and end them after the test."""
    shells: list[subprocess.Popen] = []

    def start(with_job: bool) -> int:
        """Start one fake shell, with a background job or none.

        Args:
            with_job: Whether the shell owns a job.

        Returns:
            The shell's pid, once its job has started.
        """
        shell = subprocess.Popen(
            [sys.executable, "-c", FAKE_SHELL, *(["job"] if with_job else [])],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        shells.append(shell)
        shell.stdout.readline()
        return shell.pid

    yield start
    try:
        for shell in shells:
            shell.stdin.close()
            shell.wait(timeout=10)
    finally:
        for shell in shells:
            shell.kill()


def read_closed_tabs(fake_tools: Path) -> list[str]:
    """Read the tab IDs the fake herdr was asked to close.

    Args:
        fake_tools: The fake herdr's folder.

    Returns:
        The tab IDs, in order.
    """
    return [
        call[2]
        for call in read_calls(fake_tools)
        if call[:2] == ["tab", "close"]
    ]


def run_git(folder: Path, *args: str) -> None:
    """Run git in a folder as a made-up user.

    Args:
        folder: The folder git runs in.
        *args: The git arguments, such as ``commit``.
    """
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-C",
            str(folder),
            *args,
        ],
        check=True,
        capture_output=True,
    )


@pytest.fixture
def card_repo(tmp_path: Path) -> Path:
    """Return a main checkout whose card worktree is clean and pushed.

    The worktree is ``card-wt`` next to it, on ``feat/demo-kb-001-fix``,
    since card branches start with the card's key in lowercase.

    Args:
        tmp_path: The test's own folder.

    Returns:
        The main checkout.
    """
    run_git(tmp_path, "init", "--bare", "origin.git")
    run_git(tmp_path, "clone", "origin.git", "main")
    main = tmp_path / "main"
    run_git(main, "commit", "--allow-empty", "-m", "start")
    run_git(main, "push", "origin", "HEAD")
    run_git(
        main, "worktree", "add", "-b", "feat/demo-kb-001-fix", "../card-wt"
    )
    run_git(tmp_path / "card-wt", "push", "origin", "feat/demo-kb-001-fix")
    return main


class TestClean:
    """``hub clean``, which closes the tabs of finished work."""

    @pytest.mark.parametrize("stage", ["accepted", "dropped"])
    def test_clean_closes_finished_card_free_tab(
        self, hub: Run, fake_tools: Path, card_repo: Path, stage: str
    ) -> None:
        """A free agent of a finished card loses its tab, and clean says so.

        The hub runs clean after each acceptance, so a tab it misses
        stays for the user to close by hand. The line tells the user
        which tab went and why.
        """
        write_card(fake_tools, "DEMO-KB-001", [], "", stage=stage)
        set_agents(
            fake_tools,
            [make_worker("demo-kb-001-impl", "idle", "w1:t1", card_repo)],
        )

        result = hub("clean")

        assert result.returncode == 0
        assert result.stdout == (
            f"closed w1:t1 demo-kb-001-impl: DEMO-KB-001 {stage}\n"
        )
        assert read_closed_tabs(fake_tools) == ["w1:t1"]

    @pytest.mark.parametrize(
        ("leave_work", "reason"),
        [
            pytest.param(
                lambda worktree: (worktree / "notes.txt").write_text("x"),
                "uncommitted files",
                id="uncommitted",
            ),
            pytest.param(
                lambda worktree: run_git(
                    worktree, "commit", "--allow-empty", "-m", "fix"
                ),
                "unpushed commits",
                id="unpushed",
            ),
        ],
    )
    def test_clean_keeps_tab_with_unsaved_work(
        self,
        hub: Run,
        fake_tools: Path,
        card_repo: Path,
        leave_work: Callable[[Path], object],
        reason: str,
    ) -> None:
        """A finished card's tab stays while its worktree holds unsaved work.

        The agent works from the main checkout, so only the branch name
        ties the worktree to the card. Closing the tab would hide the
        last sign of work that was never saved to the remote, and the
        kept line names the folder so the user can look.
        """
        worktree = card_repo.parent / "card-wt"
        leave_work(worktree)
        write_card(fake_tools, "DEMO-KB-001", [], "", stage="accepted")
        set_agents(
            fake_tools,
            [make_worker("demo-kb-001-impl", "done", "w1:t1", card_repo)],
        )

        result = hub("clean")

        assert result.returncode == 0
        assert result.stdout == (
            f"kept demo-kb-001-impl: {reason} in {worktree}\n"
            "nothing to close\n"
        )
        assert read_closed_tabs(fake_tools) == []

    @pytest.mark.parametrize(
        ("status", "stage", "fields", "prepare", "expected"),
        [
            pytest.param(
                "working",
                "accepted",
                {},
                lambda worktree: None,
                "kept demo-kb-001-impl: working\nnothing to close\n",
                id="working",
            ),
            pytest.param(
                "idle",
                "active",
                {},
                lambda worktree: None,
                "nothing to close\n",
                id="active-card",
            ),
            pytest.param(
                "idle",
                "accepted",
                {"focused": True},
                lambda worktree: None,
                "nothing to close\n",
                id="focused",
            ),
            pytest.param(
                "idle",
                "accepted",
                {},
                lambda worktree: (worktree / ".git").write_text(
                    "gitdir: missing"
                ),
                # Git versions word this error differently.
                "kept demo-kb-001-impl: git failed, fatal: .+\n"
                "nothing to close\n",
                id="git-failed",
            ),
        ],
    )
    def test_clean_keeps_tab_cutting_off_work(
        self,
        hub: Run,
        fake_tools: Path,
        card_repo: Path,
        status: str,
        stage: str,
        fields: dict,
        prepare: Callable[[Path], object],
        expected: str,
    ) -> None:
        """Clean leaves a tab where closing would cut off work or the user.

        A working agent would lose its turn, an active card still has
        work to do, and the focused tab is the one the user reads. When
        git fails the worktree may hold unsaved work, so the tab stays
        and the kept line shows git's message. The other kept tabs
        print nothing, since the user does not expect them to close.
        """
        worktree = card_repo.parent / "card-wt"
        prepare(worktree)
        write_card(fake_tools, "DEMO-KB-001", [], "", stage=stage)
        set_agents(
            fake_tools,
            [
                make_worker(
                    "demo-kb-001-impl",
                    status,
                    "w1:t1",
                    card_repo,
                    **fields,
                )
            ],
        )

        result = hub("clean")

        assert result.returncode == 0
        assert re.fullmatch(expected, result.stdout)
        assert read_closed_tabs(fake_tools) == []

    def test_clean_closes_agentless_tab_at_prompt(
        self, hub: Run, fake_tools: Path, start_shell: Callable[[bool], int]
    ) -> None:
        """A tab with no agent closes only while all its shells run nothing.

        Such tabs often hold the user's editor, pager or picker, and
        closing one would throw away what the user had open. Tab w1:t6
        has one idle shell and one busy one, so it stays. The shell in
        w1:t7 sits at its prompt but owns a job, such as an editor
        stopped with Ctrl-Z, which closing the tab would kill.
        """
        shell = start_shell(False)
        shell_with_job = start_shell(True)
        set_agents(fake_tools, [])
        set_shells(
            fake_tools,
            ("w1:p5", "w1:t5", shell, shell),
            ("w1:p6", "w1:t6", shell, shell),
            ("w1:p7", "w1:t6", shell, shell_with_job),
            ("w1:p8", "w1:t7", shell_with_job, shell_with_job),
        )

        result = hub("clean")

        assert result.returncode == 0
        assert result.stdout == "closed w1:t5 no agent\n"
        assert read_closed_tabs(fake_tools) == ["w1:t5"]

    def test_clean_keeps_own_tab_at_prompt(
        self, hub: Run, fake_tools: Path, start_shell: Callable[[bool], int]
    ) -> None:
        """The hub's own tab stays even when every pane in it could close.

        Closing it would end the hub mid-run. Both panes run no agent
        and sit at their prompt, so only the own-pane rule keeps them.
        """
        shell = start_shell(False)
        set_agents(fake_tools, [])
        set_shells(
            fake_tools,
            ("w1:kanban-hub", "w1:t8", shell, shell),
            ("w1:p9", "w1:t8", shell, shell),
        )

        result = hub("clean")

        assert result.returncode == 0
        assert result.stdout == "nothing to close\n"
        assert read_closed_tabs(fake_tools) == []
