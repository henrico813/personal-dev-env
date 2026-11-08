"""Tests that run the packed file ``build.py`` writes against a fake herdr."""

import json
import os
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

BUILD = Path(__file__).parents[1] / "build.py"

# Answers each call from files in $FAKE and logs its arguments to
# calls.jsonl. agents.json holds one agent list per agent list call, the
# last one repeating, so a test can change a session's state over time.
FAKE_HERDR = """\
import json, os, sys
from pathlib import Path

fake = Path(os.environ["FAKE"])
args = sys.argv[1:]
with (fake / "calls.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\\n")
if args[:1] == ["--machine"]:
    args = args[2:]
lists = json.loads((fake / "agents.json").read_text())
count_file = fake / "list-count"
count = int(count_file.read_text()) if count_file.exists() else 0
if args[:2] == ["agent", "list"]:
    count_file.write_text(str(count + 1))
agents = lists[min(count, len(lists) - 1)]
by_name = {agent["name"]: agent for agent in agents if agent.get("name")}


def refuse(code, message):
    print(json.dumps({"error": {"code": code, "message": message}}))
    sys.exit(1)


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
elif args[:2] == ["pane", "report-metadata"]:
    print(json.dumps({"result": {}}))
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
def fake(tmp_path: Path) -> Path:
    """Return the fakes' folder, with fake herdr and card in its ``bin``.

    Args:
        tmp_path: The test's own folder.

    Returns:
        The folder, which also holds the fake's answers and call log.
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
def hub(packed: Path, fake: Path) -> Run:
    """Return a function that runs hub with the fake herdr.

    The hub's own pane is ``w1:kanban-hub``, and checks run every
    hundredth of a second so ``hub next`` tests end quickly.

    Args:
        packed: The packed hub.
        fake: The fake herdr's folder, put first on PATH.

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
            "FAKE": str(fake),
            "PATH": f"{fake / 'bin'}{os.pathsep}{os.environ['PATH']}",
            "HERDR_PANE_ID": "w1:kanban-hub",
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


def write_card(fake: Path, key: str, sessions: list[str], log: str) -> None:
    """Add an active card for the fake card command to show.

    Args:
        fake: The fakes' folder.
        key: The card's key, such as ``DEMO-KB-001``.
        sessions: Its session IDs, oldest first.
        log: The lines of its Log section.
    """
    items = "".join(f'  - "opencode {session}"\n' for session in sessions)
    note = (
        f"---\nstage: active\nsessions:\n{items}topics: []\n---\n\n"
        f"# {key} A card\n---\n\n## Log\n---\n\n{log}"
    )
    (fake / "cards" / f"{key}.md").write_text(note)


def set_agents(fake: Path, *lists: list[dict]) -> None:
    """Make the fake herdr answer each agent list call with the next list.

    Args:
        fake: The fake herdr's folder.
        *lists: One agent list per call. The last repeats.
    """
    (fake / "agents.json").write_text(json.dumps(list(lists)))


def read_calls(fake: Path) -> list[list[str]]:
    """Read every herdr call the fake logged.

    Args:
        fake: The fake herdr's folder.

    Returns:
        The arguments of each call, in order.
    """
    log = fake / "calls.jsonl"
    if not log.exists():
        return []
    return [json.loads(line) for line in log.read_text().splitlines()]


def read_prompts(fake: Path) -> list[list[str]]:
    """Read the herdr agent prompt calls the fake logged.

    Args:
        fake: The fake herdr's folder.

    Returns:
        The arguments of each prompt call, in order.
    """
    return [call for call in read_calls(fake) if "prompt" in call]


def refuse_prompts(fake: Path, code: str, message: str) -> None:
    """Make the fake herdr answer every prompt with a Herdr error.

    Args:
        fake: The fake herdr's folder.
        code: Herdr's error code, such as ``agent_blocked``.
        message: Herdr's message.
    """
    refusal = {"code": code, "message": message}
    (fake / "prompt-refusal.json").write_text(json.dumps(refusal))


class TestStatus:
    """``hub status``, the hub's one look at all its sessions."""

    def test_status_shows_session_rows_except_hub(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """Each named session gets one row with what the hub acts on.

        The hub reads this table instead of listing files and panes by
        hand, so a wrong column makes it miss a report, send twice, or
        ignore a session that waits on the user. Its own row would make
        it watch itself.
        """
        set_agents(
            fake,
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
        (fake / "screens" / "demo-kb-007-impl").write_text(
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
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """Each active card shows whether its session waits on the user.

        Sessions that never report to the hub sat idle for many minutes
        before anyone noticed. A blocked session, an idle one outside
        the hub, and an unanswered needs-you note all wait on the user,
        while an idle worker that reports to the hub waits on the hub.
        A card shows its newest live session.
        """
        set_agents(
            fake,
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
            ],
        )
        write_card(fake, "DEMO-KB-001", ["ses_a"], "")
        write_card(fake, "DEMO-KB-002", ["ses_old", "ses_b"], "")
        write_card(
            fake,
            "DEMO-KB-003",
            ["ses_c"],
            "- 2026-11-06 needs-you: which?\n- 2026-11-06 answered: yes\n",
        )
        write_card(
            fake,
            "DEMO-KB-004",
            ["ses_d"],
            "- 2026-11-06 answered: yes\n- 2026-11-06 needs-you: which?\n",
        )
        write_card(fake, "DEMO-KB-005", ["ses_gone"], "")
        (run_folder / "report-demo-kb-003-impl.read").write_text("PR open.\n")

        result = hub("status", "--run", str(run_folder))

        cards = result.stdout.split("\n\n")[1]
        assert [line.split() for line in cards.splitlines()] == [
            ["CARD", "SESSION", "STATE", "WAITS"],
            ["DEMO-KB-001", "demo-kb-001-impl", "blocked", "you"],
            ["DEMO-KB-002", "demo-kb-002-impl", "idle", "you"],
            ["DEMO-KB-003", "demo-kb-003-impl", "idle", "hub"],
            ["DEMO-KB-004", "demo-kb-004-impl", "working", "you"],
            ["DEMO-KB-005", "-", "-", "-"],
        ]

    def test_status_keeps_sessions_when_card_fails(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """A failing card command still leaves the session table.

        Machines without the vault have no working card command, and
        the hub must still see its sessions there.
        """
        set_agents(fake, [make_agent("docs-fix", "idle")])
        (fake / "card-fails").touch()

        result = hub("status", "--run", str(run_folder))

        assert result.returncode == 0
        assert "docs-fix" in result.stdout
        assert result.stdout.endswith("cards: card: no default vault\n")


class TestSend:
    """``hub send``, the one way the hub messages a session."""

    def test_send_prompts_free_session_at_once(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """A free session gets the message once, word for word.

        A message changed by a shell would arrive garbled, and a second
        prompt would repeat it.
        """
        set_agents(fake, [make_agent("docs-fix", "idle")])
        message = 'Rebase on "main"; then reply with $HOME as typed.'

        result = hub("send", "--run", str(run_folder), "docs-fix", message)

        assert result.returncode == 0
        assert result.stdout == "sent docs-fix\n"
        prompts = read_prompts(fake)
        assert len(prompts) == 1
        assert prompts[0][:4] == ["agent", "prompt", "docs-fix", message]
        assert list(run_folder.iterdir()) == []

    def test_send_keeps_message_when_prompt_stalls(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """A stalled prompt leaves the message marked unsure.

        Herdr reports a stall after the text went in, so the session
        may have it. Without the mark the hub would send it again and
        the session would get it twice.
        """
        set_agents(fake, [make_agent("docs-fix", "idle")])
        refuse_prompts(fake, "agent_prompt_stalled", "prompt stalled")

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
        self, hub: Run, fake: Path, run_folder: Path, agent: dict
    ) -> None:
        """A busy session's message waits in the run folder.

        A prompt mid-turn gets lost in that turn, and one in a focused
        pane mixes with what the user types.
        """
        set_agents(fake, [agent])

        result = hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")

        assert result.returncode == 0
        assert result.stdout.startswith("held docs-fix")
        assert read_prompts(fake) == []
        held = json.loads((run_folder / "send-docs-fix.json").read_text())
        assert held["message"] == "Rebase."

    @pytest.mark.parametrize("suffix", ["json", "sending"])
    def test_send_refuses_while_earlier_message_waits(
        self, hub: Run, fake: Path, run_folder: Path, suffix: str
    ) -> None:
        """A second message to a session with one waiting exits 2.

        Two waiting messages would both go out once the session is
        free. A ``.sending`` file means the last send stopped part way,
        so the hub must check the pane before sending again.
        """
        set_agents(fake, [make_agent("docs-fix", "idle")])
        waiting = run_folder / f"send-docs-fix.{suffix}"
        waiting.write_text("earlier\n")

        result = hub("send", "--run", str(run_folder), "docs-fix", "Again.")

        assert result.returncode == 2
        assert waiting.read_text() == "earlier\n"
        assert read_prompts(fake) == []


class TestNext:
    """``hub next``, which the hub keeps running to learn of news."""

    def test_next_prints_unread_report(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """An unread report ends the wait with the session's name.

        This line is what starts the hub's turn when a worker finishes.
        """
        set_agents(fake, [make_agent("docs-fix", "idle")])
        (run_folder / "report-docs-fix.md").write_text("PR opened.\n")
        (run_folder / "report-old.read").write_text("Handled.\n")

        result = hub("next", "--run", str(run_folder))

        assert result.returncode == 0
        assert result.stdout == "report docs-fix\n"

    def test_next_prints_each_blocked_state_once(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """A session that stays blocked wakes the hub only once.

        Printing the same blocked state again would return at once on
        every restart, so the hub would loop on it. A later block, with
        a new state change number, must wake the hub again.
        """
        set_agents(fake, [make_agent("docs-fix", "blocked")])
        first = hub("next", "--run", str(run_folder))
        again = hub("next", "--run", str(run_folder), "--timeout", "0.1")
        set_agents(
            fake, [make_agent("docs-fix", "blocked", state_change_seq=2)]
        )
        later = hub("next", "--run", str(run_folder), "--timeout", "0.1")

        assert first.stdout == "blocked docs-fix\n"
        assert again.stdout == "timeout\n"
        assert later.stdout == "blocked docs-fix\n"

    def test_next_sends_held_message_once_free(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """A held message goes out once its session's turn ends.

        Sending it mid-turn would lose it, and sending it on every
        check would repeat it.
        """
        set_agents(fake, [make_agent("docs-fix", "working")])
        hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")
        set_agents(
            fake,
            [make_agent("docs-fix", "working")],
            [make_agent("docs-fix", "idle")],
        )

        result = hub("next", "--run", str(run_folder), "--timeout", "0.2")

        assert result.stdout == "timeout\n"
        prompts = read_prompts(fake)
        assert len(prompts) == 1
        assert prompts[0][:4] == ["agent", "prompt", "docs-fix", "Rebase."]
        assert list(run_folder.glob("send-*")) == []

    def test_next_sends_remote_message_through_machine(
        self, hub: Run, fake: Path, run_folder: Path
    ) -> None:
        """A held message for another machine goes through that machine.

        Without ``--machine`` the message would go to a session of the
        same name on this machine, or nowhere.
        """
        set_agents(fake, [make_agent("notes-fix", "working")])
        hub(
            "--machine",
            "laptop",
            "send",
            "--run",
            str(run_folder),
            "notes-fix",
            "Pull.",
        )
        set_agents(fake, [make_agent("notes-fix", "idle")])

        result = hub("next", "--run", str(run_folder), "--timeout", "0.1")

        assert result.stdout == "timeout\n"
        prompts = read_prompts(fake)
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
        fake: Path,
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
        set_agents(fake, [make_agent("docs-fix", "working")])
        hub("send", "--run", str(run_folder), "docs-fix", "Rebase.")
        set_agents(fake, [make_agent("docs-fix", "idle")])
        refuse_prompts(fake, code, "Herdr said no")

        result = hub("next", "--run", str(run_folder))

        assert result.stdout == line
        kept = json.loads((run_folder / left).read_text())
        assert kept["message"] == "Rebase."
        assert not (run_folder / "send-docs-fix.json").exists()


class TestNeedsYou:
    """``hub needs-you``, the sidebar mark for sessions the user owes."""

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
    def test_needs_you_labels_states_matching_status(
        self, hub: Run, fake: Path, status: str, labels: list[str]
    ) -> None:
        """The label goes on the session's pane, and clearing removes it.

        Herdr refuses the pane ID after --source, so a wrong order would
        leave the sidebar without the mark. A blocked session is marked
        only while blocked, so the mark does not stay once the user has
        answered and it works on.
        """
        set_agents(fake, [make_agent("docs-fix", status)])

        marked = hub("needs-you", "docs-fix")
        cleared = hub("needs-you", "--clear", "docs-fix")

        assert (marked.returncode, cleared.returncode) == (0, 0)
        pane = [
            "pane",
            "report-metadata",
            "w1:docs-fix",
            "--source",
            "pde-hub",
        ]
        assert [call for call in read_calls(fake) if call[0] == "pane"] == [
            [*pane, *labels],
            [*pane, "--clear-state-labels"],
        ]
