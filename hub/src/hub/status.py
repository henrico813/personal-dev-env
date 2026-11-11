"""Print ``hub status``, one table of sessions and one of active cards."""

from __future__ import annotations

import argparse
import re
import sqlite3
import subprocess
from pathlib import Path

from hub.herdr import FREE_STATES, NEEDS_YOU, list_agents, read_label
from hub.opencode import read_context_tokens
from hub.send import describe_send

# The card command allows digits in a project code, such as DEMO2-KB-001.
CARD_KEY = re.compile(r"[a-z][a-z0-9]*-kb-\d{3,}")
# card list prints one card per line, starting with its key.
CARD_LINE = re.compile(r"[A-Z][A-Z0-9]*-KB-\d{3,} ")
# card note --needs-you and card note --answered write these Log lines.
NEEDS_YOU_NOTE = re.compile(r"^- \S+ needs-you: ", re.MULTILINE)
ANSWERED_NOTE = re.compile(r"^- \S+ answered: ", re.MULTILINE)


class CardError(Exception):
    """A card command that failed or is not installed."""


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
                describe_context(args.machine, agent),
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


def describe_context(machine: str | None, agent: dict) -> str:
    """Describe the agent's context size from OpenCode's database.

    The pane's footer counts some steps twice, so it is not read.

    Args:
        machine: A saved Herdr machine, or None for this one. Only this
            machine's database can be read.
        agent: The agent as Herdr prints it.

    Returns:
        The size in thousands of tokens, such as ``108k``, or ``-``
        when it is unknown.
    """
    session = (agent.get("agent_session") or {}).get("value")
    if machine is not None or not session:
        return "-"
    try:
        tokens = read_context_tokens(session)
    except sqlite3.Error:
        # A locked or changed database costs one column, not the table.
        return "-"
    return "-" if tokens is None else f"{round(tokens / 1000)}k"


def print_table(rows: list[tuple]) -> None:
    """Print rows as columns two spaces apart.

    Args:
        rows: Rows of text cells, all of the same length.
    """
    widths = [max(len(row[i]) for row in rows) for i in range(len(rows[0]))]
    for row in rows:
        cells = [cell.ljust(width) for cell, width in zip(row, widths)]
        print("  ".join(cells).rstrip())


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
