"""Run herdr and read the agents it lists."""

from __future__ import annotations

import json
import os
import socket
import subprocess
from pathlib import Path

# Herdr takes a prompt only in these states.
FREE_STATES = ("idle", "done")
NEEDS_YOU = "needs you"
# Herdr's own socket when HERDR_SOCKET_PATH is not set.
DEFAULT_SOCKET = Path.home() / ".config" / "herdr" / "herdr.sock"
SOCKET_TIMEOUT_SECONDS = 10


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


def call_socket(method: str, params: dict) -> dict:
    """Send one request to this machine's Herdr server and return its result.

    Use it only for calls the herdr command lacks, such as
    ``workspace.move_block``. Herdr sets ``HERDR_SOCKET_PATH`` in its
    panes and key commands.

    Args:
        method: The socket API method, such as ``workspace.move_block``.
        params: The method's parameters.

    Returns:
        The ``result`` of Herdr's answer.

    Raises:
        HerdrError: The socket could not be reached, Herdr refused the
            request, or Herdr gave no usable answer. The message is
            Herdr's own when it gave one.
    """
    path = os.environ.get("HERDR_SOCKET_PATH") or str(DEFAULT_SOCKET)
    request = {"id": "hub", "method": method, "params": params}
    try:
        with socket.socket(socket.AF_UNIX) as connection:
            connection.settimeout(SOCKET_TIMEOUT_SECONDS)
            connection.connect(path)
            connection.sendall(json.dumps(request).encode() + b"\n")
            with connection.makefile("rb") as reader:
                answer = reader.readline()
    except OSError as error:
        raise HerdrError(f"cannot reach Herdr at {path}: {error}") from error
    try:
        reply = json.loads(answer)
        if "error" in reply:
            refusal = reply["error"]
            raise HerdrError(refusal["message"], refusal.get("code", ""))
        return reply["result"]
    except (ValueError, KeyError, TypeError) as error:
        raise HerdrError(f"Herdr gave no usable answer: {answer!r}") from error
