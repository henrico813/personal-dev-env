"""Watch, message and mark the Herdr sessions a hub session runs.

Usage::

    hub [--machine LABEL] status --run DIR   sessions, then active cards
    hub [--machine LABEL] send --run DIR NAME MESSAGE
    hub next --run DIR [--timeout SECONDS]   wait for news, print one line
    hub [--machine LABEL] flag NAME
    hub [--machine LABEL] unflag NAME
    hub handoff --run DIR NAME HANDOFF       start the next hub
    hub morning --run DIR                    what waits on the user
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
- ``rules.md``: the user's standing decisions. ``hub handoff`` starts
  the next hub on the model of its last ``--model '...'``.
- ``reviews.md`` and ``decisions.md``: the pull requests and questions
  that wait on the user, which ``hub morning`` prints.

``HUB_POLL_SECONDS`` sets how often ``hub next`` and ``hub handoff``
check, 5 by default.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from hub import __version__
from hub.flag import run_flag, run_unflag
from hub.handoff import run_handoff
from hub.herdr import HerdrError
from hub.morning import run_morning
from hub.send import run_next, run_send
from hub.status import run_status


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

    flag = commands.add_parser("flag", help="mark a session's pane")
    flag.add_argument("name")
    flag.set_defaults(command=run_flag)

    unflag = commands.add_parser("unflag", help="clear a session's mark")
    unflag.add_argument("name")
    unflag.set_defaults(command=run_unflag)

    handoff = commands.add_parser(
        "handoff", help="start the next hub, check its effort"
    )
    handoff.add_argument("--run", type=Path, required=True)
    handoff.add_argument("name")
    handoff.add_argument("handoff", type=Path)
    handoff.set_defaults(command=run_handoff)

    morning = commands.add_parser("morning", help="what waits on the user")
    morning.add_argument("--run", type=Path, required=True)
    morning.set_defaults(command=run_morning)

    return parser.parse_args(argv)
