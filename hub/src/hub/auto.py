"""Run ``hub auto``, the commands of an auto run.

An auto run works a list of cards the user approved, without the
user, until each card stops at a pull request or a question. Its
commands run in this order, each in the auto run's folder:

- ``ready`` writes ``ready.md``, the text of every ready card, for
  the model that sorts them.
- ``check`` checks ``run.md``, the list the user is asked to approve.
- ``approve`` saves that list and the user's reply in
  ``approved.md``, only when the reply is ``approve run N``, or with
  ``--given``, when the user's own words name exactly the run's cards.
- ``now`` writes ``brief-now.md``, one line per card of the run.

``run.md`` names every ready card once. Only the cards under
``## Run`` are worked; the others sit under headings that say why::

    # Auto run 2

    ## Run
    - DEMO-KB-007 demo: retry failed uploads

    ## Needs your hands
    - DEMO-KB-009 demo: plug in the new disk
"""

from __future__ import annotations

import argparse
import re
import sys
import time
from pathlib import Path

from hub.brief import read_decisions
from hub.status import CARD_LINE, call_card

RUN_TITLE = re.compile(r"# Auto run (\d+)$")
LISTED_CARD = re.compile(r"- ([A-Z][A-Z0-9]*-KB-\d{3,})\b")
NAMED_CARD = re.compile(r"\b[A-Z][A-Z0-9]*-KB-\d{3,}\b", re.IGNORECASE)
# The card sections that tell the sorting model what the work needs.
CARD_SECTIONS = ("Context", "Acceptance Criteria")


def run_ready(args: argparse.Namespace) -> int:
    """Write every ready card's title, project and text to ready.md.

    Args:
        args: The parsed ``auto ready`` command line.

    Returns:
        0 after printing what it wrote.
    """
    cards = [describe_card(key) for key in list_ready_cards()]
    text = "\n\n".join(cards) + "\n" if cards else "No ready cards.\n"
    (args.run / "ready.md").write_text(text)
    print(text, end="")
    return 0


def run_check(args: argparse.Namespace) -> int:
    """Check run.md against the ready cards.

    Args:
        args: The parsed ``auto check`` command line.

    Returns:
        0 when the list can go to the user, or 2 after naming each
        problem.
    """
    number, sections, problems = check_run_list(args.run / "run.md")
    if problems:
        print(
            "\n".join(f"hub: {problem}" for problem in problems),
            file=sys.stderr,
        )
        return 2
    print(f"run {number}: {len(sections['Run'])} cards to run")
    return 0


def run_approve(args: argparse.Namespace) -> int:
    """Save the checked list with the user's reply, once.

    A reply other than ``approve run N``, such as "go" or an edit,
    starts nothing, since run 1 started on a "go" meant for another
    question. With ``--given``, the reply is the user's own words,
    given before the briefing, and they pass only when they name by key
    every card under ``## Run`` and no other card.

    Args:
        args: The parsed ``auto approve`` command line.

    Returns:
        0 after writing approved.md, or 2 when the run is already
        approved, the reply is not the phrase or does not name the
        run's cards, or the list fails its check.
    """
    approved = args.run / "approved.md"
    phrase = f"approve run {args.number}"
    if approved.exists():
        return refuse(f"run {args.number} is already approved")
    if not args.given:
        said = " ".join(args.reply.lower().split()).strip("\"'.! ")
        if said != phrase:
            return refuse(f"the reply is not {phrase!r}; nothing was approved")
    number, sections, problems = check_run_list(args.run / "run.md")
    if problems:
        return refuse("run.md fails its check; run hub auto check")
    if number != args.number:
        return refuse(f"run.md is run {number}, not run {args.number}")
    if args.given:
        named = {key.upper() for key in NAMED_CARD.findall(args.reply)}
        if named != set(sections["Run"]):
            return refuse(
                "the words do not name every card under '## Run' and no"
                " other; nothing was approved"
            )
    approved.write_text(
        f"# Auto run {number} approved\n\n"
        f"Approved: {time.strftime('%Y-%m-%d %H:%M')}\n"
        f"The user's words: {args.reply}\n\n"
        + (args.run / "run.md").read_text()
    )
    print(f"approved run {number}: {len(sections['Run'])} cards")
    return 0


