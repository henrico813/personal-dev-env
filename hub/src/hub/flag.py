"""Run ``hub flag`` and ``hub unflag``, the sidebar mark for sessions."""

from __future__ import annotations

import argparse

from hub.herdr import FREE_STATES, NEEDS_YOU, call_herdr, get_agent

LABEL_SOURCE = "pde-hub"


def run_flag(args: argparse.Namespace) -> int:
    """Show "needs you" as a session's state in the sidebar.

    A blocked session gets the mark on ``blocked`` only, so it goes
    once the session works again. An idle or done session gets it on
    both ``idle`` and ``done``, and stays marked until unflagged.

    Args:
        args: The parsed ``flag`` command line.

    Returns:
        0, once Herdr took the change.
    """
    agent = get_agent(args.machine, args.name)
    status = agent["agent_status"]
    states = FREE_STATES if status in FREE_STATES else (status,)
    labels = []
    for state in states:
        labels += ["--state-label", f"{state}={NEEDS_YOU}"]
    report_pane_labels(args.machine, agent["pane_id"], labels)
    print(f"flagged {args.name}")
    return 0


def run_unflag(args: argparse.Namespace) -> int:
    """Remove the "needs you" mark from a session's pane.

    Args:
        args: The parsed ``unflag`` command line.

    Returns:
        0, once Herdr took the change.
    """
    agent = get_agent(args.machine, args.name)
    report_pane_labels(
        args.machine, agent["pane_id"], ["--clear-state-labels"]
    )
    print(f"unflagged {args.name}")
    return 0


def report_pane_labels(
    machine: str | None, pane_id: str, options: list[str]
) -> None:
    """Send one pane's label change to Herdr.

    Args:
        machine: The saved Herdr machine, or None for this one.
        pane_id: The pane to change.
        options: The label options, such as ``--clear-state-labels``.
    """
    # Herdr takes the pane ID before --source and refuses it after.
    call_herdr(
        machine,
        "pane",
        "report-metadata",
        pane_id,
        "--source",
        LABEL_SOURCE,
        *options,
    )
