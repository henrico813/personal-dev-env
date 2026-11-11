"""Run ``hub send`` and ``hub next``, which never lose or repeat a message.

``hub send`` holds a message for a busy session in the run folder, and
the ``hub next`` that runs in the background sends it once the session
is free.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from pathlib import Path

from hub.herdr import (
    FREE_STATES,
    HerdrError,
    call_herdr,
    get_agent,
    list_agents,
)

# Herdr gives these errors after the text went in, so it may have arrived.
UNSURE_ERRORS = ("agent_prompt_stalled", "timeout")
# Long enough for a session to start its turn, as herdr-handoff waits.
PROMPT_TIMEOUT_MS = "30000"
# OpenCode's input box shows a long message as "┃  [Pasted ~1 lines]".
PASTED_TEXT = re.compile(r"^\s*┃\s+\[Pasted ~\d+ lines?\]\s*$", re.MULTILINE)


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

    A stalled prompt can leave the message pasted in the input box but
    not sent. One Enter key press then sends it.

    Args:
        machine: A saved Herdr machine, or None for this one.
        name: The agent's name.
        message: The text to send, passed to Herdr unchanged.

    Raises:
        HerdrError: Herdr refused the message, or the session did not
            start on it in time.
    """
    try:
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
    except HerdrError as error:
        if error.code != "agent_prompt_stalled":
            raise
        if not submit_pasted_text(machine, name):
            raise


def submit_pasted_text(machine: str | None, name: str) -> bool:
    """Press Enter once when the session's input box holds pasted text.

    Args:
        machine: A saved Herdr machine, or None for this one.
        name: The agent's name.

    Returns:
        True when the session started on the text after the key press.
        False when the screen showed no pasted text, the session was no
        longer free, or a herdr call failed, so the message may or may
        not have arrived.
    """
    try:
        screen = call_herdr(
            machine, "agent", "read", name, "--source", "visible"
        )
        if not PASTED_TEXT.search(screen):
            return False
        # The user may have focused the pane during the stall, and Enter
        # would then send their typing too.
        if not is_free(get_agent(machine, name)):
            return False
        call_herdr(machine, "agent", "send-keys", name, "Enter")
        call_herdr(
            machine,
            "agent",
            "wait",
            name,
            "--until",
            "working",
            "--until",
            "blocked",
            "--timeout",
            PROMPT_TIMEOUT_MS,
        )
    except HerdrError:
        return False
    return True


def write_file_whole(path: Path, text: str) -> None:
    """Write the file so a reader never sees half of it.

    Args:
        path: The file to replace.
        text: Its new text, without the last newline.
    """
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(text + "\n")
    temporary.replace(path)
