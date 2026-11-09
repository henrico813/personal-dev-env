"""Run ``hub needs-you``, the sidebar mark for sessions the user owes."""

from __future__ import annotations

import argparse

from hub.herdr import FREE_STATES, NEEDS_YOU, call_herdr, get_agent

LABEL_SOURCE = "pde-hub"


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
