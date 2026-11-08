"""Watch, message and mark the Herdr sessions a hub session runs.

Usage::

    hub [--machine LABEL] status --run DIR   sessions, then active cards
    hub [--machine LABEL] send --run DIR NAME MESSAGE
    hub next --run DIR [--timeout SECONDS]   wait for news, print one line
    hub [--machine LABEL] needs-you [--clear] NAME
    hub --version

Read ``main`` first, then the ``run_<command>`` function of each command.

The hub's run folder DIR holds these files:

- ``report-NAME.md``: news from session NAME that the hub has not read.
  The hub renames it to ``report-NAME.read`` once handled.
- ``send-NAME.json``: a message that waits until NAME is free. The
  ``hub next`` that runs in the background sends it.
- ``send-NAME.sending``: a message being sent. One left behind means
  a send stopped part way, so the message may have arrived.
- ``send-NAME.failed``: a message Herdr refused before taking it, with
  the reason.
- ``next-seen.json``: the blocked states ``hub next`` already printed.

``HUB_POLL_SECONDS`` sets how often ``hub next`` checks, 5 by default.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

from hub import __version__

# Herdr takes a prompt only in these states.
FREE_STATES = ("idle", "done")
NEEDS_YOU = "needs you"
LABEL_SOURCE = "pde-hub"
# Herdr gives these errors after the text went in, so it may have arrived.
UNSURE_ERRORS = ("agent_prompt_stalled", "timeout")
# OpenCode's footer shows the context size as "98.3K (10%)".
CONTEXT_SIZE = re.compile(r"(\d+(?:\.\d+)?[KM]) \(\d+%\)")
# The card command allows digits in a project code, such as DEMO2-KB-001.
CARD_KEY = re.compile(r"[a-z][a-z0-9]*-kb-\d{3,}")
# Long enough for a session to start its turn, as herdr-handoff waits.
PROMPT_TIMEOUT_MS = "30000"
# card list prints one card per line, starting with its key.
CARD_LINE = re.compile(r"[A-Z][A-Z0-9]*-KB-\d{3,} ")
# card note --needs-you and card note --answered write these Log lines.
NEEDS_YOU_NOTE = re.compile(r"^- \S+ needs-you: ", re.MULTILINE)
ANSWERED_NOTE = re.compile(r"^- \S+ answered: ", re.MULTILINE)


class CardError(Exception):
    """A card command that failed or is not installed."""


class HerdrError(Exception):
    """A herdr command that failed, with Herdr's own message.

    Attributes:
        code: Herdr's error code, such as ``agent_blocked``, or an empty
            string when Herdr gave none.
    """

    def __init__(self, message: str, code: str = "") -> None:
        """Keep Herdr's message and error code.

        Args:
            message: Herdr's message, shown to the hub.
            code: Herdr's error code, or an empty string.
        """
        super().__init__(message)
        self.code = code


def main(argv: list[str] | None = None) -> int:
    """Run one hub command and return its exit code.

    Args:
        argv: The arguments after ``hub``, or None for ``sys.argv``.

    Returns:
        0 when the command did its work, 1 when a herdr call failed, and
        2 when the command was refused.
    """
    args = parse_args(argv)
    try:
        return args.command(args)
    except HerdrError as error:
        print(f"hub: {error}", file=sys.stderr)
        return 1


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse the command line.

    Args:
        argv: The arguments after ``hub``, or None for ``sys.argv``.

    Returns:
        The parsed arguments. ``command`` holds the function that runs
        the chosen command.
    """
    parser = argparse.ArgumentParser(
        prog="hub",
        description="Watch, message and mark the sessions a hub runs.",
    )
    parser.add_argument("--version", action="version", version=__version__)
    parser.add_argument(
        "--machine", help="a saved Herdr machine, as herdr --machine takes"
    )
    commands = parser.add_subparsers(required=True)

    status = commands.add_parser("status", help="one line per session")
    status.add_argument("--run", type=Path, required=True)
    status.set_defaults(command=run_status)

    send = commands.add_parser("send", help="send now, or hold until free")
    send.add_argument("--run", type=Path, required=True)
    send.add_argument("name")
    send.add_argument("message")
    send.set_defaults(command=run_send)

    wait = commands.add_parser("next", help="wait for news, print one line")
    wait.add_argument("--run", type=Path, required=True)
    wait.add_argument("--timeout", type=float, help="give up after SECONDS")
    wait.set_defaults(command=run_next)

    label = commands.add_parser("needs-you", help="mark a session's pane")
    label.add_argument("--clear", action="store_true")
    label.add_argument("name")
    label.set_defaults(command=run_needs_you)

    return parser.parse_args(argv)


