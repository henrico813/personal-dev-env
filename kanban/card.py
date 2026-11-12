#!/usr/bin/env python3
"""Keep kanban cards as notes in the Obsidian vault.

A card is one piece of work the user wants, like a GitHub issue. A plan
made from it is like a pull request. Each card is one note,
``500 Zettelkasten/Projects/<Project>/<KEY> <Title>.md``, with a key
such as PDEV-KB-012 built from the project code in the project
folder's AGENTS.md.

The tables FIELDS, SECTIONS, STAGES, KINDS and MOVES below define the
note layout and the rules. ``card template`` prints the Obsidian
template from the same tables, so cards made in either place match.

Usage::

    card [--vault NAME] COMMAND ...

    card new PROJECT TITLE [--kind K] [--user]  make a card in draft
    card new PROJECT TITLE --adopt STAGE --user [--why TEXT]
                                       [--session [AGENT]]
    card list [--stage S] [--project P] [--kind K]
    card board                         counts and the review queue
    card show KEY [--section S]
    card grep PATTERN [--section S] [--stage S] [--project P]
    card move KEY STAGE [--user] [--why TEXT] [--session [AGENT]]
    card note KEY TEXT [--needs-you | --answered]
    card add KEY SECTION TEXT          add a bullet or criteria checkbox
    card attach KEY FILE               copy FILE to attachments, embed it
    card edit KEY SECTION N TEXT|--remove  replace or remove item N
    card set KEY title|kind VALUE
    card link KEY PLAN|PR_URL
    card link KEY --session [AGENT]    record a Herdr agent's session
    card delete KEY --user
    card check                         cards with a broken layout
    card template                      print the Obsidian template

Agents pass --user only when the user told them to make that change.

The vault is on a network drive with sync, and a write cut short
there is copied everywhere. So card writes a note only when its text
changes. It writes ``.<KEY>.tmp`` in the same folder, then renames it
over the note. Just before the rename it checks that nobody saved the
note since it was read.
"""

import argparse
import fcntl
import json
import os
import re
import shutil
import subprocess
import sys
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import date
from pathlib import Path

STATE_DIR = Path.home() / ".local/state/card"
PROJECTS_SUBDIR = "500 Zettelkasten/Projects"

STAGES = ["draft", "ready", "active", "review", "accepted", "dropped"]
KINDS = {
    "build": "ends in a change to code, config or the running setup",
    "research": "ends in a document or a decision",
}
# A move missing here is refused, so accepted is final and follow-up
# work gets a new card. "user" moves need --user, and "staff" moves
# take it when the user makes them.
MOVES = {
    ("draft", "ready"): "user",
    ("ready", "draft"): "staff",
    ("ready", "active"): "staff",
    ("active", "ready"): "staff",
    ("active", "draft"): "user",
    ("active", "review"): "staff",
    ("review", "active"): "user",
    ("review", "accepted"): "user",
    ("dropped", "draft"): "user",
    **{
        (stage, "dropped"): "user"
        for stage in STAGES
        if stage not in ("accepted", "dropped")
    },
}
# Only the user brings work already under way onto the board, straight
# into one of these stages. These are not moves, so MOVES leaves them out.
ADOPT_STAGES = ["active", "review"]
# Cards in active plus review across all projects. The user may start
# a card past it with --user.
WORK_LIMIT = 8

FIELDS = [
    "tags",
    "type",
    "stage",
    "stage_date",
    "template_version",
    "project",
    "kind",
    "date_created",
    "plans",
    "prs",
    "sessions",
    "topics",
]
SECTIONS = [
    "Context",
    "Acceptance Criteria",
    "Open Questions",
    "Decisions",
    "Log",
]
CRITERIA = "Acceptance Criteria"

WIDTH = 60
BOARD_WIDTH = 40

CARD_NAME_RE = re.compile(r"^([A-Z][A-Z0-9]*-KB-\d{3,})\b.*\.md$")
PROJECT_CODE_RE = re.compile(r"Project code:\s*`?([A-Z][A-Z0-9]*)`?")
FIELD_RE = re.compile(r"^([A-Za-z_][\w-]*):(.*)$")
LIST_ITEM_RE = re.compile(r"^\s+-\s+(.*)$")
HEADING_RE = re.compile(r"^(#{1,2})\s+(.*?)\s*$")
CHECKBOX_RE = re.compile(r"^\s*- \[[ xX]\] \S", re.M)
# A section item's bullet or checkbox, then its words.
ITEM_PREFIX_RE = re.compile(r"^(- \[[ xX]\] |[-*] )?(.*)$")
# Characters Obsidian refuses in file names, plus link syntax.
BAD_NAME_CHARS_RE = re.compile(r'[\\/:*?"<>|#^\[\]]')


class CardError(Exception):
    """A card command that cannot go ahead, with a message for the user."""


@dataclass
class Card:
    """One card note as last read from disk."""

    path: Path
    key: str
    text: str
    mtime_ns: int

    @property
    def project(self) -> str:
        """The project folder name, such as DevEnv."""
        return self.path.parent.name

    @property
    def stage(self) -> str:
        """The stage field, or an empty string when it is missing."""
        return str(read_fields(self.text).get("stage", ""))

    @property
    def kind(self) -> str:
        """The kind field, or an empty string when it is missing."""
        return str(read_fields(self.text).get("kind", ""))

    @property
    def stage_date(self) -> str:
        """The day of the last move, else the creation date."""
        fields = read_fields(self.text)
        return str(fields.get("stage_date") or fields.get("date_created", ""))

    @property
    def title(self) -> str:
        """The title from the first heading, else from the file name."""
        title = read_title(self.text, self.key)
        return title or self.path.stem.removeprefix(self.key).strip()


@dataclass(frozen=True)
class AgentSession:
    """A Herdr agent's name and its entry for the sessions field.

    The entry is the agent kind and session id, such as
    ``opencode ses_e4dc27c20ffeJkm3JZ8dwLh7rW``.
    """

    name: str
    entry: str


def main(argv: list[str], projects: Path | None = None) -> int:
    """Run one card command.

    Args:
        argv: The arguments after the program name.
        projects: The vault's Projects folder. When None, card asks
            ``nobs vault path`` for the vault.

    Returns:
        The exit code.
    """
    args = build_parser().parse_args(argv)
    try:
        if args.command == "template":
            print(render_template(), end="")
            return 0
        if projects is None:
            projects = find_projects_folder(args.vault)
        return args.run(args, projects)
    except CardError as error:
        print(f"card: {error}", file=sys.stderr)
        return 1