def run_now(args: argparse.Namespace) -> int:
    """Write brief-now.md, one line per card of the run, and print it.

    A card's line is the first line of its worker's status file, and
    says "needs you" while decisions.md holds a question for that card.
    The title of each question follows the count.

    Args:
        args: The parsed ``auto now`` command line.

    Returns:
        0 after printing, or 2 when the run has no approved.md.
    """
    approved = args.run / "approved.md"
    if not approved.exists():
        return refuse("the run is not approved; start nothing")
    number, sections = read_run_list(approved.read_text())
    questions = read_decisions(args.run / "decisions.md")
    asking = {title.split()[0] for title, _ in questions if title}
    lines = [f"Auto run {number}"]
    for key in sections.get("Run", []):
        status = args.run / f"status-{key.lower()}-auto.md"
        news = read_first_line(status) or "not started"
        mark = " (needs you)" if key in asking else ""
        lines.append(f"{key}{mark}: {news}")
    lines.append(f"Questions for you: {len(questions)}")
    lines += [f"- {title}" for title, _ in questions]
    text = "\n".join(lines) + "\n"
    (args.run / "brief-now.md").write_text(text)
    print(text, end="")
    return 0


def list_ready_cards() -> list[str]:
    """List the keys of the cards in ready, as the card command shows them.

    Returns:
        The keys, in the order ``card list`` prints them.
    """
    lines = call_card("list", "--stage", "ready").splitlines()
    return [line.split()[0] for line in lines if CARD_LINE.match(line)]


def describe_card(key: str) -> str:
    """Describe one card for the sorting model.

    Args:
        key: The card's key, such as ``DEMO-KB-007``.

    Returns:
        A ``## <key> <title>`` heading, a ``Project:`` line, then the
        bullets of the card's Context and Acceptance Criteria.
    """
    note = call_card("show", key).splitlines()
    title = next(
        (line[2:] for line in note if line.startswith(f"# {key}")), key
    )
    project = next(
        (
            line.split(":", 1)[1].strip()
            for line in note
            if line.startswith("project:")
        ),
        "-",
    )
    lines = [f"## {title}", f"Project: {project}"]
    section = ""
    for line in note:
        if line.startswith("## "):
            section = line[3:].strip()
        elif section in CARD_SECTIONS and line.startswith("- "):
            lines.append(line)
    return "\n".join(lines)


def check_run_list(
    path: Path,
) -> tuple[int | None, dict[str, list[str]], list[str]]:
    """Read run.md and find what keeps it from going to the user.

    Args:
        path: The run folder's ``run.md``.

    Returns:
        The run's number, its card keys by heading, and one line per
        problem. A card must be ready, listed once, and every ready
        card must be listed, so the briefing accounts for each one.
    """
    if not path.exists():
        return None, {}, ["run.md does not exist"]
    number, sections = read_run_list(path.read_text())
    ready = list_ready_cards()
    listed = [key for keys in sections.values() for key in keys]
    problems = []
    if number is None:
        problems.append("run.md lacks its '# Auto run N' title")
    if not sections.get("Run"):
        problems.append("run.md lists no card under '## Run'")
    for key in sorted(set(listed)):
        if listed.count(key) > 1:
            problems.append(f"{key} is listed more than once")
        if key not in ready:
            problems.append(
                f"{key} is not a ready card; only the user moves a card"
                " to ready"
            )
    for key in ready:
        if key not in listed:
            problems.append(f"ready card {key} is missing from run.md")
    return number, sections, problems


def read_run_list(text: str) -> tuple[int | None, dict[str, list[str]]]:
    """Read the run's number and the card keys under each heading.

    Args:
        text: The text of ``run.md`` or ``approved.md``.

    Returns:
        The number from the ``# Auto run N`` line, or None, and the
        keys listed under each ``## <heading>``, in file order.
    """
    number = None
    sections: dict[str, list[str]] = {}
    heading = ""
    for line in text.splitlines():
        if title := RUN_TITLE.match(line):
            number = int(title.group(1))
        elif line.startswith("## "):
            heading = line[3:].strip()
            sections.setdefault(heading, [])
        elif heading and (card := LISTED_CARD.match(line)):
            sections[heading].append(card.group(1))
    return number, sections


def read_first_line(path: Path) -> str:
    """Return a file's first line that is not blank, or "" without one.

    Args:
        path: A worker's status file, which may not exist yet.

    Returns:
        The line without its surrounding spaces.
    """
    if not path.exists():
        return ""
    return next(
        (
            line.strip()
            for line in path.read_text().splitlines()
            if line.strip()
        ),
        "",
    )


def refuse(message: str) -> int:
    """Print why the command refused, and return its exit code.

    Args:
        message: The reason, without the ``hub:`` prefix.

    Returns:
        2, the exit code of a refused command.
    """
    print(f"hub: {message}", file=sys.stderr)
    return 2