def run_status(args: argparse.Namespace) -> int:
    """Print one line per named session, then one per active card.

    The card lines cover sessions that do not report to the hub, so
    the hub sees every card whose session waits on the user.

    Args:
        args: The parsed ``status`` command line.

    Returns:
        0, after printing the tables.
    """
    agents = list_agents(args.machine)
    rows = [("NAME", "STATE", "CTX", "CARD", "REPORT", "SEND", "LABEL")]
    for agent in agents:
        name = agent["name"]
        card = CARD_KEY.match(name)
        report = args.run / f"report-{name}.md"
        rows.append(
            (
                name,
                agent["agent_status"],
                read_context_size(args.machine, name),
                card.group().upper() if card else "-",
                "unread" if report.exists() else "-",
                describe_send(args.run, name),
                read_label(agent),
            )
        )
    print_table(rows)
    print()
    try:
        print_table(describe_active_cards(args.run, agents))
    except CardError as error:
        print(f"cards: {error}")
    return 0


def run_send(args: argparse.Namespace) -> int:
    """Send a message now when its session is free, or hold it.

    A held message goes out from the ``hub next`` that runs in the
    background, so a session never gets one message twice.

    Args:
        args: The parsed ``send`` command line.

    Returns:
        0 when the message was sent or held, 2 when one to the same
        session already waits.

    Raises:
        HerdrError: Herdr did not find the session or did not take the
            message. When the message may have arrived, it stays in
            ``send-NAME.sending``.
    """
    held = args.run / f"send-{args.name}.json"
    sending = held.with_suffix(".sending")
    for path in (held, sending):
        if path.exists():
            print(
                f"hub: a message to {args.name} waits in {path}",
                file=sys.stderr,
            )
            return 2
    held.with_suffix(".failed").unlink(missing_ok=True)
    message = {
        "name": args.name,
        "machine": args.machine,
        "message": args.message,
    }

    agent = get_agent(args.machine, args.name)
    if not is_free(agent):
        write_file_whole(held, json.dumps(message))
        print(f"held {args.name}: it is {describe_state(agent)}")
        return 0
    try:
        prompt_agent(args.machine, args.name, args.message)
    except HerdrError as error:
        if error.code in UNSURE_ERRORS:
            write_file_whole(sending, json.dumps(message))
        raise
    print(f"sent {args.name}")
    return 0


def run_next(args: argparse.Namespace) -> int:
    """Wait until a session reports, turns blocked or a send goes wrong.

    Prints one line, such as ``report demo-kb-007-impl``, so the
    command ending starts the hub's turn. Held messages go out while it
    waits. A blocked state is printed once, so a session that stays
    blocked does not wake the hub again.

    Args:
        args: The parsed ``next`` command line.

    Returns:
        0, after printing the line.
    """
    poll_seconds = float(os.environ.get("HUB_POLL_SECONDS", "5"))
    deadline = None
    if args.timeout is not None:
        deadline = time.monotonic() + args.timeout
    seen_path = args.run / "next-seen.json"
    seen = json.loads(seen_path.read_text()) if seen_path.exists() else {}
    while True:
        reports = sorted(args.run.glob("report-*.md"))
        if reports:
            print(f"report {reports[0].stem.removeprefix('report-')}")
            return 0
        agents = {agent["name"]: agent for agent in list_agents(None)}
        problem = send_held_messages(args.run, agents)
        if problem:
            print(problem)
            return 0
        for name, agent in sorted(agents.items()):
            change = agent.get("state_change_seq")
            if agent["agent_status"] == "blocked" and seen.get(name) != change:
                seen[name] = change
                write_file_whole(seen_path, json.dumps(seen))
                print(f"blocked {name}")
                return 0
        if deadline is not None and time.monotonic() >= deadline:
            print("timeout")
            return 0
        time.sleep(poll_seconds)


