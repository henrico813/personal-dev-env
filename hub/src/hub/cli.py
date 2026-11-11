"""Watch, message and mark the Herdr sessions a hub session runs.

Usage::

    hub [--machine LABEL] status --run DIR   sessions, then active cards
    hub [--machine LABEL] send --run DIR NAME MESSAGE
    hub next --run DIR [--timeout SECONDS]   wait for news, print one line
    hub read --run DIR NAME [--note TEXT]    print, archive, log a report
    hub alive SESSION                        working, finished or stopped
    hub [--machine LABEL] flag NAME
    hub [--machine LABEL] unflag NAME
    hub handoff --run DIR NAME HANDOFF       start the next hub
    hub brief --run DIR                      what waits on the user
    hub clean                                close finished tabs
    hub auto ready --run DIR                 ready cards, for sorting
    hub auto check --run DIR                 check the run's card list
    hub auto approve --run DIR N REPLY       save the user's approval
    hub auto approve --run DIR --given N WORDS
    hub auto now --run DIR                   one line per run card
    hub go                                   show the next hub
    hub gather                               put hubs at the top
    hub --version

Read ``main`` first, then the ``run_<command>`` function of each command.

The hub's run folder DIR holds these files:

- ``report-NAME.md``: news from session NAME that the hub has not read.
  ``hub read`` renames it to ``report-NAME.HHMM.read``, with the time.
- ``notes.md``: the hub's log for the user. ``hub read --note`` adds
  a line to it.
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
  that wait on the user, which ``hub brief`` prints.
- ``ready.md``, ``run.md``, ``approved.md`` and ``brief-now.md``: an
  auto run's cards, as ``hub auto`` writes and reads them.

``HUB_POLL_SECONDS`` sets how often ``hub next`` and ``hub handoff``
check, 5 by default.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from hub import __version__
from hub.alive import run_alive
from hub.auto import run_approve, run_check, run_now, run_ready
from hub.brief import run_brief
from hub.clean import run_clean
from hub.flag import run_flag, run_unflag
from hub.handoff import run_handoff
from hub.herdr import HerdrError
from hub.read import run_read
from hub.send import run_next, run_send
from hub.sidebar import run_gather, run_go
from hub.status import CardError, run_status

DEPRECATION = (
    "hub-py is deprecated and will be removed;"
    " full installs use hub, the Rust hub"
)


def main(argv: list[str] | None = None) -> int:
    """Warn that hub-py is deprecated, then run one hub command.

    Args:
        argv: The arguments after ``hub``, or None for ``sys.argv``.

    Returns:
        0 when the command did its work, 1 when a herdr or card call
        failed, and 2 when the command was refused.
    """
    print(DEPRECATION, file=sys.stderr)
    args = parse_args(argv)
    try:
        return args.command(args)
    except (HerdrError, CardError) as error:
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

    read = commands.add_parser("read", help="print, archive and log a report")
    read.add_argument("--run", type=Path, required=True)
    read.add_argument("name")
    read.add_argument("--note", help="a line to add to notes.md")
    read.set_defaults(command=run_read)

    alive = commands.add_parser("alive", help="whether a session still works")
    alive.add_argument("session", help="an OpenCode session ID")
    alive.set_defaults(command=run_alive)

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

    brief = commands.add_parser("brief", help="what waits on the user")
    brief.add_argument("--run", type=Path, required=True)
    brief.set_defaults(command=run_brief)

    clean = commands.add_parser("clean", help="close finished tabs")
    clean.set_defaults(command=run_clean)

    auto = commands.add_parser("auto", help="the steps of an auto run")
    steps = auto.add_subparsers(required=True)
    for name, command, summary in (
        ("ready", run_ready, "write the ready cards to ready.md"),
        ("check", run_check, "check run.md against the ready cards"),
        ("approve", run_approve, "save run.md with the user's reply"),
        ("now", run_now, "write and print brief-now.md"),
    ):
        step = steps.add_parser(name, help=summary)
        step.add_argument("--run", type=Path, required=True)
        step.set_defaults(command=command)
        if name == "approve":
            step.add_argument(
                "--given",
                action="store_true",
                help="the reply is the user's own words, naming the cards",
            )
            step.add_argument("number", type=int)
            step.add_argument("reply", help="the user's reply, word for word")
    go = commands.add_parser("go", help="show the next hub")
    go.set_defaults(command=run_go)

    gather = commands.add_parser("gather", help="put hubs at the top")
    gather.set_defaults(command=run_gather)

    return parser.parse_args(argv)
