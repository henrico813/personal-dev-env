"""Run ``hub alive``, which tells whether a session still works.

It reads the session's newest messages from OpenCode's database, so it
sees workers and subagents that Herdr does not list.
"""

from __future__ import annotations

import argparse
import datetime
import json
import sys
import time

from hub.opencode import query_session

# A session with no tool running that changed nothing for this long has
# stopped.
QUIET_MS = 2 * 60 * 1000


def run_alive(args: argparse.Namespace) -> int:
    """Print one line on the session, ending in its state.

    The line reads ``412 messages, newest 14:03:22, last tool shell,
    working``. The newest time is local.

    Args:
        args: The parsed ``alive`` command line.

    Returns:
        0 after printing the line, 1 when this machine's database has
        no such session, and 2 with ``--machine``.
    """
    if args.machine is not None:
        print(
            "hub: alive reads this machine's OpenCode database; run it there",
            file=sys.stderr,
        )
        return 2
    rows = query_session(
        "select type, time_updated, data from session_message"
        " where session_id = ? order by seq desc",
        args.session,
    )
    if not rows:
        print(
            f"hub: no OpenCode session {args.session} on this machine",
            file=sys.stderr,
        )
        return 1
    newest = datetime.datetime.fromtimestamp(rows[0][1] / 1000)
    state = describe_activity(rows, time.time() * 1000)
    print(
        f"{len(rows)} messages, newest {newest:%H:%M:%S},"
        f" last tool {find_last_tool(rows)}, {state}"
    )
    return 0


def describe_activity(rows: list[tuple], now_ms: float) -> str:
    """Tell whether the session works, finished its turn or stopped.

    An unfinished step alone does not mean the session works, because
    a session killed in the middle of a step keeps that step forever.
    A running tool does, because a parent that waits on a subagent
    changes nothing for many minutes.

    Args:
        rows: Each message's type, last change in milliseconds since
            1970, and data, newest first.
        now_ms: The time now, in milliseconds since 1970.

    Returns:
        ``working``, ``finished`` or ``stopped``.
    """
    kind, updated, data = rows[0]
    message = json.loads(data)
    if kind == "idle":
        if message.get("outcome") == "succeeded":
            return "finished"
        return "stopped"
    if kind == "assistant" and any(
        (part.get("state") or {}).get("status") == "running"
        for part in message.get("content") or []
    ):
        return "working"
    if now_ms - updated < QUIET_MS:
        return "working"
    return "stopped"


def find_last_tool(rows: list[tuple]) -> str:
    """Find the name of the newest tool the session called.

    Args:
        rows: Each message's type, last change and data, newest first.

    Returns:
        The tool's name, such as ``shell``, or ``-`` when it called
        none.
    """
    for kind, _, data in rows:
        if kind != "assistant":
            continue
        tools = [
            part
            for part in json.loads(data).get("content") or []
            if part.get("type") == "tool"
        ]
        if tools:
            return tools[-1].get("name", "-")
    return "-"