def run_needs_you(args: argparse.Namespace) -> int:
    """Show "needs you" as a session's state in the sidebar, or clear it.

    A blocked session gets the mark on ``blocked`` only, so it goes
    once the session works again. An idle or done session gets it on
    both ``idle`` and ``done``, and stays marked until cleared.

    Args:
        args: The parsed ``needs-you`` command line.

    Returns:
        0, once Herdr took the change.
    """
    agent = get_agent(args.machine, args.name)
    # Herdr takes the pane ID before --source and refuses it after.
    command = [
        "pane",
        "report-metadata",
        agent["pane_id"],
        "--source",
        LABEL_SOURCE,
    ]
    if args.clear:
        command.append("--clear-state-labels")
    elif agent["agent_status"] in FREE_STATES:
        for state in FREE_STATES:
            command += ["--state-label", f"{state}={NEEDS_YOU}"]
    else:
        command += ["--state-label", f"{agent['agent_status']}={NEEDS_YOU}"]
    call_herdr(args.machine, *command)
    print(f"{'cleared' if args.clear else 'marked'} {args.name}")
    return 0


def describe_active_cards(run: Path, agents: list[dict]) -> list[tuple]:
    """Describe each active card's live session and who it waits on.

    Args:
        run: The hub's run folder.
        agents: The named agents ``hub status`` lists.

    Returns:
        A heading row, then one row per active card. A card's session is
        its newest one that Herdr still runs.

    Raises:
        CardError: The card command failed or is not installed.
    """
    by_session = {
        agent.get("agent_session", {}).get("value"): agent for agent in agents
    }
    rows = [("CARD", "SESSION", "STATE", "WAITS")]
    for line in call_card("list", "--stage", "active").splitlines():
        if not CARD_LINE.match(line):
            continue
        key = line.split()[0]
        sessions, note_open = read_card(key)
        live = [
            by_session[session]
            for session in sessions
            if session in by_session
        ]
        agent = live[-1] if live else None
        rows.append(
            (
                key,
                agent["name"] if agent else "-",
                agent["agent_status"] if agent else "-",
                find_who_waits(run, agent, note_open),
            )
        )
    return rows


def find_who_waits(run: Path, agent: dict | None, note_open: bool) -> str:
    """Tell who a card's session waits on.

    An idle session that never reported to this hub has no one but the
    user to answer it, so it counts as waiting on the user.

    Args:
        run: The hub's run folder.
        agent: The card's live session, or None.
        note_open: Whether the card has a needs-you note with no answer.

    Returns:
        ``you`` when it waits on the user, ``hub`` when it waits on the
        hub, or ``-`` when it works or no session runs.
    """
    if note_open:
        return "you"
    if agent is None:
        return "-"
    if agent["agent_status"] == "blocked" or read_label(agent) == NEEDS_YOU:
        return "you"
    if agent["agent_status"] not in FREE_STATES:
        return "-"
    if any(run.glob(f"report-{agent['name']}.*")):
        return "hub"
    return "you"