def build_parser() -> argparse.ArgumentParser:
    """Build the command line parser.

    Returns:
        The parser, where each command sets ``run`` to its function.
    """
    parser = argparse.ArgumentParser(
        prog="card",
        description="Keep kanban cards as notes in the Obsidian vault.",
        epilog="Agents pass --user only when the user told them to.",
    )
    parser.add_argument("--vault", help="vault name for nobs vault path")
    commands = parser.add_subparsers(dest="command", required=True)

    new = commands.add_parser("new", help="make a card in draft")
    new.add_argument("project", help="project folder, such as DevEnv")
    new.add_argument("title")
    new.add_argument(
        "--kind",
        choices=list(KINDS),
        default="build",
        help="; ".join(f"{name} {meaning}" for name, meaning in KINDS.items()),
    )
    new.add_argument(
        "--user",
        action="store_true",
        help="the user asked for this card; agents pass it only when told",
    )
    new.add_argument(
        "--adopt",
        choices=ADOPT_STAGES,
        help="start here, for work already under way; needs --user",
    )
    new.add_argument("--why", help="a reason for the Log line")
    add_session_option(new, "with --adopt, ")
    add_author(new)
    new.set_defaults(run=run_new)

    listing = commands.add_parser("list", help="one line per card")
    add_filters(listing)
    listing.set_defaults(run=run_list)

    board = commands.add_parser("board", help="counts and review queue")
    board.set_defaults(run=run_board)

    show = commands.add_parser("show", help="print a card")
    show.add_argument("key")
    show.add_argument("--section", choices=SECTIONS)
    show.set_defaults(run=run_show)

    grep = commands.add_parser("grep", help="search card text")
    grep.add_argument("pattern", help="regular expression, any case")
    grep.add_argument("--section", choices=SECTIONS)
    add_filters(grep)
    grep.set_defaults(run=run_grep)

    move = commands.add_parser(
        "move",
        help="move a card to another stage",
        epilog=format_moves(),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    move.add_argument("key")
    move.add_argument("stage", choices=STAGES)
    move.add_argument(
        "--user",
        action="store_true",
        help="the user made this move; agents pass it only when told",
    )
    move.add_argument("--why", help="a reason for the Log line")
    add_session_option(move, "with a move to active, ")
    add_author(move)
    move.set_defaults(run=run_move)

    note = commands.add_parser("note", help="add a note to the Log")
    note.add_argument("key")
    note.add_argument("text")
    signal = note.add_mutually_exclusive_group()
    signal.add_argument(
        "--needs-you",
        action="store_true",
        help="a question, approval or failure waiting on the user",
    )
    signal.add_argument(
        "--answered", action="store_true", help="the user's answer"
    )
    add_author(note)
    note.set_defaults(run=run_note)

    add = commands.add_parser(
        "add", help="add a bullet, or a criteria checkbox, to a section"
    )
    add.add_argument("key")
    add.add_argument(
        "section", choices=[name for name in SECTIONS if name != "Log"]
    )
    add.add_argument("text")
    add.set_defaults(run=run_add)

    attach = commands.add_parser(
        "attach", help="copy a file to attachments and embed it in Context"
    )
    attach.add_argument("key")
    attach.add_argument("file")
    attach.set_defaults(run=run_attach)

    edit = commands.add_parser(
        "edit",
        help="replace or remove one item of a section",
        description="N is the item's number in card show KEY --section S.",
    )
    edit.add_argument("key")
    edit.add_argument(
        "section", choices=[name for name in SECTIONS if name != "Log"]
    )
    edit.add_argument("number", type=int, metavar="N")
    edit.add_argument("text", nargs="?", help="the item's new text")
    edit.add_argument("--remove", action="store_true", help="remove item N")
    add_author(edit)
    edit.set_defaults(run=run_edit)

    set_ = commands.add_parser("set", help="change the title or kind")
    set_.add_argument("key")
    set_.add_argument("field", choices=["title", "kind"])
    set_.add_argument("value")
    set_.set_defaults(run=run_set)

    link = commands.add_parser(
        "link", help="link a plan note, a PR URL or a Herdr session"
    )
    link.add_argument("key")
    link.add_argument("target", nargs="?", help="plan note name, or a PR URL")
    add_session_option(link, "")
    link.set_defaults(run=run_link)

    delete = commands.add_parser("delete", help="remove a card note")
    delete.add_argument("key")
    delete.add_argument("--user", action="store_true", required=True)
    delete.set_defaults(run=run_delete)

    check = commands.add_parser("check", help="find broken cards")
    check.set_defaults(run=run_check)

    commands.add_parser("template", help="print the Obsidian template")
    return parser


def add_author(parser: argparse.ArgumentParser) -> None:
    """Add the --by option for commands that write a Log line.

    Args:
        parser: The command's parser.
    """
    parser.add_argument(
        "--by",
        help="who acts, for the Log; needed outside a named Herdr agent",
    )


def add_session_option(parser: argparse.ArgumentParser, when: str) -> None:
    """Add the --session option, which records a Herdr agent's session.

    Args:
        parser: The command's parser.
        when: When the option applies, such as ``"with --adopt, "``.
    """
    parser.add_argument(
        "--session",
        nargs="?",
        const="",
        metavar="AGENT",
        help=f"{when}add the agent's session to sessions;"
        " no AGENT means this pane's agent",
    )


def add_filters(parser: argparse.ArgumentParser) -> None:
    """Add the --stage, --project and --kind filters.

    Args:
        parser: The command's parser.
    """
    parser.add_argument("--stage", choices=STAGES)
    parser.add_argument("--project", help="project folder, any case")
    parser.add_argument("--kind", choices=list(KINDS))


def format_moves() -> str:
    """Format MOVES as help text, one move per line.

    Returns:
        The help text.
    """
    lines = [
        "moves (user moves need --user; the user also makes staff moves,"
        " with --user):"
    ]
    for (source, target), who in MOVES.items():
        lines.append(f"  {source} -> {target}: {who}")
    lines.append(
        f"the user starts a card past the work limit of {WORK_LIMIT}"
        " with ready -> active --user"
    )
    lines.append("work already under way: card new --adopt STAGE --user")
    return "\n".join(lines)


def run_new(args: argparse.Namespace, projects: Path) -> int:
    """Make a card in draft, or adopt work under way, and print its name.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    if args.adopt and not args.user:
        raise CardError(
            "only the user adopts work already under way."
            " Pass --user only when the user said so."
        )
    if args.session is not None and not args.adopt:
        raise CardError(
            "--session goes with --adopt. A draft card gets its session"
            " when it moves to active."
        )
    stage = args.adopt or STAGES[0]
    verb = "adopted by" if args.adopt else "by"
    entry = format_log_entry(
        stage, f"{verb} {find_author(args)}", args.user, args.why
    )
    session = None
    if args.session is not None:
        session = find_agent_session(args.session)
    path = create_card(
        projects, args.project, args.title, args.kind, stage, entry, session
    )
    print(path.stem)
    return 0


def create_card(
    projects: Path,
    project: str,
    title: str,
    kind: str,
    stage: str,
    log_entry: str,
    session: AgentSession | None = None,
) -> Path:
    """Take the next key in a project and write a new card.

    A local lock stops two card commands on this computer from taking
    one number. A card made in Obsidian at the same moment can still
    take it, so the folder is checked again after the write.

    Args:
        projects: The vault's Projects folder.
        project: The project folder name, in any case.
        title: The card title.
        kind: One of KINDS.
        stage: The first stage, draft unless the card is adopted.
        log_entry: The first Log line, without its date.
        session: A Herdr agent session to record, or None.

    Returns:
        The new card's path.

    Raises:
        CardError: The project has no code, or two files hold the key.
    """
    folder = find_project_folder(projects, project)
    code = read_project_code(folder)
    name = BAD_NAME_CHARS_RE.sub("", title).strip()
    if not name:
        raise CardError("the title needs a letter or digit")
    today = date.today().isoformat()
    with hold_lock(STATE_DIR / "new.lock"):
        key = f"{code}-KB-{find_next_number(folder, code):03d}"
        path = folder / f"{key} {name}.md"
        text = render_card(
            key=key,
            title=title.strip(),
            project=folder.name,
            kind=kind,
            today=today,
            stage=stage,
            log_entry=log_entry,
        )
        if session:
            text = add_session(text, today, session)
        write_safely(path, key, text)
    holders = list_key_holders(folder, key)
    if len(holders) > 1:
        names = ", ".join(holder.name for holder in holders)
        raise CardError(f"two files hold {key}, fix by hand: {names}")
    return path


def find_next_number(folder: Path, code: str) -> int:
    """Return the highest card number in the folder plus one.

    Args:
        folder: A project folder.
        code: The project code, such as PDEV.

    Returns:
        The number for the next card.
    """
    number_re = re.compile(rf"^{re.escape(code)}-KB-(\d+)")
    numbers = [
        int(match[1])
        for path in folder.iterdir()
        if (match := number_re.match(path.name))
    ]
    return max(numbers, default=0) + 1


def run_list(args: argparse.Namespace, projects: Path) -> int:
    """Print one line per card, in stage order.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    cards = filter_cards(load_all_cards(projects), args)
    cards.sort(key=lambda card: (rank_stage(card.stage), card.key))
    for card in cards:
        print(fit(f"{card.key} {card.stage:<8} {card.kind:<8} {card.title}"))
    print(f"{len(cards)} cards")
    return 0


def run_board(args: argparse.Namespace, projects: Path) -> int:
    """Print counts per stage, then the review queue, oldest first.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    cards = load_all_cards(projects)
    counts = {stage: 0 for stage in STAGES}
    for card in cards:
        counts[card.stage] = counts.get(card.stage, 0) + 1
    print("  ".join(f"{stage} {counts[stage]}" for stage in STAGES[:3]))
    print("  ".join(f"{stage} {counts[stage]}" for stage in STAGES[3:]))
    busy = counts["active"] + counts["review"]
    print(f"active+review {busy} of {WORK_LIMIT}")
    review = [card for card in cards if card.stage == "review"]
    review.sort(key=lambda card: (card.stage_date, card.key))
    print(f"\nREVIEW, oldest first ({len(review)})")
    for card in review:
        print(fit(f"- {card.key} {card.stage_date}", BOARD_WIDTH))
        print(fit(f"  {card.title}", BOARD_WIDTH))
    return 0


def run_show(args: argparse.Namespace, projects: Path) -> int:
    """Print a whole card, or one section of it.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    if not args.section:
        print(card.text, end="")
        return 0
    text = read_section(card.text, args.section)
    if text is None:
        raise CardError(f"{card.key} has no {args.section} section")
    lines = card.text.split("\n")
    numbers = {
        start: number
        for number, (start, _) in enumerate(list_items(lines, args.section), 1)
    }
    bounds = find_section(lines, args.section)
    assert bounds is not None, "read_section found the section"
    shown = [
        f"{numbers[index]}. {lines[index]}"
        if index in numbers
        else lines[index]
        for index in range(*bounds)
    ]
    print(f"## {args.section}\n" + "\n".join(shown).strip())
    return 0


def run_grep(args: argparse.Namespace, projects: Path) -> int:
    """Print each matching line as ``KEY Section: line``.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code, 1 when nothing matched.
    """
    try:
        pattern = re.compile(args.pattern, re.I)
    except re.error as error:
        raise CardError(f"bad pattern: {error}") from error
    cards = filter_cards(load_all_cards(projects), args)
    matches = 0
    matched_cards = 0
    for card in sorted(cards, key=lambda card: card.key):
        found = [
            (section, line)
            for section, line in label_lines(card.text)
            if pattern.search(line)
            and (not args.section or section == args.section)
        ]
        for section, line in found:
            print(fit(f"{card.key} {section}: {line.strip()}"))
        matches += len(found)
        matched_cards += bool(found)
    print(f"{matches} matches in {matched_cards} cards")
    return 0 if matches else 1


def label_lines(text: str) -> Iterator[tuple[str, str]]:
    """Yield each text line of a card with the section it sits in.

    Frontmatter lines get "fields" and the first heading gets "Title".
    Headings and ``---`` lines are left out.

    Args:
        text: The card text.

    Yields:
        Pairs of section name and line.
    """
    lines = text.split("\n")
    end = find_frontmatter_end(lines)
    for line in lines[1:end]:
        yield "fields", line
    section = "Title"
    for line in lines[end + 1 if end else 0 :]:
        heading = HEADING_RE.match(line)
        if heading and heading[1] == "##":
            section = heading[2]
        elif heading:
            yield "Title", heading[2]
        elif line.strip() and line.strip() != "---":
            yield section, line


def run_move(args: argparse.Namespace, projects: Path) -> int:
    """Move a card to another stage and add a Log line.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    if args.session is not None and args.stage != "active":
        raise CardError("--session goes with a move to active")
    card = find_card(projects, args.key)
    today = date.today().isoformat()
    entry = format_log_entry(
        args.stage, f"by {find_author(args)}", args.user, args.why
    )
    session = None
    if args.session is not None:
        session = find_agent_session(args.session)

    def change(text: str) -> str:
        """Check the move against the card as it is now, then make it."""
        current = str(read_fields(text).get("stage", ""))
        check_move(text, current, args.stage, args.user, projects)
        text = set_field(text, "stage", args.stage)
        text = set_field(text, "stage_date", today)
        text = add_to_section(text, "Log", f"- {today} {entry}")
        return add_session(text, today, session) if session else text

    update_card(card, change)
    print(f"{card.key} {args.stage}")
    return 0


def format_log_entry(
    stage: str, author: str, user: bool, why: str | None
) -> str:
    """Return a stage Log line without its date.

    Args:
        stage: The stage the card enters.
        author: Who acts, such as ``by general-staff``.
        user: True when the user made the decision.
        why: A reason, or None.

    Returns:
        A line such as ``review: by general-staff for the user. PR open``.
    """
    entry = f"{stage}: {author}"
    # The Log is the only record that the user, not the agent, decided.
    if user:
        entry += " for the user"
    if why:
        entry += f". {why}"
    return entry


def check_move(
    text: str, current: str, target: str, user: bool, projects: Path
) -> None:
    """Refuse a move the rules do not allow.

    Args:
        text: The card text.
        current: The card's stage now.
        target: The stage to move to.
        user: True when the user made the move.
        projects: The vault's Projects folder, for the work limit.

    Raises:
        CardError: The move is not allowed, with the reason.
    """
    who = MOVES.get((current, target))
    if who is None:
        options = [to for (source, to) in MOVES if source == current]
        allowed = ", ".join(options) or "none, the stage is unknown"
        raise CardError(f"no move {current} -> {target}. Allowed: {allowed}")
    if who == "user" and not user:
        raise CardError(
            f"only the user moves {current} -> {target}."
            " Pass --user only when the user said so."
        )
    # Only the user's release needs criteria. Paused work may be adopted
    # work that has none yet.
    if (current, target) == ("draft", "ready") and not CHECKBOX_RE.search(
        read_section(text, CRITERIA) or ""
    ):
        raise CardError(
            f"add a '- [ ] ...' item to {CRITERIA} before moving to ready"
        )
    if current == "ready" and target == "active" and not user:
        busy = sum(
            card.stage in ("active", "review")
            for card in load_all_cards(projects)
        )
        if busy >= WORK_LIMIT:
            raise CardError(
                f"{busy} cards are active or in review, the limit is"
                f" {WORK_LIMIT}. Ask the user which to finish first."
            )


def run_note(args: argparse.Namespace, projects: Path) -> int:
    """Add a dated note, question or answer to a card's Log.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    today = date.today().isoformat()
    if args.needs_you:
        entry = f"- {today} needs-you: {args.text}"
    elif args.answered:
        entry = f"- {today} answered: {args.text}"
    else:
        entry = f"- {today} note ({find_author(args)}): {args.text}"
    update_card(card, lambda text: add_to_section(text, "Log", entry))
    print(f"{card.key} Log: {entry}")
    return 0


def run_add(args: argparse.Namespace, projects: Path) -> int:
    """Add a bullet, or a checkbox in Acceptance Criteria, to a section.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    entry = (
        f"- [ ] {args.text}" if args.section == CRITERIA else f"- {args.text}"
    )
    update_card(card, lambda text: add_to_section(text, args.section, entry))
    print(f"{card.key} {args.section}: {entry}")
    return 0


def run_attach(args: argparse.Namespace, projects: Path) -> int:
    """Copy a file into the project's attachments folder and embed it.

    The copy is ``<project>/attachments/<KEY> <file name>``, and Context
    gets a ``![[...]]`` line for it. When the card cannot be saved,
    card removes the copy it just made, so trying again is not refused.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.

    Raises:
        CardError: FILE is missing or a folder, its name is empty once
            the characters Obsidian refuses are removed, the attachment
            already exists, or the card kept changing while being saved.
    """
    source = Path(args.file)
    if not source.is_file():
        raise CardError(f"no file at {args.file}")
    card = find_card(projects, args.key)
    name = BAD_NAME_CHARS_RE.sub("", source.name).strip()
    if not name:
        raise CardError(
            f"nothing is left of the name {source.name!r} once the"
            " characters Obsidian refuses are removed"
        )
    attachment = card.path.parent / "attachments" / f"{card.key} {name}"
    entry = f"- ![[{attachment.name}]]"
    copy_to_new_file(source, attachment)
    try:
        update_card(card, lambda text: add_to_section(text, "Context", entry))
    except BaseException:
        attachment.unlink()
        raise
    print(f"{card.key} Context: {entry}")
    return 0


def copy_to_new_file(source: Path, target: Path) -> None:
    """Copy a file to a path where no file exists yet.

    The exclusive create checks for the name and makes the file in one
    step, so two commands cannot both write it. A rename would replace
    a file already there. A copy cut short is removed.

    Args:
        source: The file to copy.
        target: The new file, whose folder is made when missing.

    Raises:
        CardError: A file already has the target's name.
    """
    with open(source, "rb") as reader:
        target.parent.mkdir(exist_ok=True)
        try:
            writer = open(target, "xb")
        except FileExistsError:
            raise CardError(
                f"{target.name} is already in {target.parent.name},"
                " rename the file and try again"
            ) from None
        try:
            with writer:
                shutil.copyfileobj(reader, writer)
                writer.flush()
                os.fsync(writer.fileno())
        except BaseException:
            target.unlink()
            raise


def run_edit(args: argparse.Namespace, projects: Path) -> int:
    """Replace or remove one numbered item of a section, and log it.

    A replaced item keeps its bullet or checkbox, with its tick.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    if (args.text is None) != args.remove:
        raise CardError("give either the new text or --remove")
    if args.text is not None and not split_item(args.text.strip())[1]:
        raise CardError("the new text is empty; use --remove")
    card = find_card(projects, args.key)
    today = date.today().isoformat()
    author = find_author(args)
    lines = card.text.split("\n")
    old = lines[find_item(lines, card.key, args.section, args.number)[0]]
    prefix, old_words = split_item(old)
    label = f"{args.section} item {args.number}"
    if args.remove:
        note = f"removed {label}, {json.dumps(old_words)}"
    else:
        note = f"replaced {label}, was {json.dumps(old_words)}"
    entry = f"- {today} note ({author}): {note}"

    def change(text: str) -> str:
        """Edit the item, unless someone changed it since card read it."""
        lines = text.split("\n")
        start, stop = find_item(lines, card.key, args.section, args.number)
        if lines[start] != old:
            raise CardError(f"{card.key} {label} changed; run card show again")
        if args.remove:
            del lines[start:stop]
            # Keep one blank line where the item was.
            if 0 < start < len(lines) and not (
                lines[start - 1].strip() or lines[start].strip()
            ):
                del lines[start]
        else:
            # The item keeps its own bullet or box, so a given one is
            # used only on a plain line.
            given_prefix, words = split_item(args.text.strip())
            lines[start] = (prefix or given_prefix) + words
        return add_to_section("\n".join(lines), "Log", entry)

    update_card(card, change)
    print(f"{card.key} Log: {entry}")
    return 0


def list_items(lines: list[str], section: str) -> list[tuple[int, int]]:
    """List where each item of a section starts and stops.

    An item is a line that is not blank or indented, with the indented
    lines right after it. ``card show --section`` numbers these from 1.

    Args:
        lines: The card text split into lines.
        section: The section name.

    Returns:
        Start and stop indexes per item, none when the section is
        missing.
    """
    bounds = find_section(lines, section)
    if bounds is None:
        return []
    items: list[tuple[int, int]] = []
    for index in range(*bounds):
        line = lines[index]
        if not line.strip():
            continue
        if not line[:1].isspace():
            items.append((index, index + 1))
        elif items and items[-1][1] == index:
            items[-1] = (items[-1][0], index + 1)
    return items


def split_item(line: str) -> tuple[str, str]:
    """Split an item line into its bullet or checkbox and its words.

    Args:
        line: The item's first line, such as ``- [ ] Tests pass.``.

    Returns:
        The prefix, empty for a plain line, and the words.
    """
    match = ITEM_PREFIX_RE.match(line)
    assert match, "the pattern matches any line"
    return match[1] or "", match[2]


def find_item(
    lines: list[str], key: str, section: str, number: int
) -> tuple[int, int]:
    """Return where item ``number`` of a section starts and stops.

    Args:
        lines: The card text split into lines.
        key: The card key, for the error.
        section: The section name.
        number: The item number, from 1.

    Returns:
        Start and stop indexes.

    Raises:
        CardError: The section has no such item.
    """
    items = list_items(lines, section)
    if not 1 <= number <= len(items):
        raise CardError(
            f"{key} {section} has {len(items)} items, so no item {number}"
        )
    return items[number - 1]


def run_set(args: argparse.Namespace, projects: Path) -> int:
    """Change a card's kind, or its title and file name.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    if args.field == "kind":
        if args.value not in KINDS:
            raise CardError(f"kind is one of {', '.join(KINDS)}")
        update_card(card, lambda text: set_field(text, "kind", args.value))
        print(f"{card.key} kind {args.value}")
        return 0
    name = BAD_NAME_CHARS_RE.sub("", args.value).strip()
    if not name:
        raise CardError("the title needs a letter or digit")
    new_path = card.path.with_name(f"{card.key} {name}.md")
    if new_path != card.path and new_path.exists():
        raise CardError(f"{new_path.name} already exists")
    update_card(
        card, lambda text: set_title(text, card.key, args.value.strip())
    )
    if new_path != card.path:
        os.rename(card.path, new_path)
    print(new_path.stem)
    return 0


def run_link(args: argparse.Namespace, projects: Path) -> int:
    """Add a plan link to ``plans``, a PR URL to ``prs``, or a session.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    if (args.target is None) == (args.session is None):
        raise CardError("give a plan, a PR URL or --session")
    card = find_card(projects, args.key)
    if args.session is not None:
        session = find_agent_session(args.session)
        today = date.today().isoformat()
        written = update_card(
            card, lambda text: add_session(text, today, session)
        )
        already = "" if written else " (already)"
        print(f"{card.key} sessions: {session.entry}{already}")
        return 0
    target = args.target.strip()
    if target.startswith(("http://", "https://")):
        field, value = "prs", target
    else:
        name = target.removeprefix("[[").removesuffix("]]")
        field, value = "plans", f"[[{name.removesuffix('.md')}]]"

    def change(text: str) -> str:
        """Append the link unless the card already has it."""
        values = as_list(read_fields(text).get(field))
        if value in values:
            return text
        return set_field(text, field, [*values, value])

    written = update_card(card, change)
    print(f"{card.key} {field}: {value}{'' if written else ' (already)'}")
    return 0


def add_session(text: str, today: str, session: AgentSession) -> str:
    """Append a session to ``sessions`` and log it, unless already there.

    Args:
        text: The card text.
        today: The date, as YYYY-MM-DD.
        session: The Herdr agent session.

    Returns:
        The new card text, the same text when the card has the session.
    """
    values = as_list(read_fields(text).get("sessions"))
    if session.entry in values:
        return text
    text = set_field(text, "sessions", [*values, session.entry])
    line = f"- {today} session: {session.name} ({session.entry})"
    return add_to_section(text, "Log", line)


def run_delete(args: argparse.Namespace, projects: Path) -> int:
    """Remove a card note. Unwanted work normally goes to dropped instead.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    card.path.unlink()
    print(f"deleted {card.path.name}")
    return 0


def run_check(args: argparse.Namespace, projects: Path) -> int:
    """Print cards whose layout or fields card cannot use.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code, 1 when a problem was found.
    """
    cards = load_all_cards(projects)
    problems = []
    seen: dict[str, str] = {}
    for card in cards:
        problems += [f"{card.key} {issue}" for issue in find_card_issues(card)]
        if card.key in seen:
            problems.append(f"{card.key} also in {seen[card.key]}")
        seen[card.key] = card.path.name
    for problem in problems:
        print(fit(problem))
    print(f"{len(problems)} problems in {len(cards)} cards")
    return 1 if problems else 0


def find_card_issues(card: Card) -> list[str]:
    """List what is wrong with one card's fields and sections.

    Args:
        card: The card.

    Returns:
        One short text per problem.
    """
    lines = card.text.split("\n")
    if not find_frontmatter_end(lines):
        return ["has no frontmatter"]
    issues = []
    if card.stage not in STAGES:
        issues.append(f"unknown stage {card.stage!r}")
    if card.kind not in KINDS:
        issues.append(f"unknown kind {card.kind!r}")
    for name in SECTIONS:
        if find_heading(lines, name) is None:
            issues.append(f"no {name} section")
    return issues


def render_card(
    key: str,
    title: str,
    project: str,
    kind: str,
    today: str,
    stage: str,
    log_entry: str,
) -> str:
    """Return the text of a new card.

    Args:
        key: The card key.
        title: The title for the first heading.
        project: The project folder name.
        kind: One of KINDS, or a template expression.
        today: The date, as YYYY-MM-DD.
        stage: The first stage.
        log_entry: The first Log line, without its date.

    Returns:
        The note text.
    """
    values: dict[str, str | list[str]] = {
        "tags": ["#Kanban"],
        "type": "card",
        "stage": stage,
        "stage_date": today,
        "template_version": "1",
        "project": project,
        "kind": kind,
        "date_created": today,
        "plans": [],
        "prs": [],
        "sessions": [],
        "topics": [f"[[{project}]]"],
    }
    lines = ["---"]
    for name in FIELDS:
        lines += render_field(name, values[name])
    lines += ["---", "", f"# {key} {title}", "---", ""]
    for name in SECTIONS:
        lines += [f"## {name}", "---", ""]
    lines.append(f"- {today} {log_entry}")
    return "\n".join(lines) + "\n"


def render_template() -> str:
    """Return the Templater template that makes a card in Obsidian.

    It asks for the project, title and kind, picks the next number
    from the folder listing and moves the note into the project
    folder. It takes no lock, so ``card check`` finds a clash.

    Returns:
        The template text.
    """
    script = TEMPLATER_SCRIPT.replace("KINDS_JSON", json.dumps(KINDS))
    text = render_card(
        key="<% key %>",
        title="<% title %>",
        project="<% project.name %>",
        kind="<% kind %>",
        today="<% today %>",
        stage=STAGES[0],
        log_entry=f"{STAGES[0]}: by the user in Obsidian",
    )
    return text.replace("---\n", "---\n" + script, 1)


# Runs inside Obsidian's Templater plugin. Keep its name rule the same
# as BAD_NAME_CHARS_RE and its number rule the same as find_next_number.
TEMPLATER_SCRIPT = r"""<%*
const root = "500 Zettelkasten/Projects";
const projects = [];
for (const folder of app.vault.getAbstractFileByPath(root).children) {
  const agents = app.vault.getAbstractFileByPath(`${folder.path}/AGENTS.md`);
  if (!agents) continue;
  const text = await app.vault.read(agents);
  const found = text.match(/Project code:\s*`?([A-Z][A-Z0-9]*)`?/);
  if (found) projects.push({ name: folder.name, code: found[1], folder });
}
const project = await tp.system.suggester(
  (p) => p.name, projects, true, "Project");
const title = (await tp.system.prompt("Card title", "", true)).trim();
const kinds = KINDS_JSON;
const kind = await tp.system.suggester(
  Object.entries(kinds).map(([name, meaning]) => `${name}: ${meaning}`),
  Object.keys(kinds), true, "Kind");
const numberRe = new RegExp(`^${project.code}-KB-(\\d+)`);
let high = 0;
for (const file of project.folder.children) {
  const found = file.name.match(numberRe);
  if (found) high = Math.max(high, Number(found[1]));
}
const key = `${project.code}-KB-${String(high + 1).padStart(3, "0")}`;
const today = tp.date.now("YYYY-MM-DD");
const name = title.replace(/[\\/:*?"<>|#^\[\]]/g, "").trim();
await tp.file.move(`${project.folder.path}/${key} ${name}`);
-%>
"""


def find_frontmatter_end(lines: list[str]) -> int:
    """Return the index of the line that closes the frontmatter.

    Args:
        lines: The card text split into lines.

    Returns:
        The index, or 0 when the card has no frontmatter.
    """
    if not lines or lines[0].strip() != "---":
        return 0
    for index in range(1, len(lines)):
        if lines[index].strip() == "---":
            return index
    return 0


def read_fields(text: str) -> dict[str, str | list[str]]:
    """Read the frontmatter fields card uses.

    It reads only what card writes and what Obsidian's property editor
    writes by hand: plain values, inline lists and indented lists.

    Args:
        text: The card text.

    Returns:
        Field values by name, a list for list fields.
    """
    lines = text.split("\n")
    fields: dict[str, str | list[str]] = {}
    name = None
    for line in lines[1 : find_frontmatter_end(lines)]:
        field = FIELD_RE.match(line)
        item = LIST_ITEM_RE.match(line)
        if field:
            name = field[1]
            fields[name] = parse_value(field[2].strip())
        elif item and name:
            fields[name] = [*as_list(fields[name]), unquote(item[1])]
    return fields


def parse_value(value: str) -> str | list[str]:
    """Parse a plain value or an inline list such as ``["a", b]``.

    Args:
        value: The text after the field name and colon.

    Returns:
        The unquoted value, or a list of them.
    """
    if not (value.startswith("[") and value.endswith("]")):
        return unquote(value)
    items = re.findall(r'"[^"]*"|\'[^\']*\'|[^,\s][^,]*', value[1:-1])
    return [unquote(item.strip()) for item in items]


def unquote(value: str) -> str:
    """Strip one pair of matching quotes.

    Args:
        value: A YAML value.

    Returns:
        The value without its quotes.
    """
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


def as_list(value: str | list[str] | None) -> list[str]:
    """Treat a missing or empty field as an empty list.

    Args:
        value: A field value.

    Returns:
        The value as a list.
    """
    if isinstance(value, list):
        return value
    return [value] if value else []


def render_field(name: str, value: str | list[str]) -> list[str]:
    """Render one frontmatter field as YAML lines.

    Args:
        name: The field name.
        value: A plain value, or a list.

    Returns:
        The lines for the field.
    """
    if isinstance(value, str):
        return [f"{name}: {value}"]
    if not value:
        return [f"{name}: []"]
    return [f"{name}:", *(f"  - {json.dumps(item)}" for item in value)]


def set_field(text: str, name: str, value: str | list[str]) -> str:
    """Replace one frontmatter field and leave the other lines as they are.

    A missing field goes after the field that FIELDS puts before it,
    such as ``sessions`` after ``prs``, else at the end.

    Args:
        text: The card text.
        name: The field name.
        value: The new value.

    Returns:
        The new card text.

    Raises:
        CardError: The card has no frontmatter.
    """
    lines = text.split("\n")
    end = find_frontmatter_end(lines)
    if not end:
        raise CardError("the card has no frontmatter; fix it by hand")
    bounds = find_field(lines, end, name)
    if bounds is None:
        earlier = FIELDS[: FIELDS.index(name)] if name in FIELDS else []
        found = [
            other_bounds
            for other in reversed(earlier)
            if (other_bounds := find_field(lines, end, other))
        ]
        place = found[0][1] if found else end
        bounds = (place, place)
    lines[bounds[0] : bounds[1]] = render_field(name, value)
    return "\n".join(lines)


def find_field(
    lines: list[str], end: int, name: str
) -> tuple[int, int] | None:
    """Return the lines a frontmatter field spans, list items included.

    Args:
        lines: The card text split into lines.
        end: The index of the line that closes the frontmatter.
        name: The field name.

    Returns:
        Start and stop indexes, or None when the field is missing.
    """
    for start in range(1, end):
        field = FIELD_RE.match(lines[start])
        if field and field[1] == name:
            stop = start + 1
            while stop < end and lines[stop][:1] in (" ", "\t"):
                stop += 1
            return start, stop
    return None


def read_title(text: str, key: str) -> str:
    """Return the first heading without the key.

    Args:
        text: The card text.
        key: The card key.

    Returns:
        The title, or an empty string when there is no heading.
    """
    lines = text.split("\n")
    for line in lines[find_frontmatter_end(lines) + 1 :]:
        if line.startswith("# "):
            return line[2:].strip().removeprefix(key).strip()
    return ""


def set_title(text: str, key: str, title: str) -> str:
    """Replace the first heading with the key and a new title.

    Args:
        text: The card text.
        key: The card key.
        title: The new title.

    Returns:
        The new card text.
    """
    lines = text.split("\n")
    end = find_frontmatter_end(lines)
    for index in range(end + 1 if end else 0, len(lines)):
        if lines[index].startswith("# "):
            lines[index] = f"# {key} {title}"
            return "\n".join(lines)
    lines[end + 1 : end + 1] = ["", f"# {key} {title}", "---"]
    return "\n".join(lines)


def find_heading(lines: list[str], name: str) -> int | None:
    """Return the index of a ``##`` heading, matched in any case.

    Args:
        lines: The card text split into lines.
        name: The section name.

    Returns:
        The index, or None when the section is missing.
    """
    start = find_frontmatter_end(lines) + 1
    for index in range(start, len(lines)):
        heading = HEADING_RE.match(lines[index])
        if (
            heading
            and heading[1] == "##"
            and heading[2].lower() == name.lower()
        ):
            return index
    return None


def find_section(lines: list[str], name: str) -> tuple[int, int] | None:
    """Return where a section's text starts and stops.

    The text starts after the heading and its ``---`` line, and stops
    at the next ``#`` or ``##`` heading. Deeper headings stay inside.

    Args:
        lines: The card text split into lines.
        name: The section name.

    Returns:
        Start and stop indexes, or None when the section is missing.
    """
    heading = find_heading(lines, name)
    if heading is None:
        return None
    start = heading + 1
    if start < len(lines) and lines[start].strip() == "---":
        start += 1
    stop = start
    while stop < len(lines) and not HEADING_RE.match(lines[stop]):
        stop += 1
    return start, stop


def read_section(text: str, name: str) -> str | None:
    """Return a section's text, or None when the section is missing.

    Args:
        text: The card text.
        name: The section name.

    Returns:
        The text between the heading and the next heading.
    """
    lines = text.split("\n")
    bounds = find_section(lines, name)
    if bounds is None:
        return None
    return "\n".join(lines[bounds[0] : bounds[1]])


def add_to_section(text: str, name: str, entry: str) -> str:
    """Add a line at the end of a section, making the section if missing.

    Args:
        text: The card text.
        name: One of SECTIONS.
        entry: The line to add.

    Returns:
        The new card text.
    """
    lines = text.rstrip("\n").split("\n")
    if find_heading(lines, name) is None:
        insert_section(lines, name)
    bounds = find_section(lines, name)
    assert bounds is not None, "insert_section just made it"
    start, stop = bounds
    body = lines[start:stop]
    while body and not body[-1].strip():
        body.pop()
    body = [*body, entry] if body else ["", entry]
    if stop < len(lines):
        body.append("")
    lines[start:stop] = body
    return "\n".join(lines) + "\n"


def insert_section(lines: list[str], name: str) -> None:
    """Insert an empty section before the next section in SECTIONS order.

    Args:
        lines: The card text split into lines, changed in place.
        name: One of SECTIONS.
    """
    later = list(SECTIONS)[list(SECTIONS).index(name) + 1 :]
    places = [find_heading(lines, other) for other in later]
    place = next((index for index in places if index is not None), None)
    if place is None:
        lines += ["", f"## {name}", "---"]
    else:
        lines[place:place] = [f"## {name}", "---", ""]


def update_card(card: Card, change: Callable[[str], str]) -> bool:
    """Apply a change to a card and write it when the text changed.

    When the note changes on disk while card works, card reads it again
    and applies the change once more, so a hand edit saved in Obsidian
    is kept. A note that changes twice stops the command. The check runs
    just before the rename, so a save in that last moment can still be
    lost.

    Args:
        card: The card as read.
        change: Takes the card text and returns the new text. It may
            raise CardError to refuse the change.

    Returns:
        True when the card was written.

    Raises:
        CardError: The change was refused, or the note kept changing.
    """
    for attempt in range(2):
        if attempt:
            card = load_card(card.path)
        new_text = change(card.text)
        if new_text == card.text:
            return False
        if write_safely(card.path, card.key, new_text, card.mtime_ns):
            return True
    raise CardError(f"{card.key} changed twice while writing; try again")


def write_safely(
    path: Path, key: str, text: str, expected_mtime_ns: int | None = None
) -> bool:
    """Write a note through ``.<KEY>.tmp`` and a rename.

    Args:
        path: The note to write.
        key: The card key, for the temporary file name.
        text: The new note text.
        expected_mtime_ns: The note's modified time when read, or None
            for a new note.

    Returns:
        True when written. False when the note changed since it was
        read, and nothing was written.
    """
    temp = path.with_name(f".{key}.tmp")
    with open(temp, "w", encoding="utf-8") as handle:
        handle.write(text)
        handle.flush()
        os.fsync(handle.fileno())
    if expected_mtime_ns is not None:
        if path.stat().st_mtime_ns != expected_mtime_ns:
            temp.unlink()
            return False
    os.replace(temp, path)
    return True


@contextmanager
def hold_lock(path: Path) -> Iterator[None]:
    """Hold an exclusive lock file on this computer.

    Args:
        path: The lock file, made if missing.

    Yields:
        Nothing; the lock is held inside the ``with`` block.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        yield


def load_card(path: Path) -> Card:
    """Read a card note from disk.

    Args:
        path: The note.

    Returns:
        The card.

    Raises:
        CardError: The file name holds no card key.
    """
    match = CARD_NAME_RE.match(path.name)
    if not match:
        raise CardError(f"{path.name} is not a card note")
    mtime_ns = path.stat().st_mtime_ns
    return Card(path, match[1], path.read_text(encoding="utf-8"), mtime_ns)


def load_all_cards(projects: Path) -> list[Card]:
    """Read every card note in every project folder.

    Args:
        projects: The vault's Projects folder.

    Returns:
        The cards.
    """
    return [
        load_card(path)
        for path in sorted(projects.glob("*/*-KB-*.md"))
        if CARD_NAME_RE.match(path.name)
    ]


def filter_cards(cards: list[Card], args: argparse.Namespace) -> list[Card]:
    """Keep the cards that match the --stage, --project and --kind filters.

    Args:
        cards: The cards.
        args: The parsed arguments.

    Returns:
        The matching cards.
    """
    return [
        card
        for card in cards
        if (not args.stage or card.stage == args.stage)
        and (not args.kind or card.kind == args.kind)
        and (not args.project or card.project.lower() == args.project.lower())
    ]


def find_card(projects: Path, key: str) -> Card:
    """Find the one note that holds a key, in any project.

    Args:
        projects: The vault's Projects folder.
        key: The card key, in any case.

    Returns:
        The card.

    Raises:
        CardError: No note or more than one note holds the key.
    """
    key = key.upper()
    paths = [
        path
        for path in projects.glob(f"*/{key}*.md")
        if (match := CARD_NAME_RE.match(path.name)) and match[1] == key
    ]
    if not paths:
        raise CardError(f"no card {key}")
    if len(paths) > 1:
        names = ", ".join(path.name for path in paths)
        raise CardError(f"two files hold {key}, fix by hand: {names}")
    return load_card(paths[0])


def list_key_holders(folder: Path, key: str) -> list[Path]:
    """List the notes in a folder whose name starts with the key.

    Args:
        folder: A project folder.
        key: The card key.

    Returns:
        The notes.
    """
    return [
        path
        for path in folder.glob(f"{key}*.md")
        if (match := CARD_NAME_RE.match(path.name)) and match[1] == key
    ]


def find_project_folder(projects: Path, project: str) -> Path:
    """Find a project folder by name, in any case.

    Args:
        projects: The vault's Projects folder.
        project: The folder name.

    Returns:
        The folder.

    Raises:
        CardError: No folder has the name.
    """
    for folder in projects.iterdir():
        if folder.is_dir() and folder.name.lower() == project.lower():
            return folder
    raise CardError(f"no project folder {project} in {projects}")


def read_project_code(folder: Path) -> str:
    """Read the project code from the folder's AGENTS.md.

    Args:
        folder: A project folder.

    Returns:
        The code, such as PDEV.

    Raises:
        CardError: The project has no code. The user picks one.
    """
    try:
        text = (folder / "AGENTS.md").read_text(encoding="utf-8")
    except OSError:
        text = ""
    match = PROJECT_CODE_RE.search(text)
    if not match:
        raise CardError(
            f"{folder.name} has no 'Project code:' line in AGENTS.md."
            " Ask the user for one."
        )
    return match[1]


def find_projects_folder(vault: str | None) -> Path:
    """Ask nobs where the vault is and return its Projects folder.

    Args:
        vault: A vault name, or None for the default vault.

    Returns:
        The Projects folder.

    Raises:
        CardError: nobs failed or the folder is missing.
    """
    command = ["nobs", "vault", "path", *([vault] if vault else [])]
    try:
        proc = subprocess.run(
            command, capture_output=True, text=True, timeout=20
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CardError(f"{' '.join(command)} failed: {error}") from error
    if proc.returncode != 0:
        raise CardError(f"{' '.join(command)} failed: {proc.stderr.strip()}")
    projects = Path(proc.stdout.strip()) / PROJECTS_SUBDIR
    if not projects.is_dir():
        raise CardError(f"no folder {projects}")
    return projects


def find_author(args: argparse.Namespace) -> str:
    """Return who acts, for the Log.

    The name comes from --by, else from the Herdr agent running in this
    pane. A Log line with a guessed name could credit the user with an
    agent's move, so without either name card stops.

    Args:
        args: The parsed arguments.

    Returns:
        A name such as kanban-demo.

    Raises:
        CardError: Neither --by nor a Herdr agent name is available.
    """
    if args.by:
        return args.by
    hint = "Pass --by NAME; the user passes --by user."
    pane = os.environ.get("HERDR_PANE_ID")
    if not pane:
        raise CardError(f"no Herdr pane, so no author for the Log. {hint}")
    try:
        proc = subprocess.run(
            ["herdr", "agent", "list"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        agents = json.loads(proc.stdout)["result"]["agents"]
    except (OSError, subprocess.TimeoutExpired, ValueError, KeyError) as error:
        raise CardError(
            f"herdr agent list failed ({error}), so no author. {hint}"
        ) from error
    names = [
        agent.get("name") for agent in agents if agent.get("pane_id") == pane
    ]
    if not names or not names[0]:
        raise CardError(f"no named Herdr agent in pane {pane}. {hint}")
    return names[0]


def find_agent_session(agent: str) -> AgentSession:
    """Ask Herdr for an agent's kind and session id.

    Args:
        agent: A Herdr agent name or pane, or an empty string for the
            agent in this pane.

    Returns:
        The session. Its name is the target when the agent has none.

    Raises:
        CardError: There is no target, or Herdr has no session for it.
    """
    target = agent or os.environ.get("HERDR_PANE_ID", "")
    if not target:
        raise CardError("no Herdr pane, so pass --session AGENT")
    command = ["herdr", "agent", "get", target]
    try:
        proc = subprocess.run(
            command, capture_output=True, text=True, timeout=10
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CardError(f"{' '.join(command)} failed: {error}") from error
    try:
        found = json.loads(proc.stdout)["result"]["agent"]
        kind = found["agent"]
        session_id = found["agent_session"]["value"]
    except (ValueError, KeyError, TypeError) as error:
        raise CardError(
            f"{' '.join(command)} gave no session:"
            f" {(proc.stderr or proc.stdout).strip()}"
        ) from error
    return AgentSession(found.get("name") or target, f"{kind} {session_id}")


def rank_stage(stage: str) -> int:
    """Return the stage's place in STAGES, unknown stages last.

    Args:
        stage: A stage name.

    Returns:
        The sort rank.
    """
    return STAGES.index(stage) if stage in STAGES else len(STAGES)


def fit(text: str, width: int = WIDTH) -> str:
    """Cut a line to the screen width.

    Args:
        text: The line.
        width: The most characters to keep.

    Returns:
        The line, ending in an ellipsis when cut.
    """
    return text if len(text) <= width else text[: width - 1] + "…"


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
