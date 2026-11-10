"""Run ``hub handoff``, which starts the next hub and checks its effort.

The old hub writes the next hub's handoff with only the run's state.
This command adds the rest, starts the next hub on the model the
user's rules give, and reads back the reasoning effort its first reply
ran at. A hub once told the user the wrong effort because nothing
checked it.
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import re
import sqlite3
import sys
import time
from contextlib import closing
from pathlib import Path

from hub.herdr import HerdrError, call_herdr, get_agent
from hub.send import prompt_agent

# rules.md names the hub's model as --model '<provider>/<model>#<effort>'.
RULES_MODEL = re.compile(r"--model '([^']+)'")
# Long enough for OpenCode to load the folder and open the session.
START_TIMEOUT_MS = "90000"
REPLY_TIMEOUT_SECONDS = 120

REVIEW_HANDOFF = """\
# Handoff: session review of the hub {old}

You are a new session, agent `{reviewer}`, started by the hub
`{new}`. Review the hub session `{old}`, which ran the hub in Herdr
workspace `{workspace}` and handed off to `{new}` on {date}. Its
OpenCode session ID is `{session}`. Its tab is closed; do not look for
it in Herdr.

Load `session-review` now.

## The session to review

Follow `session-review` with `--session {session}`. The hub's run
folder `{run}` holds `notes.md` (a dated log) and `rules.md` (the
user's standing decisions).
{previous}
## Your work

Follow `session-review`: numbers, at most three proposals, lessons as
a pick list, local models. Also judge the autonomous workflow in its
own section:

- what ran without the user
- what still needed the user, and whether it had to
- mistakes made without the user, and how the user would have caught
  them
- proposals that cut both the user's load and those mistakes

Save the review as `{review}`, have it checked by one fresh subagent
as the skill says, then report in the skill's output format.

## Coordination

- Read: the OpenCode database (read-only), the run folder, the
  handoffs folder `{handoffs}`, the session-reviews folder
  `{reviews}`, and skills (read only).
- Edit: only `{review}` and your report `{report}`.
- Never edit skills, lessons, memories, rules files, cards or the run
  folder; the user picks what to apply.
- Never read or repeat a card or session the hub's handoffs mark as
  private.
- Report only to the hub `{new}`: write your report (replace its old
  text), then end your turn. Put questions there too. Never prompt the
  hub. Never use an interactive question tool, since Herdr then
  rejects every message to you.
- When the hub says the work is done, close your own tab with
  `herdr tab close <tab>`, the tab being `.result.agent.tab_id` of
  `herdr agent get "$HERDR_PANE_ID"`.

## Rules

No emojis; never the words "canonical" or "contract". Never report
dollars. Never print secret values.
"""

PREVIOUS_REVIEW = """
The newest earlier review is `{path}`. Read it so you do not repeat
its proposals, and say when `{old}` repeated a mistake it named.
"""

OLD_HUB_STEPS = """
## The old hub

`{old}` handed off to you. Once `hub status` shows it idle or done,
close its tab with `herdr tab close {tab}`. Then start its session
review with `herdr-handoff`: agent `{reviewer}`, handoff file
`{review}`. Bring the review's pick list to the user.
"""


def run_handoff(args: argparse.Namespace) -> int:
    """Start the next hub on the rules' model and check its effort.

    Checks every input before it starts anything. It writes the
    session review handoff for this hub, starts the next hub's tab,
    then adds to the next hub's handoff how to close this hub's tab
    and start that review. Only then does the next hub get its
    prompt, so it reads the whole handoff.

    Args:
        args: The parsed ``handoff`` command line.

    Returns:
        0 when the next hub's first reply ran at the expected effort,
        or 2 when it did not, gave no reply in time, Herdr failed after
        the start, or an input was missing, which stops the command
        before it starts anything.
    """
    rules = args.run / "rules.md"
    model = read_rules_model(rules)
    pane = os.environ.get("HERDR_PANE_ID")
    if model is None:
        return refuse(f"{rules} has no --model '...' line")
    if not args.handoff.is_file():
        return refuse(f"{args.handoff} does not exist")
    if not pane:
        return refuse("run hub handoff in the hub's own pane")
    old = get_agent(None, pane)
    if not (old.get("agent_session") or {}).get("value"):
        return refuse("Herdr does not know this hub's OpenCode session")

    state = Path.home() / ".local" / "state" / "pde"
    review = write_review_handoff(state, args.run.resolve(), old, args.name)
    start_hub(old["workspace_id"], args.name, model)
    with args.handoff.open("a") as handoff:
        handoff.write(
            OLD_HUB_STEPS.format(
                old=old["name"],
                tab=old["tab_id"],
                reviewer=name_reviewer(old["name"]),
                review=review,
            )
        )
    # The next hub already runs here, so these messages say so and a
    # rerun, which would start a second hub, is not the fix.
    try:
        prompt_agent(
            None,
            args.name,
            f"Read the handoff at {args.handoff.resolve()} and follow it."
            " Load each skill it names when it says.",
        )
        effort = wait_for_reply_effort(args.name)
    except HerdrError as error:
        return refuse(f"started {args.name}, but {error}")
    expected = model.partition("#")[2] or "default"
    if effort is None:
        return refuse(
            f"started {args.name}, but it gave no reply in"
            f" {REPLY_TIMEOUT_SECONDS} seconds, so its effort is unchecked"
        )
    if effort != expected:
        return refuse(
            f"started {args.name}, but it runs at {effort} effort,"
            f" not {expected}"
        )
    print(f"started {args.name} on {model}; its first reply ran at {effort}")
    print(f"review handoff {review}")
    return 0


def refuse(message: str) -> int:
    """Print the reason the command stopped and return its exit code.

    Args:
        message: The reason, without the ``hub:`` prefix.

    Returns:
        2.
    """
    print(f"hub: {message}", file=sys.stderr)
    return 2


def read_rules_model(rules: Path) -> str | None:
    """Read the model of the last ``--model '...'`` in rules.md.

    Args:
        rules: The run folder's ``rules.md``.

    Returns:
        The model, such as ``claude-code/claude-opus-5-5[1m]#medium``,
        or None when the file or the line is missing.
    """
    if not rules.exists():
        return None
    models = RULES_MODEL.findall(rules.read_text())
    return models[-1] if models else None


def write_review_handoff(state: Path, run: Path, old: dict, new: str) -> Path:
    """Write the handoff for the session review of the old hub.

    Args:
        state: PDE's state folder, holding ``handoffs`` and
            ``session-reviews``.
        run: The hub's run folder.
        old: The old hub's agent as Herdr prints it.
        new: The next hub's agent name.

    Returns:
        The handoff file, replaced when it already exists.
    """
    today = datetime.date.today()
    handoffs = state / "handoffs"
    reviews = state / "session-reviews"
    earlier = sorted(
        reviews.glob("*.md"), key=lambda path: path.stat().st_mtime
    )
    previous = ""
    if earlier:
        previous = PREVIOUS_REVIEW.format(path=earlier[-1], old=old["name"])
    reviewer = name_reviewer(old["name"])
    text = REVIEW_HANDOFF.format(
        old=old["name"],
        reviewer=reviewer,
        new=new,
        workspace=old["workspace_id"],
        date=today.isoformat(),
        session=old["agent_session"]["value"],
        run=run,
        previous=previous,
        review=reviews / f"{today:%Y%m%d}-{old['name']}.md",
        handoffs=handoffs,
        reviews=reviews,
        report=run / f"report-{reviewer}.md",
    )
    handoffs.mkdir(parents=True, exist_ok=True)
    path = handoffs / f"{today:%Y%m%d}-{old['name']}-review.md"
    path.write_text(text)
    return path


def name_reviewer(old: str) -> str:
    """Name the session that reviews the old hub.

    Args:
        old: The old hub's agent name.

    Returns:
        ``review-<old>``, cut to the 32 characters Herdr allows.
    """
    return f"review-{old}"[:32]


def start_hub(workspace: str, name: str, model: str) -> None:
    """Start an OpenCode session in a new tab of the hub's workspace.

    Args:
        workspace: The hub's Herdr workspace.
        name: The tab label and agent name.
        model: The model, with ``#<effort>`` when it sets one.
    """
    tab = call_herdr(
        None,
        "tab",
        "create",
        "--no-focus",
        "--workspace",
        workspace,
        "--label",
        name,
        "--cwd",
        os.getcwd(),
    )
    pane = json.loads(tab)["result"]["root_pane"]["pane_id"]
    call_herdr(
        None,
        "agent",
        "start",
        name,
        "--kind",
        "opencode",
        "--pane",
        pane,
        "--timeout",
        START_TIMEOUT_MS,
        "--",
        "--model",
        model,
        "--auto",
    )


def wait_for_reply_effort(name: str) -> str | None:
    """Wait for the session's first reply and read its effort.

    Herdr learns the session's OpenCode ID only after it starts, so
    both are polled until ``REPLY_TIMEOUT_SECONDS`` runs out.

    Args:
        name: The agent's name.

    Returns:
        The effort OpenCode logged, such as ``medium`` or ``default``,
        or None when no reply came in time.
    """
    poll_seconds = float(os.environ.get("HUB_POLL_SECONDS", "5"))
    deadline = time.monotonic() + REPLY_TIMEOUT_SECONDS
    while True:
        session = (get_agent(None, name).get("agent_session") or {}).get(
            "value"
        )
        if session:
            effort = read_first_reply_effort(session)
            if effort is not None:
                return effort
        if time.monotonic() >= deadline:
            return None
        time.sleep(poll_seconds)


def read_first_reply_effort(session: str) -> str | None:
    """Read the effort of a session's first reply from OpenCode's database.

    Args:
        session: The OpenCode session ID.

    Returns:
        The reply's ``model.variant``, or None before the first reply.
    """
    database = Path.home() / ".local" / "share" / "opencode" / "opencode.db"
    if not database.exists():
        return None
    with closing(
        sqlite3.connect(f"{database.as_uri()}?mode=ro", uri=True)
    ) as connection:
        row = connection.execute(
            "select data from session_message"
            " where session_id = ? and type = 'assistant'"
            " order by seq limit 1",
            (session,),
        ).fetchone()
    if row is None:
        return None
    return json.loads(row[0]).get("model", {}).get("variant", "default")