def read_card(key: str) -> tuple[list[str], bool]:
    """Read a card's sessions and whether a needs-you note is open.

    Args:
        key: The card's key, such as ``DEMO-KB-007``.

    Returns:
        The card's session IDs, oldest first, and whether its Log has a
        needs-you line after its last answered line.

    Raises:
        CardError: The card command failed or is not installed.
    """
    note = call_card("show", key)
    lines = note.splitlines()
    front_end = lines.index("---", 1) if "---" in lines[1:] else 0
    sessions = []
    in_sessions = False
    for line in lines[1:front_end]:
        if not line.startswith(" "):
            in_sessions = line == "sessions:"
        elif in_sessions and line.lstrip().startswith("- "):
            # Each item reads "opencode ses_..." with the ID last.
            sessions.append(line.split()[-1].strip('"'))
    asked = [match.start() for match in NEEDS_YOU_NOTE.finditer(note)]
    answered = [match.start() for match in ANSWERED_NOTE.finditer(note)]
    note_open = bool(asked) and asked[-1] > max(answered, default=-1)
    return sessions, note_open


def send_held_messages(run: Path, agents: dict[str, dict]) -> str | None:
    """Send each held message whose session is free.

    Args:
        run: The hub's run folder.
        agents: This machine's named agents, by name. Agents on other
            machines are read from those machines.

    Returns:
        A line naming the first message that went wrong, or None. It
        starts with ``failed`` when Herdr refused the message before
        taking it, and ``unsure`` when it may have arrived.
    """
    remote: dict[str, dict[str, dict]] = {}
    for held in sorted(run.glob("send-*.json")):
        message = json.loads(held.read_text())
        machine = message["machine"]
        if machine is None:
            agent = agents.get(message["name"])
        else:
            if machine not in remote:
                remote[machine] = {
                    found["name"]: found for found in list_agents(machine)
                }
            agent = remote[machine].get(message["name"])
        if agent is not None and not is_free(agent):
            continue
        sending = held.with_suffix(".sending")
        try:
            # A second hub next that took the file first sends it instead.
            held.rename(sending)
        except FileNotFoundError:
            continue
        try:
            if agent is None:
                raise HerdrError("the session is gone")
            prompt_agent(machine, message["name"], message["message"])
        except HerdrError as error:
            if error.code in UNSURE_ERRORS:
                return f"unsure {message['name']}: {error}"
            message["error"] = str(error)
            write_file_whole(held.with_suffix(".failed"), json.dumps(message))
            sending.unlink()
            return f"failed {message['name']}: {error}"
        sending.unlink()
    return None


def list_agents(machine: str | None) -> list[dict]:
    """List Herdr's named agents, leaving out the one in this pane.

    Args:
        machine: A saved Herdr machine, or None for this one.

    Returns:
        Each agent as ``herdr agent list`` prints it.
    """
    agents = json.loads(call_herdr(machine, "agent", "list"))["result"]
    own_pane = os.environ.get("HERDR_PANE_ID") if machine is None else None
    return [
        agent
        for agent in agents["agents"]
        if agent.get("name") and agent.get("pane_id") != own_pane
    ]


def get_agent(machine: str | None, name: str) -> dict:
    """Look up one agent.

    Args:
        machine: A saved Herdr machine, or None for this one.
        name: The agent's name.

    Returns:
        The agent as ``herdr agent get`` prints it.

    Raises:
        HerdrError: No live agent has that name.
    """
    return json.loads(call_herdr(machine, "agent", "get", name))["result"][
        "agent"
    ]


def is_free(agent: dict) -> bool:
    """Tell whether a prompt can go to the agent now.

    A focused pane is not free, so a message does not land in the
    middle of what the user types there.

    Args:
        agent: The agent as Herdr prints it.

    Returns:
        True when the agent is idle or done and its pane is not focused.
    """
    return agent["agent_status"] in FREE_STATES and not agent.get("focused")


def describe_state(agent: dict) -> str:
    """Describe why the agent cannot take a message now.

    Args:
        agent: The agent as Herdr prints it.

    Returns:
        Its state, noting when the user is in its pane.
    """
    if agent.get("focused"):
        return f"{agent['agent_status']} with the user in its pane"
    return agent["agent_status"]


