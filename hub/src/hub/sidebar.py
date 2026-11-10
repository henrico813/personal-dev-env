"""Run ``hub go`` and ``hub gather``, which find the hubs in the sidebar.

A hub is an agent whose name ends in ``-hub-<n>``, such as
``kanban-hub-13``. Both commands work on this machine's Herdr server
only.
"""

from __future__ import annotations

import argparse
import json
import re
import sys

from hub.herdr import call_herdr, call_socket

HUB_NAME = re.compile(r"-hub-\d+$")


def run_go(args: argparse.Namespace) -> int:
    """Show the hub after the one the user looks at, in sidebar order.

    Running it again cycles through every hub. When the user looks at
    no hub, it shows the first one.

    Args:
        args: The parsed ``go`` command line.

    Returns:
        0 once Herdr showed a hub, or 2 when no hub runs or
        ``--machine`` was given.
    """
    if refuse_other_machine(args.machine, "go"):
        return 2
    hubs = list_hubs(list_workspace_ids())
    if not hubs:
        print("hub: no hub is running", file=sys.stderr)
        return 2
    places = [place for place, hub in enumerate(hubs) if hub.get("focused")]
    target = hubs[(places[0] + 1) % len(hubs)] if places else hubs[0]
    call_herdr(None, "agent", "focus", target["name"])
    print(f"showing {target['name']}")
    return 0


def run_gather(args: argparse.Namespace) -> int:
    """Move every hub's workspace into one block at the top of the sidebar.

    Herdr puts new workspaces at the end, so hubs drift down as cards
    start. Herdr keeps the new order in its saved session.

    Args:
        args: The parsed ``gather`` command line.

    Returns:
        0 once the hubs sit at the top, or 2 when no hub runs or
        ``--machine`` was given.
    """
    if refuse_other_machine(args.machine, "gather"):
        return 2
    workspace_ids = list_workspace_ids()
    hubs = list_hubs(workspace_ids)
    if not hubs:
        print("hub: no hub is running", file=sys.stderr)
        return 2
    # Two hubs can share a workspace, and Herdr refuses repeated IDs.
    block = list(dict.fromkeys(hub["workspace_id"] for hub in hubs))
    others = [id_ for id_ in workspace_ids if id_ not in block]
    if others:
        call_socket(
            "workspace.move_block",
            {"workspace_ids": block, "before_workspace_id": others[0]},
        )
    print(f"hubs at the top: {' '.join(block)}")
    return 0


def list_hubs(workspace_ids: list[str]) -> list[dict]:
    """List this machine's hubs in sidebar order.

    Unlike ``list_agents``, this keeps the agent in this pane, since a
    hub runs ``hub gather`` from its own pane.

    Args:
        workspace_ids: The workspace IDs in sidebar order.

    Returns:
        Each hub as ``herdr agent list`` prints it, ordered by its
        workspace's place, then by name.
    """
    places = {id_: place for place, id_ in enumerate(workspace_ids)}
    agents = json.loads(call_herdr(None, "agent", "list"))["result"]["agents"]
    hubs = [
        agent for agent in agents if HUB_NAME.search(agent.get("name") or "")
    ]
    return sorted(
        hubs,
        key=lambda hub: (
            places.get(hub["workspace_id"], len(places)),
            hub["name"],
        ),
    )


def list_workspace_ids() -> list[str]:
    """List this machine's workspace IDs in sidebar order.

    Returns:
        The IDs, such as ``w4X``, top first.
    """
    workspaces = json.loads(call_herdr(None, "workspace", "list"))
    return [
        workspace["workspace_id"]
        for workspace in workspaces["result"]["workspaces"]
    ]


def refuse_other_machine(machine: str | None, command: str) -> bool:
    """Tell the user this command does not take ``--machine``.

    Each Herdr server keeps its own sidebar, and showing a hub on
    another machine from here is untested.

    Args:
        machine: The ``--machine`` value, or None for this machine.
        command: The command name, such as ``go``.

    Returns:
        True when ``--machine`` was given and the command must stop.
    """
    if machine is None:
        return False
    print(f"hub: {command} works on this machine only", file=sys.stderr)
    return True
