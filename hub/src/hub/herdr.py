"""Run herdr and read the agents it lists."""

from __future__ import annotations

import json
import os
import subprocess

# Herdr takes a prompt only in these states.
FREE_STATES = ("idle", "done")
NEEDS_YOU = "needs you"


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


def read_label(agent: dict) -> str:
    """Read the label the pane shows for its present state.

    Args:
        agent: The agent as Herdr prints it.

    Returns:
        The label, such as ``needs you``, or ``-``.
    """
    labels = agent.get("state_labels") or {}
    return labels.get(agent["agent_status"], "-")


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