def prompt_agent(machine: str | None, name: str, message: str) -> None:
    """Send a message and wait until the session starts on it.

    Args:
        machine: A saved Herdr machine, or None for this one.
        name: The agent's name.
        message: The text to send, passed to Herdr unchanged.

    Raises:
        HerdrError: Herdr refused the message, or the session did not
            start on it in time.
    """
    call_herdr(
        machine,
        "agent",
        "prompt",
        name,
        message,
        "--wait",
        "--until",
        "working",
        "--until",
        "blocked",
        "--timeout",
        PROMPT_TIMEOUT_MS,
    )


def read_context_size(machine: str | None, name: str) -> str:
    """Read the context size from the session's footer.

    Args:
        machine: A saved Herdr machine, or None for this one.
        name: The agent's name.

    Returns:
        The size, such as ``98.3K``, or ``-`` when the screen shows
        none or cannot be read.
    """
    try:
        screen = call_herdr(
            machine, "agent", "read", name, "--source", "visible"
        )
    except HerdrError:
        return "-"
    sizes = CONTEXT_SIZE.findall(screen)
    return sizes[-1] if sizes else "-"


def describe_send(run: Path, name: str) -> str:
    """Describe what the run folder holds for a message to the session.

    Args:
        run: The hub's run folder.
        name: The agent's name.

    Returns:
        ``unsure``, ``held``, ``failed`` or ``-``.
    """
    for suffix, word in (
        (".sending", "unsure"),
        (".json", "held"),
        (".failed", "failed"),
    ):
        if (run / f"send-{name}{suffix}").exists():
            return word
    return "-"


def read_label(agent: dict) -> str:
    """Read the label the pane shows for its present state.

    Args:
        agent: The agent as Herdr prints it.

    Returns:
        The label, such as ``needs you``, or ``-``.
    """
    labels = agent.get("state_labels") or {}
    return labels.get(agent["agent_status"], "-")


def print_table(rows: list[tuple]) -> None:
    """Print rows as columns two spaces apart.

    Args:
        rows: Rows of text cells, all of the same length.
    """
    widths = [max(len(row[i]) for row in rows) for i in range(len(rows[0]))]
    for row in rows:
        cells = [cell.ljust(width) for cell, width in zip(row, widths)]
        print("  ".join(cells).rstrip())


def write_file_whole(path: Path, text: str) -> None:
    """Write the file so a reader never sees half of it.

    Args:
        path: The file to replace.
        text: Its new text, without the last newline.
    """
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(text + "\n")
    temporary.replace(path)


def call_card(*args: str) -> str:
    """Run the card command and return what it prints.

    Args:
        *args: The card arguments, such as ``show DEMO-KB-007``.

    Returns:
        Card's standard output.

    Raises:
        CardError: card is not installed or exited nonzero.
    """
    try:
        result = subprocess.run(
            ["card", *args], capture_output=True, text=True, check=False
        )
    except FileNotFoundError:
        raise CardError("the card command is not installed") from None
    if result.returncode != 0:
        raise CardError(result.stderr.strip() or "card failed")
    return result.stdout


def call_herdr(machine: str | None, *args: str) -> str:
    """Run herdr and return what it prints.

    Args:
        machine: A saved Herdr machine, or None for this one.
        *args: The herdr arguments, such as ``agent list``.

    Returns:
        Herdr's standard output.

    Raises:
        HerdrError: herdr exited nonzero. The message is Herdr's own,
            such as "agent target docs-fix not found".
    """
    command = ["herdr"]
    if machine is not None:
        command += ["--machine", machine]
    result = subprocess.run(
        [*command, *args], capture_output=True, text=True, check=False
    )
    if result.returncode == 0:
        return result.stdout
    try:
        error = json.loads(result.stdout)["error"]
        message, code = error["message"], error.get("code", "")
    except (ValueError, KeyError, TypeError):
        message = result.stderr.strip() or f"herdr exited {result.returncode}"
        code = ""
    raise HerdrError(message, code)
