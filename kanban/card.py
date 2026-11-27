#!/usr/bin/env python3
"""Keep kanban cards as notes in the Obsidian vault.

A card is one piece of work the user wants, like a GitHub issue. A plan
made from it is like a pull request. Each card is one note,
``<KEY> <Title>.md``, in its project's folder in the vault. The key,
such as DEMO-41, starts with the project code from that folder's
AGENTS.md. Done and dropped cards keep older keys such as DEMO-KB-012,
which card still reads; ``card rekey`` gives open ones new keys. The
key and the file name follow the naming rules in
``chezmoi/dot_config/pde/names.toml``, installed as
``~/.config/pde/names.toml``.

The tables FIELDS, SECTIONS, STAGES, KINDS and MOVES below define the
note layout and the rules. ``card template``
prints the Obsidian template from the same tables, so cards made in
either place match.

Usage::

    card [--vault NAME] COMMAND ...

    card new PROJECT TITLE [--kind K] [--user]  make a card in draft
    card new PROJECT --from FILE [--user]  title, kind, sections
    card new PROJECT TITLE --adopt STAGE --user [--why TEXT]
                                       [--session [AGENT]]
    card list [--stage S] [--project P] [--kind K] [--tabs | --json]
    card board [--json]                counts and the review queue
    card show KEY [--section S] [--json]  S is any section, in any case
    card outline KEY                   sections with their line numbers
    card grep PATTERN [--section S] [--stage S] [--project P]
    card move KEY STAGE [--user] [--why TEXT] [--session [AGENT]]
    card note KEY TEXT                 add a dated line to the Log
    card add KEY SECTION TEXT          add a bullet or criteria checkbox
    card attach KEY FILE               copy FILE to attachments, embed it
    card edit KEY SECTION N TEXT|--remove  replace or remove item N
    card edit KEY SECTION N --tick|--untick  tick or untick item N
    card set KEY title|kind VALUE
    card link KEY PLAN|PR_URL
    card link KEY --session [AGENT]    record a Herdr agent's session
    card merged PR_URL                 move cards linking it to done
    card delete KEY --user
    card rekey [--apply --user]        new keys for open old-key cards
    card check [--json]                cards with a broken layout or title
    card name RULE VALUE...            make a name, such as a branch
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
import functools
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import tomllib
from collections import Counter, deque
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import date
from pathlib import Path
from typing import Any, TextIO

STATE_DIR = Path.home() / ".local/state/card"
PROJECTS_SUBDIR = "500 Zettelkasten/Projects"

# Each stage's word on cards and the command line. Code names a stage
# only through these, so renaming a stage changes one line here.
DRAFT = "draft"
TODO = "todo"
ACTIVE = "active"
IN_REVIEW = "in-review"
BLOCKED = "blocked"
DONE = "done"
DROPPED = "dropped"
STAGES = [DRAFT, TODO, ACTIVE, IN_REVIEW, BLOCKED, DONE, DROPPED]
KINDS = {
    "build": "ends in a change to code, config or the running setup",
    "research": "ends in a document or a decision",
}
DEFAULT_KIND = "build"
# The allowed moves and who may make them. When no single move reaches
# a stage, card move takes the shortest chain, such as active to
# in-review to done.
MOVES = {
    (DRAFT, TODO): "staff",
    (TODO, DRAFT): "staff",
    (TODO, ACTIVE): "staff",
    (ACTIVE, TODO): "staff",
    (ACTIVE, DRAFT): "user",
    (ACTIVE, IN_REVIEW): "staff",
    (IN_REVIEW, ACTIVE): "user",
    (IN_REVIEW, DONE): "user",
    # A card in this stage waits only on people outside the team, so
    # it never counts against a limit.
    (ACTIVE, BLOCKED): "staff",
    (IN_REVIEW, BLOCKED): "staff",
    (BLOCKED, IN_REVIEW): "staff",
    (BLOCKED, ACTIVE): "staff",
    (BLOCKED, DONE): "user",
    (DROPPED, DRAFT): "user",
    **{
        (stage, DROPPED): "user"
        for stage in STAGES
        if stage not in (DONE, DROPPED)
    },
}
# Stages a chain of moves may pass through. Waiting on people outside
# the team needs a reason naming who, and giving up on a card is a
# decision, so a chain only ends in those stages.
CHAIN_STAGES = [DRAFT, TODO, ACTIVE, IN_REVIEW]
# Only the user brings work already under way onto the board, straight
# into one of these stages. These are not moves, so MOVES leaves them out.
ADOPT_STAGES = [ACTIVE, IN_REVIEW, BLOCKED]
# Cards in each stage across all projects. The user may move a card
# past a limit with --user.
ACTIVE_LIMIT = 8
REVIEW_LIMIT = 6

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
    "Notes",
    "Log",
    "Appendix",
]
# Sections the version 2 layout added. card check does not ask old
# cards for them.
VERSION_2_SECTIONS = ["Notes", "Appendix"]
# Sections that `run_add` and `run_edit` may fill, as may the input
# file of `run_new`. Notes is the user's own text, the Appendix holds
# agent notes parts, and only this program writes the Log.
WRITABLE_SECTIONS = [
    name for name in SECTIONS if name not in ("Notes", "Log", "Appendix")
]
CRITERIA = "Acceptance Criteria"
OPEN_QUESTIONS = "Open Questions"
DECISIONS = "Decisions"
TITLE_LIMIT = 50

WIDTH = 60
BOARD_WIDTH = 40

RULES_FILE = "~/.config/pde/names.toml"
CHECKOUT_RULES_FILE = (
    Path(__file__).resolve().parents[1] / "chezmoi/dot_config/pde/names.toml"
)
RULES_VERSION = 1
# A field in a template, such as {key|lower}, with an optional filter.
SLOT_RE = re.compile(r"\{([a-z_]+)(?:\|([a-z0-9]+))?\}")
FILTERS: dict[str, Callable[[str], str]] = {
    "lower": str.lower,
    "slug": lambda text: "-".join(re.findall(r"[a-z0-9]+", text.lower())),
    "pad3": lambda text: text.zfill(3),
}
PROJECT_CODE_RE = re.compile(r"Project code:\s*`?([A-Z][A-Z0-9]*)`?")
FIELD_RE = re.compile(r"^([A-Za-z_][\w-]*):(.*)$")
LIST_ITEM_RE = re.compile(r"^\s+-\s+(.*)$")
HEADING_RE = re.compile(r"^(#{1,2})\s+(.*?)\s*$")
# A #, ## or ### heading, the levels card outline reads.
OUTLINE_HEADING_RE = re.compile(r"^(#{1,3})\s+\S")
CHECKBOX_RE = re.compile(r"^\s*- \[[ xX]\] \S", re.M)
UNTICKED_RE = re.compile(r"^\s*(?:[-*+]|\d+[.)]) \[[^xX\]]\] \S", re.M)
# A GitHub pull request URL, maybe followed by a page such as /files.
PR_URL_RE = re.compile(
    r"^(?:https?://)?(?:www\.)?github\.com/([^/\s]+)/([^/\s]+)/pull/(\d+)",
    re.IGNORECASE,
)
# A section item's bullet or checkbox, then its words.
ITEM_PREFIX_RE = re.compile(r"^(- \[[ xX]\] |[-*] )?(.*)$")
# An Open Questions item's words after the bullet, such as
# `Q4 asked 2026-11-25: Deploy tonight?` or `Q2: Which bucket?`. A Q
# number over six digits reads as words, since the hub keeps it in 32 bits.
QUESTION_RE = re.compile(r"^Q(\d{1,6})(?: asked (\d{4}-\d\d-\d\d))?: (.*)$")
# Characters Obsidian refuses in file names, plus link syntax.
BAD_NAME_CHARS_RE = re.compile(r'[\\/:*?"<>|#^\[\]]')
# The target of a wikilink, before any #heading or |alias.
LINK_TARGET_RE = re.compile(r"\[\[([^\]|#\n]+)")


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
class NewCard:
    """What card new needs to write a card, from its arguments or a file.

    ``sections`` maps a name from WRITABLE_SECTIONS to the text under
    its heading.
    """

    title: str
    kind: str
    sections: dict[str, str]


@dataclass(frozen=True)
class AgentSession:
    """A Herdr agent's name and its entry for the sessions field.

    The entry is the agent kind and session id, such as
    ``opencode ses_e4dc27c20ffeJkm3JZ8dwLh7rW``.
    """

    name: str
    entry: str


@dataclass(frozen=True)
class NoteChange:
    """A note's new text, and its new path when card rekey renames it."""

    path: Path
    new_path: Path
    new_text: str
    mtime_ns: int


@dataclass(frozen=True)
class Rule:
    """One rule from names.toml.

    ``make`` builds a name and ``parse`` reads it back.
    """

    name: str
    make: str
    parse: re.Pattern[str]


@dataclass(frozen=True)
class TitleFinding:
    """What one title check found, such as ``T2 label: "board:"``.

    card refuses a title with any finding, and card check lists them.
    """

    rule: str
    name: str
    detail: str

    def __str__(self) -> str:
        """Return the finding as card prints it."""
        return f"{self.rule} {self.name}: {self.detail}"


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
        read_today()
        if args.command == "template":
            print(render_template(), end="")
            return 0
        if args.command == "name":
            return args.run(args)
        if projects is None:
            projects = find_projects_folder(args.vault)
        return args.run(args, projects)
    except CardError as error:
        print(f"card: {error}", file=sys.stderr)
        return 1


def read_today() -> date:
    shown = os.environ.get("CARD_TODAY")
    if not shown:
        return date.today()
    if re.fullmatch(r"\d{4}-\d{2}-\d{2}", shown):
        try:
            return date.fromisoformat(shown)
        except ValueError:
            pass
    raise CardError(f"CARD_TODAY must be a YYYY-MM-DD date, not {shown}")


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

    new = commands.add_parser(
        "new",
        help="make a card in draft",
        epilog=format_card_file_help(),
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    new.add_argument("project", help="project folder, such as DevEnv")
    new.add_argument("title", nargs="?", help="the title, unless --from")
    new.add_argument(
        "--from",
        dest="from_file",
        metavar="FILE",
        help="take the title, kind and sections from FILE, laid out below",
    )
    new.add_argument(
        "--kind",
        choices=list(KINDS),
        help="; ".join(f"{name} {meaning}" for name, meaning in KINDS.items())
        + f"; default {DEFAULT_KIND}",
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
    output = listing.add_mutually_exclusive_group()
    output.add_argument(
        "--tabs",
        action="store_true",
        help="whole lines for scripts: key, stage, kind, project, title",
    )
    add_json_option(output)
    listing.set_defaults(run=run_list)

    board = commands.add_parser("board", help="counts and review queue")
    add_json_option(board)
    board.set_defaults(run=run_board)

    show = commands.add_parser("show", help="print a card")
    show.add_argument("key")
    show.add_argument("--section", help="any ## heading, in any case")
    add_json_option(show)
    show.set_defaults(run=run_show)

    outline = commands.add_parser(
        "outline",
        help="list sections with their line numbers",
    )
    outline.add_argument("key")
    outline.set_defaults(run=run_outline)

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
    add_author(note)
    note.set_defaults(run=run_note)

    add = commands.add_parser(
        "add",
        help="add a bullet, or a criteria checkbox, to a section",
    )
    add.add_argument("key")
    add.add_argument("section", choices=WRITABLE_SECTIONS)
    add.add_argument("text")
    add.set_defaults(run=run_add)

    attach = commands.add_parser(
        "attach",
        help="copy a file to attachments and embed it in Context",
    )
    attach.add_argument("key")
    attach.add_argument("file")
    attach.set_defaults(run=run_attach)

    edit = commands.add_parser(
        "edit",
        help="replace, remove, tick or untick one item of a section",
        description="N is the item's number in card show KEY --section S,"
        " or Q<n> for the Open Questions item numbered Q<n>.",
    )
    edit.add_argument("key")
    edit.add_argument("section", choices=WRITABLE_SECTIONS)
    edit.add_argument("number", metavar="N")
    edit.add_argument("text", nargs="?", help="the item's new text")
    edit.add_argument("--remove", action="store_true", help="remove item N")
    edit.add_argument(
        "--tick",
        action="store_true",
        help="set item N's checkbox to - [x]",
    )
    edit.add_argument(
        "--untick",
        action="store_true",
        help="set item N's checkbox to - [ ]",
    )
    add_author(edit)
    edit.set_defaults(run=run_edit)

    set_ = commands.add_parser("set", help="change the title or kind")
    set_.add_argument("key")
    set_.add_argument("field", choices=["title", "kind"])
    set_.add_argument("value")
    set_.set_defaults(run=run_set)

    link = commands.add_parser(
        "link",
        help="link a plan note, a PR URL or a Herdr session",
    )
    link.add_argument("key")
    link.add_argument("target", nargs="?", help="plan note name, or a PR URL")
    add_session_option(link, "")
    link.set_defaults(run=run_link)

    merged = commands.add_parser(
        "merged",
        help="move the cards linking a merged PR to done",
        description="A card moves only when all its PRs merged and all its"
        " Acceptance Criteria are ticked. Otherwise it stays, and its Log"
        " says why; when gh fails, it stays unchanged.",
    )
    merged.add_argument("url", help="the merged PR's URL")
    merged.set_defaults(run=run_merged)

    delete = commands.add_parser("delete", help="remove a card note")
    delete.add_argument("key")
    delete.add_argument("--user", action="store_true", required=True)
    delete.set_defaults(run=run_delete)

    rekey = commands.add_parser(
        "rekey",
        help="give open cards with old keys new keys",
    )
    rekey.add_argument(
        "--apply",
        action="store_true",
        help="make the changes; without it, only print them",
    )
    rekey.add_argument(
        "--user",
        action="store_true",
        help="the user asked for this; needed with --apply",
    )
    rekey.set_defaults(run=run_rekey)

    check = commands.add_parser("check", help="find broken cards")
    add_json_option(check)
    check.set_defaults(run=run_check)

    name = commands.add_parser(
        "name",
        help="make a name from a naming rule",
        description="VALUEs fill the rule's fields in order, and the last"
        " field takes the rest, such as: card name branch feat retry uploads",
    )
    name.add_argument("rule", help="a rule in names.toml, such as branch")
    name.add_argument("values", nargs="*", metavar="VALUE")
    name.set_defaults(run=run_name)

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


def add_json_option(
    parser: argparse.ArgumentParser | argparse._MutuallyExclusiveGroup,
) -> None:
    """Add the --json option for commands that scripts read.

    Args:
        parser: The command's parser, or a group of options that
            exclude each other, such as card list's --tabs.
    """
    parser.add_argument(
        "--json",
        action="store_true",
        help="print JSON for scripts, with whole titles",
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
        " with --user):",
    ]
    for (source, target), who in MOVES.items():
        lines.append(f"  {source} -> {target}: {who}")
    lines.append(
        "a stage no single move reaches takes the shortest chain of these"
        f" moves, through {', '.join(CHAIN_STAGES[:-1])} and"
        f" {CHAIN_STAGES[-1]}, one Log line each",
    )
    lines.append(
        f"the user moves a card past the limits of {ACTIVE_LIMIT} {ACTIVE}"
        f" and {REVIEW_LIMIT} {IN_REVIEW} with --user",
    )
    lines.append(
        f"a move to {BLOCKED} needs --why, naming who the card waits on",
    )
    lines.append("work already under way: card new --adopt STAGE --user")
    return "\n".join(lines)


def format_card_file_help() -> str:
    """Format help text that shows the layout of a --from file.

    Returns:
        The help text.
    """
    return "\n".join(
        [
            "--from FILE holds the title and kind, then sections:",
            "  ---",
            "  title: Retry failed uploads",
            "  kind: build",
            "  ---",
            "",
            "  ## Context",
            "  - Uploads fail on a slow link.",
            "",
            "  ## Acceptance Criteria",
            "  - [ ] A failed upload retries.",
            "",
            f"kind is optional, {DEFAULT_KIND} by default. Each of these",
            "sections may appear once, in any letter case:",
            f"  {', '.join(WRITABLE_SECTIONS)}",
            "Section text is copied as written, so criteria need - [ ].",
        ],
    )


def run_new(args: argparse.Namespace, projects: Path) -> int:
    """Make a card in draft, or adopt work under way, and print its name.

    A title with a check_title finding is refused.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    if (args.title is None) == (args.from_file is None):
        raise CardError("give either a TITLE or --from FILE")
    if args.from_file is not None and args.kind:
        raise CardError("with --from, FILE gives the kind")
    if args.adopt and not args.user:
        raise CardError(
            "only the user adopts work already under way."
            " Pass --user only when the user said so.",
        )
    if args.session is not None and not args.adopt:
        raise CardError(
            "--session goes with --adopt. A draft card gets its session"
            " when it moves to active.",
        )
    require_blocked_reason(args.adopt, args.why)
    if args.from_file is not None:
        new_card = read_card_file(Path(args.from_file))
    else:
        new_card = NewCard(args.title, args.kind or DEFAULT_KIND, {})
    require_passing_title(new_card.title.strip())
    stage = args.adopt or STAGES[0]
    verb = "adopted by" if args.adopt else "by"
    entry = format_log_entry(
        stage,
        f"{verb} {find_author(args)}",
        args.user,
        args.why,
    )
    session = None
    if args.session is not None:
        session = find_agent_session(args.session)
    path = create_card(projects, args.project, new_card, stage, entry, session)
    print(path.stem)
    return 0


def read_card_file(path: Path) -> NewCard:
    """Read a new card's title, kind and section text from a file.

    The file has frontmatter with ``title`` and an optional ``kind``,
    then ``##`` headings named as in WRITABLE_SECTIONS, in any case.
    Any other heading, or text before the first heading, is refused so
    that no line is dropped without notice.

    Args:
        path: The file, laid out as ``card new --help`` shows.

    Returns:
        The card's title, kind and section text.

    Raises:
        CardError: The file cannot be read, has no title, names an
            unknown kind, or holds text outside the allowed sections.
    """
    try:
        text = path.read_text()
    except OSError as error:
        raise CardError(f"cannot read {path}: {error.strerror}") from error
    fields = read_fields(text)
    title = str(fields.get("title", "")).strip()
    if not title:
        raise CardError(f"{path} needs a title field in its frontmatter")
    kind = str(fields.get("kind") or DEFAULT_KIND)
    if kind not in KINDS:
        raise CardError(f"kind is one of {', '.join(KINDS)}")
    allowed = [name.lower() for name in WRITABLE_SECTIONS]
    seen: set[str] = set()
    lines = text.split("\n")
    for line in lines[find_frontmatter_end(lines) + 1 :]:
        heading = HEADING_RE.match(line)
        if heading is None:
            if line.strip() and not seen:
                raise CardError(
                    f"{path} has text before its first section: {line!r}",
                )
            continue
        name = heading[2].lower()
        if heading[1] != "##" or name not in allowed or name in seen:
            raise CardError(
                f"{path} has the heading {line.strip()!r}. Each section"
                f" may appear once: {', '.join(WRITABLE_SECTIONS)}",
            )
        seen.add(name)
    sections = {
        name: body.strip("\n")
        for name in WRITABLE_SECTIONS
        if (body := read_section(text, name)) and body.strip()
    }
    return NewCard(title, kind, sections)


def create_card(
    projects: Path,
    project: str,
    new_card: NewCard,
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
        new_card: The title, kind and section text.
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
    name = BAD_NAME_CHARS_RE.sub("", new_card.title).strip()
    if not name:
        raise CardError("the title needs a letter or digit")
    today = read_today().isoformat()
    with hold_lock(STATE_DIR / "new.lock"):
        number = str(find_next_number(projects, code))
        key = make_name(find_rule("card_key"), {"code": code, "n": number})
        path = folder / make_name(
            find_rule("card_file"),
            {"key": key, "title": name},
        )
        text = render_card(
            key=key,
            title=new_card.title.strip(),
            project=folder.name,
            kind=new_card.kind,
            today=today,
            stage=stage,
            log_entry=log_entry,
            sections=new_card.sections,
        )
        if session:
            text = add_session(text, today, session)
        write_safely(path, key, text)
    holders = list_key_holders(folder, key)
    if len(holders) > 1:
        names = ", ".join(holder.name for holder in holders)
        raise CardError(f"two files hold {key}, fix by hand: {names}")
    return path


def find_next_number(projects: Path, code: str) -> int:
    """Return one more than the highest card or plan number of a code.

    A plan made for a card shares its key, and plans sit in any project
    folder or archive, so every folder counts. Old keys such as
    PDEV-KB-012 count too, so a new key never repeats an old card's
    number.

    Args:
        projects: The vault's Projects folder.
        code: The project code, such as PDEV.

    Returns:
        The number for the next card.
    """
    card_file = find_rule("card_file")
    plan_file = find_rule("plan_file")
    numbers = [0]
    for path in projects.glob(f"**/{code}-*.md"):
        name = path.name
        fields = parse_name(card_file, name) or parse_name(plan_file, name)
        if fields:
            digits = re.sub("[^0-9]", "", fields["key"][len(code) :])
            numbers.append(int(digits))
    return max(numbers) + 1


def run_list(args: argparse.Namespace, projects: Path) -> int:
    """Print one line per card, in stage order.

    With --json, print a list of describe_card objects instead.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    cards = filter_cards(load_all_cards(projects), args)
    cards.sort(key=lambda card: (rank_stage(card.stage), card.key))
    if args.json:
        print_json([describe_card(card) for card in cards])
        return 0
    for card in cards:
        if args.tabs:
            fields = [card.key, card.stage, card.kind, card.project]
            print(*fields, card.title, sep="\t")
            continue
        print(fit(f"{card.key} {card.stage:<9} {card.kind:<8} {card.title}"))
    print(f"{len(cards)} cards")
    return 0


def run_board(args: argparse.Namespace, projects: Path) -> int:
    """Print counts per stage, then the in-review and blocked cards.

    The hub reads the limits line, the blocked line and the BLOCKED
    list, so keep their words as they are. Both lists put the card that
    has waited longest first. The blocked line and list appear only
    when a card is in that stage. With --json, print the same counts,
    limits and lists as one object.

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
    today = read_today()
    blocked = [
        (count_days_since(card.stage_date, today), card)
        for card in cards
        if card.stage == BLOCKED
    ]
    blocked.sort(key=lambda entry: (-entry[0], entry[1].key))
    in_review = [card for card in cards if card.stage == IN_REVIEW]
    in_review.sort(key=lambda card: (card.stage_date, card.key))
    if args.json:
        print_json(
            {
                "counts": counts,
                "limits": {ACTIVE: ACTIVE_LIMIT, IN_REVIEW: REVIEW_LIMIT},
                IN_REVIEW: [describe_card(card) for card in in_review],
                BLOCKED: [
                    {
                        **describe_card(card),
                        "days": days,
                        "reason": read_blocked_reason(card.text),
                    }
                    for days, card in blocked
                ],
            },
        )
        return 0
    # Three lines of stages keep each one within 40 columns.
    for stages in (STAGES[:3], STAGES[3:5], STAGES[5:]):
        print("  ".join(f"{stage} {counts[stage]}" for stage in stages))
    print(
        f"{ACTIVE} {counts[ACTIVE]} of {ACTIVE_LIMIT},"
        f" {IN_REVIEW} {counts[IN_REVIEW]} of {REVIEW_LIMIT}",
    )
    if blocked:
        oldest = format_days(blocked[0][0])
        print(f"{BLOCKED} {len(blocked)}, oldest {oldest}")
    print(f"\n{IN_REVIEW.upper()}, oldest first ({len(in_review)})")
    for card in in_review:
        print(fit(f"- {card.key} {card.stage_date}", BOARD_WIDTH))
        print(fit(f"  {card.title}", BOARD_WIDTH))
    if blocked:
        print(f"\n{BLOCKED.upper()}, oldest first ({len(blocked)})")
    for days, card in blocked:
        print(fit(f"- {card.key} {format_days(days)}", BOARD_WIDTH))
        print(fit(f"  {card.title}", BOARD_WIDTH))
        reason = read_blocked_reason(card.text)
        if reason:
            print(fit(f"  {reason}", BOARD_WIDTH))
    return 0


def count_days_since(day: str, today: date) -> int:
    """Return whole days from an ISO date to today.

    A date that does not parse, such as a hand-edited stage_date,
    counts as 0, so one bad card does not stop the board.

    Args:
        day: A date such as 2026-11-05.
        today: Today's date.

    Returns:
        The number of days, never below 0.
    """
    try:
        return max((today - date.fromisoformat(day)).days, 0)
    except ValueError:
        return 0


def format_days(days: int) -> str:
    """Return ``1 day`` or ``N days``.

    Args:
        days: A number of days.

    Returns:
        The text.
    """
    return "1 day" if days == 1 else f"{days} days"


def read_blocked_reason(text: str) -> str:
    """Return the --why of a card's newest move to blocked.

    format_log_entry writes ``blocked: by AUTHOR. WHY``, so the reason
    is the text after the first ". ".

    Args:
        text: The card text.

    Returns:
        The reason, or an empty string when no Log line has one.
    """
    reason = ""
    for line in (read_section(text, "Log") or "").split("\n"):
        words = line.split(" ", 2)
        if len(words) < 3 or words[0] != "-":
            continue
        entry = words[2]
        if entry.startswith(f"{BLOCKED}: "):
            reason = entry.partition(". ")[2].strip()
    return reason


def describe_card(card: Card) -> dict[str, Any]:
    """Return the fields scripts read from a card, for --json output.

    Titles stay whole. ``open_questions`` is list_questions' result.

    Args:
        card: The card.

    Returns:
        A JSON-ready object keyed by field name.
    """
    fields = read_fields(card.text)
    return {
        "key": card.key,
        "title": card.title,
        "stage": card.stage,
        "kind": card.kind,
        "project": card.project,
        "stage_date": card.stage_date,
        "plans": as_list(fields.get("plans")),
        "prs": as_list(fields.get("prs")),
        "sessions": as_list(fields.get("sessions")),
        "open_questions": list_questions(card.text),
    }


def list_questions(text: str) -> list[dict[str, Any]]:
    """Return each Open Questions item with its Q number and asked date.

    An item with no ``Q<n>`` gets the highest number written in Open
    Questions or Decisions plus its place among such items, so
    ``- Q3: a`` then ``- b`` gives Q3 and Q4.

    Args:
        text: The card text.

    Returns:
        One object per item, in note order, such as ``{"number": 4,
        "asked": "2026-11-25", "text": "Deploy tonight?", "lines":
        ["- A. Yes"]}``. ``asked`` is None for an undated item.
    """
    lines = text.split("\n")
    items = []
    for start, stop in list_items(lines, OPEN_QUESTIONS):
        words = split_item(lines[start])[1]
        later = [line.removeprefix("  ") for line in lines[start + 1 : stop]]
        number: int | None = None
        asked = None
        match = QUESTION_RE.match(words)
        if match:
            number, words = int(match[1]), match[3]
            if match[2] and is_valid_date(match[2]):
                asked = match[2]
        items.append(
            {"number": number, "asked": asked, "text": words, "lines": later},
        )
    highest = max((item["number"] or 0 for item in items), default=0)
    highest = max(highest, find_highest_answered_number(lines))
    for item in items:
        if item["number"] is None:
            highest += 1
            item["number"] = highest
    return items


def is_valid_date(day: str) -> bool:
    """Return True when ``day`` is a real ISO date, such as 2026-11-25.

    Args:
        day: Text shaped like a date.

    Returns:
        Whether date.fromisoformat reads it.
    """
    try:
        date.fromisoformat(day)
    except ValueError:
        return False
    return True


def print_json(value: object) -> None:
    """Print a value as indented JSON, keeping non-ASCII text as is.

    Args:
        value: Any value json.dumps accepts.
    """
    print(json.dumps(value, indent=2, ensure_ascii=False))


def run_show(args: argparse.Namespace, projects: Path) -> int:
    """Print a whole card, or one section of it.

    --section takes any ``##`` heading in any case, and a SECTIONS name the
    card lacks prints as empty. With --json, print describe_card's object
    plus ``sections``, which maps each name from list_section_names, or
    only --section, to its text.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    lines = card.text.split("\n")
    section = None
    if args.section:
        section = find_section_name(lines, args.section)
        if section is None:
            raise CardError(f"{card.key} has no {args.section} section")
    if args.json:
        names = [section] if section else list_section_names(lines)
        sections = {
            name: (read_section(card.text, name) or "").strip()
            for name in names
        }
        print_json({**describe_card(card), "sections": sections})
        return 0
    if section is None:
        print(card.text, end="")
        return 0
    numbers = {
        start: number
        for number, (start, _) in enumerate(list_items(lines, section), 1)
    }
    # A known section the card lacks has no lines to show.
    bounds = find_section(lines, section) or (0, 0)
    shown = [
        f"{numbers[index]}. {lines[index]}"
        if index in numbers
        else lines[index]
        for index in range(*bounds)
    ]
    print(f"## {section}\n" + "\n".join(shown).strip())
    return 0


def find_section_name(lines: list[str], name: str) -> str | None:
    """Return a section name as the card's heading or SECTIONS spells it.

    Args:
        lines: The card text split into lines.
        name: A section name in any case, such as ``open questions``.

    Returns:
        The name, such as ``Open Questions``, or None when the card has
        no such heading and SECTIONS does not name it.
    """
    index = find_heading(lines, name)
    if index is not None:
        heading = HEADING_RE.match(lines[index])
        assert heading, "find_heading matched this line"
        return heading[2]
    known = [
        section for section in SECTIONS if section.lower() == name.lower()
    ]
    return known[0] if known else None


def list_section_names(lines: list[str]) -> list[str]:
    """List SECTIONS, then each other ``##`` heading of the card in order.

    Args:
        lines: The card text split into lines.

    Returns:
        The names, with the card's other headings as it spells them.
    """
    names = list(SECTIONS)
    seen = {name.lower() for name in names}
    for line in lines[find_frontmatter_end(lines) + 1 :]:
        heading = HEADING_RE.match(line)
        if heading and heading[1] == "##" and heading[2].lower() not in seen:
            names.append(heading[2])
            seen.add(heading[2].lower())
    return names


def run_outline(args: argparse.Namespace, projects: Path) -> int:
    """Print each ``##`` and ``###`` heading with the lines its part covers.

    Lines count from 1, as ``sed -n`` counts them, such as
    ``21-26     ## Context``. A part ends before the next heading of its
    level or higher, so the Appendix covers its Agent Notes parts.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    lines = card.text.splitlines()
    headings = []
    for index in range(find_frontmatter_end(lines) + 1, len(lines)):
        heading = OUTLINE_HEADING_RE.match(lines[index])
        if heading:
            headings.append((index, len(heading[1])))
    for position, (index, level) in enumerate(headings):
        if level == 1:
            continue
        later = headings[position + 1 :]
        ends = [other for other, other_level in later if other_level <= level]
        span = f"{index + 1}-{ends[0] if ends else len(lines)}"
        print(f"{span:<9} {'  ' * (level - 2)}{lines[index]}")
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
    """Move a card to another stage and add a Log line per move.

    When no single move reaches the stage, the card takes the shortest
    chain of moves the caller may make, in one write, and each move
    gets its own Log line and printed line.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    if args.session is not None and args.stage != ACTIVE:
        raise CardError(f"--session goes with a move to {ACTIVE}")
    require_blocked_reason(args.stage, args.why)
    card = find_card(projects, args.key)
    today = read_today().isoformat()
    author = f"by {find_author(args)}"
    session = None
    if args.session is not None:
        session = find_agent_session(args.session)
    chain: list[str] = []

    def change(text: str) -> str:
        """Check the moves against the card as it is now, then make them."""
        current = str(read_fields(text).get("stage", ""))
        chain[:] = plan_moves(
            text,
            card.key,
            current,
            args.stage,
            args.user,
            projects,
        )
        for stage in chain:
            why = args.why if stage == args.stage else None
            entry = format_log_entry(stage, author, args.user, why)
            text = add_to_section(text, "Log", f"- {today} {entry}")
        text = set_field(text, "stage", args.stage)
        text = set_field(text, "stage_date", today)
        return add_session(text, today, session) if session else text

    update_card(card, change)
    for stage in chain:
        print(f"{card.key} {stage}")
    return 0


def require_blocked_reason(stage: str | None, why: str | None) -> None:
    """Refuse a card entering blocked without a --why.

    The board shows the reason, so the user knows whom to chase.

    Args:
        stage: The stage the card enters, or None.
        why: The --why text, or None.

    Raises:
        CardError: The stage is blocked and why is empty.
    """
    if stage == BLOCKED and not (why and why.strip()):
        raise CardError(f"{BLOCKED} needs --why naming who the card waits on")


def format_log_entry(
    stage: str,
    author: str,
    user: bool,
    why: str | None,
) -> str:
    """Return a stage Log line without its date.

    Args:
        stage: The stage the card enters.
        author: Who acts, such as ``by general-staff``.
        user: True when the user made the decision.
        why: A reason, or None.

    Returns:
        A line such as ``in-review: by general-staff for the user. PR open``.
    """
    entry = f"{stage}: {author}"
    # The Log is the only record that the user, not the agent, decided.
    if user:
        entry += " for the user"
    if why:
        entry += f". {why}"
    return entry


def plan_moves(
    text: str,
    key: str,
    current: str,
    target: str,
    user: bool,
    projects: Path,
) -> list[str]:
    """Return the stages a move passes through, target last.

    Each move in the chain is checked as if it were made alone, so a
    chain passes no rule, limit or criteria check that a single move
    would fail.

    Args:
        text: The card text.
        key: The card's key, for the hint in a refusal.
        current: The card's stage now.
        target: The stage to move to.
        user: True when the user made the move.
        projects: The vault's Projects folder, for the stage limits.

    Returns:
        The stages in order, such as ``["in-review", "done"]``.

    Raises:
        CardError: No allowed chain reaches target, with a hint, or a
            move in the chain is refused.
    """
    chain = find_move_chain(current, target, user)
    if chain is None:
        raise CardError(format_refused_move(key, current, target, user))
    source = current
    for stage in chain:
        check_move(text, key, source, stage, user, projects)
        source = stage
    return chain


def find_move_chain(current: str, target: str, user: bool) -> list[str] | None:
    """Return the shortest chain of moves from current to target.

    Args:
        current: The card's stage now.
        target: The stage to move to.
        user: True to use every move, False for staff moves only.

    Returns:
        The stages after current, target last, or None when no chain of
        moves through CHAIN_STAGES reaches target.
    """
    if current == target:
        return None
    moves = [move for move, who in MOVES.items() if user or who == "staff"]
    came_from = {current: current}
    queue = deque([current])
    while queue and target not in came_from:
        stage = queue.popleft()
        if stage != current and stage not in CHAIN_STAGES:
            continue
        for source, to in moves:
            if source == stage and to not in came_from:
                came_from[to] = stage
                queue.append(to)
    if target not in came_from:
        return None
    chain = [target]
    while came_from[chain[-1]] != current:
        chain.append(came_from[chain[-1]])
    return chain[::-1]


def format_refused_move(
    key: str,
    current: str,
    target: str,
    user: bool,
) -> str:
    """Return the one-line reason no chain of moves reaches target.

    For staff, it names the first user move on the way, and the stage
    staff can reach before it with the command that gets there.

    Args:
        key: The card's key.
        current: The card's stage now.
        target: The stage to move to.
        user: True when the user made the move.

    Returns:
        The reason, such as ``only the user moves in-review -> done.
        You can move to in-review: card move KEY in-review``.
    """
    chain = None if user else find_move_chain(current, target, True)
    if chain is None:
        options = [to for (source, to) in MOVES if source == current]
        allowed = ", ".join(options) or "none, the stage is unknown"
        return f"no move {current} -> {target}. Allowed: {allowed}"
    source = current
    for stage in chain:
        if MOVES[(source, stage)] == "user":
            break
        source = stage
    reason = f"only the user moves {source} -> {stage}"
    if source != current:
        return f"{reason}. You can move to {source}: card move {key} {source}"
    if stage != target:
        reason += f", on the way to {target}"
    return f"{reason}. Pass --user only when the user said so."


def check_move(
    text: str,
    key: str,
    current: str,
    target: str,
    user: bool,
    projects: Path,
) -> None:
    """Refuse a single move for its criteria, its title or a full stage.

    find_move_chain already keeps to MOVES and to the caller's moves.

    Args:
        text: The card text.
        key: The card's key, which read_title drops from the heading.
        current: The card's stage now.
        target: The stage to move to.
        user: True when the user made the move.
        projects: The vault's Projects folder, for the stage limits.

    Raises:
        CardError: The move is not allowed, with the reason.
    """
    if (current, target) == (DRAFT, TODO) and not CHECKBOX_RE.search(
        read_section(text, CRITERIA) or "",
    ):
        raise CardError(
            f"add a '- [ ] ...' item to {CRITERIA} before moving to {TODO}",
        )
    # Workers start todo cards, so every move into todo is checked,
    # a staff pause included.
    if target == TODO:
        require_passing_title(read_title(text, key))
    # Only staff moves can fill a stage that has a limit; the user's
    # moves pass.
    limit = {ACTIVE: ACTIVE_LIMIT, IN_REVIEW: REVIEW_LIMIT}.get(target)
    if limit is not None and not user:
        count = sum(card.stage == target for card in load_all_cards(projects))
        if count >= limit:
            raise CardError(
                f"{count} cards are in {target}, the limit is {limit}."
                " Ask the user which to finish first.",
            )


def run_merged(args: argparse.Namespace, projects: Path) -> int:
    """Move each open card that links a merged PR to done, or log why not.

    A merge is the user's approval, so the move is the user's and an
    active card passes through in-review. One card's failure leaves
    that card as it was, and the other cards still go.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code, 1 when gh failed or a card could not be written.
    """
    merged = normalize_pr_url(args.url)
    cards = [
        card
        for card in load_all_cards(projects)
        if card.stage not in (DONE, DROPPED)
        and merged in list_card_prs(card.text)
    ]
    if not cards:
        print(f"no open card links {merged}")
        return 0
    code = 0
    for card in cards:
        try:
            print(apply_merge_to_card(card, merged, projects))
        except (CardError, OSError) as error:
            print(f"card: {card.key} unchanged: {error}", file=sys.stderr)
            code = 1
    return code


def apply_merge_to_card(card: Card, merged: str, projects: Path) -> str:
    """Move a card to done after its PR merged, or add a Log note why not.

    The merged PR needs no gh call. The card stays when gh says another
    of its PRs has not merged, when an Acceptance Criteria box is not
    ticked, or when the move rules refuse a move of the chain.

    Args:
        card: An open card that links the merged PR.
        merged: The merged PR's URL, normalized.
        projects: The vault's Projects folder, for the move rules.

    Returns:
        The line to print, such as ``PDEV-5 active -> in-review -> done``.

    Raises:
        CardError: gh failed, or the note kept changing.
    """
    unmerged = []
    for url in list_card_prs(card.text):
        if url != merged and (state := read_pr_state(url)) != "MERGED":
            unmerged.append(f"{url} is {state.lower()}")
    today = read_today().isoformat()
    author = "by card merged"
    line = ""

    def change(text: str) -> str:
        """Check the card as it is now, then move it or log why not."""
        nonlocal line
        current = str(read_fields(text).get("stage", ""))
        waits = list(unmerged)
        criteria = read_section(text, CRITERIA) or ""
        if unticked := len(UNTICKED_RE.findall(criteria)):
            waits.append(f"{unticked} unticked in {CRITERIA}")
        chain: list[str] = []
        if not waits:
            try:
                chain = plan_moves(
                    text,
                    card.key,
                    current,
                    DONE,
                    True,
                    projects,
                )
            except CardError as error:
                waits.append(str(error))
        if waits:
            line = f"{card.key} stays {current}: {', '.join(waits)}"
            note = (
                f"note (card merged): {merged} merged, but the card stays"
                f" {current}: {', '.join(waits)}"
            )
            # A rerun for the same merge must not add the same note again.
            if (read_section(text, "Log") or "").rstrip().endswith(note):
                return text
            return add_to_section(text, "Log", f"- {today} {note}")
        for stage in chain:
            why = f"{merged} merged" if stage == DONE else None
            entry = format_log_entry(stage, author, True, why)
            text = add_to_section(text, "Log", f"- {today} {entry}")
        line = f"{card.key} {' -> '.join([current, *chain])}"
        text = set_field(text, "stage", DONE)
        return set_field(text, "stage_date", today)

    update_card(card, change)
    return line


def normalize_pr_url(url: str) -> str:
    """Return a PR URL in one form, so two spellings of it match.

    ``http://GitHub.com/Me/Repo/pull/7/files#top`` becomes
    ``https://github.com/me/repo/pull/7``. Any other URL only loses its
    ``#...`` part and a trailing slash.

    Args:
        url: A PR URL as given or as stored in ``prs``.

    Returns:
        The normalized URL.
    """
    url = url.strip()
    if match := PR_URL_RE.match(url):
        owner, repo, number = match.groups()
        return f"https://github.com/{owner}/{repo}/pull/{number}".lower()
    return url.split("#")[0].rstrip("/")


def list_card_prs(text: str) -> list[str]:
    """Return the PR URLs in a card's ``prs`` field, normalized, each once.

    Args:
        text: The card text.

    Returns:
        The URLs in field order.
    """
    urls = as_list(read_fields(text).get("prs"))
    return list(dict.fromkeys(normalize_pr_url(url) for url in urls))


def read_pr_state(url: str) -> str:
    """Ask gh for a pull request's state.

    Args:
        url: The pull request URL.

    Returns:
        The state, such as MERGED, OPEN or CLOSED.

    Raises:
        CardError: gh failed or took longer than 5 seconds.
    """
    command = ["gh", "pr", "view", url, "--json", "state", "--jq", ".state"]
    try:
        proc = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise CardError(f"{' '.join(command)} failed: {error}") from error
    if proc.returncode != 0:
        raise CardError(f"{' '.join(command)} failed: {proc.stderr.strip()}")
    return proc.stdout.strip()


def run_note(args: argparse.Namespace, projects: Path) -> int:
    """Add a dated note to a card's Log.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    card = find_card(projects, args.key)
    today = read_today().isoformat()
    entry = f"- {today} note ({find_author(args)}): {args.text}"
    update_card(card, lambda text: add_to_section(text, "Log", entry))
    print(f"{card.key} Log: {entry}")
    return 0


def run_add(args: argparse.Namespace, projects: Path) -> int:
    """Add a bullet, or a checkbox in Acceptance Criteria, to a section.

    An Open Questions item gets the next Q number and today's date,
    such as ``- Q4 asked 2026-11-25: Deploy tonight?``.

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

    def change(text: str) -> str:
        """Add the entry, numbering Open Questions items first."""
        nonlocal entry
        if args.section == OPEN_QUESTIONS:
            text = stamp_question_numbers(text)
            number = find_highest_question_number(text) + 1
            today = read_today().isoformat()
            entry = f"- Q{number} asked {today}: {args.text}"
        return add_to_section(text, args.section, entry)

    update_card(card, change)
    print(f"{card.key} {args.section}: {entry}")
    return 0


def find_highest_question_number(text: str) -> int:
    """Return the card's highest Q number, open or answered.

    ``hub answer`` keeps the ``Q3 asked 2026-11-25:`` head on the
    Decisions item it writes, so a number is never given twice and a
    late reply key cannot name a newer question.

    Args:
        text: The card text.

    Returns:
        The highest number, or 0 when the card has none.
    """
    numbers = [item["number"] for item in list_questions(text)]
    highest_open = max(numbers, default=0)
    return max(highest_open, find_highest_answered_number(text.split("\n")))


def find_highest_answered_number(lines: list[str]) -> int:
    """Return the highest Q number at the head of a Decisions item, or 0.

    Args:
        lines: The card text, split into lines.

    Returns:
        The highest number, such as 3 for ``- Q3 asked 2026-11-25: a``.
    """
    numbers = [0]
    for start, _ in list_items(lines, DECISIONS):
        match = QUESTION_RE.match(split_item(lines[start])[1])
        if match:
            numbers.append(int(match[1]))
    return max(numbers)


def stamp_question_numbers(text: str) -> str:
    """Write each unnumbered Open Questions item's shown Q number into it.

    A later add or removal then cannot shift that number: ``- b`` after
    ``- Q3: a`` becomes ``- Q4: b``.

    Args:
        text: The card text.

    Returns:
        The new card text.
    """
    lines = text.split("\n")
    items = list_items(lines, OPEN_QUESTIONS)
    for (start, _), question in zip(items, list_questions(text)):
        prefix, words = split_item(lines[start])
        if not QUESTION_RE.match(words):
            lines[start] = f"{prefix or '- '}Q{question['number']}: {words}"
    return "\n".join(lines)


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
            " characters Obsidian refuses are removed",
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
                " rename the file and try again",
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
    """Replace, remove, tick or untick one numbered item, and log it.

    A replaced item keeps its bullet or checkbox, with its tick. Tick
    and untick keep the words, and refuse an item with no checkbox or a
    box already in that state. In Open Questions, unnumbered items get
    their Q numbers first, and a replaced item keeps its ``Q<n>`` head.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code.
    """
    changes = [args.text is not None, args.remove, args.tick, args.untick]
    if changes.count(True) != 1:
        raise CardError("give the new text, --remove, --tick or --untick")
    if args.text is not None and not split_item(args.text.strip())[1]:
        raise CardError("the new text is empty; use --remove")
    card = find_card(projects, args.key)
    today = read_today().isoformat()
    author = find_author(args)
    questions = args.section == OPEN_QUESTIONS
    text = stamp_question_numbers(card.text) if questions else card.text
    lines = text.split("\n")
    number = find_item_position(text, card.key, args.section, args.number)
    old = lines[find_item(lines, card.key, args.section, number)[0]]
    prefix, old_words = split_item(old)
    label = f"{args.section} item {args.number}"
    if args.tick or args.untick:
        box = "- [x] " if args.tick else "- [ ] "
        verb = "ticked" if args.tick else "unticked"
        if not prefix.startswith("- ["):
            raise CardError(f"{card.key} {label} has no checkbox")
        if prefix.lower() == box:
            raise CardError(f"{card.key} {label} is already {verb}")
        note = f"{verb} {label}, {json.dumps(old_words)}"
    elif args.remove:
        note = f"removed {label}, {json.dumps(old_words)}"
    else:
        note = f"replaced {label}, was {json.dumps(old_words)}"
    entry = f"- {today} note ({author}): {note}"

    def change(text: str) -> str:
        """Edit the item, unless someone changed it since card read it."""
        if questions:
            text = stamp_question_numbers(text)
        lines = text.split("\n")
        number = find_item_position(text, card.key, args.section, args.number)
        start, stop = find_item(lines, card.key, args.section, number)
        if lines[start] != old:
            raise CardError(f"{card.key} {label} changed; run card show again")
        if args.remove:
            del lines[start:stop]
            # Keep one blank line where the item was.
            if 0 < start < len(lines) and not (
                lines[start - 1].strip() or lines[start].strip()
            ):
                del lines[start]
        elif args.tick or args.untick:
            lines[start] = box + old_words
        else:
            # The item keeps its own bullet or box, so a given one is
            # used only on a plain line.
            given_prefix, words = split_item(args.text.strip())
            if questions:
                words = keep_question_head(old_words, words)
            lines[start] = (prefix or given_prefix) + words
        return add_to_section("\n".join(lines), "Log", entry)

    update_card(card, change)
    print(f"{card.key} Log: {entry}")
    return 0


def find_item_position(text: str, key: str, section: str, given: str) -> int:
    """Return the position, from 1, of the item that N names.

    Args:
        text: The card text.
        key: The card key, for the error.
        section: The section name.
        given: N as typed, such as ``2``, or ``Q4`` in Open Questions.

    Returns:
        The item's position, which find_item checks.

    Raises:
        CardError: N is neither form, or no item has that Q number.
    """
    if given.isdigit():
        return int(given)
    match = re.fullmatch(r"[Qq](\d+)", given)
    if section != OPEN_QUESTIONS or match is None:
        raise CardError("N is an item number, or Q<n> in Open Questions")
    numbers = [item["number"] for item in list_questions(text)]
    if int(match[1]) not in numbers:
        raise CardError(f"{key} {section} has no Q{match[1]}")
    return numbers.index(int(match[1])) + 1


def keep_question_head(old_words: str, new_words: str) -> str:
    """Return the new words behind the old item's ``Q<n> asked DATE:`` head.

    New words that bring their own head are kept as given.

    Args:
        old_words: The item's words before the edit.
        new_words: The words given to card edit.

    Returns:
        The item's new words.
    """
    head = QUESTION_RE.match(old_words)
    if head is None or QUESTION_RE.match(new_words):
        return new_words
    return old_words[: head.start(3)] + new_words


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
    lines: list[str],
    key: str,
    section: str,
    number: int,
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
            f"{key} {section} has {len(items)} items, so no item {number}",
        )
    return items[number - 1]


def run_set(args: argparse.Namespace, projects: Path) -> int:
    """Change a card's kind, or its title and file name.

    A title with a check_title finding is refused, as in run_new.

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
    require_passing_title(args.value.strip())
    name = BAD_NAME_CHARS_RE.sub("", args.value).strip()
    if not name:
        raise CardError("the title needs a letter or digit")
    new_path = card.path.with_name(
        make_name(find_rule("card_file"), {"key": card.key, "title": name}),
    )
    if new_path != card.path and new_path.exists():
        raise CardError(f"{new_path.name} already exists")
    update_card(
        card,
        lambda text: set_title(text, card.key, args.value.strip()),
    )
    if new_path != card.path:
        os.rename(card.path, new_path)
    print(new_path.stem)
    return 0


def run_link(args: argparse.Namespace, projects: Path) -> int:
    """Add a plan link to ``plans``, a PR URL to ``prs``, or a session.

    A session only another card holds is refused, since the hub maps
    each session to one card.

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
        # The hub relinks a worker's own card when it parks the worker, so
        # a session this card holds passes even if another card has it.
        held = find_session_holder([card], session.entry) is not None
        cards = [] if held else load_all_cards(projects)
        holder = find_session_holder(cards, session.entry)
        if holder is not None:
            raise CardError(
                f"session ID already on {holder.key}: {session.entry}."
                f" To log work on {card.key}, run card note {card.key} TEXT.",
            )
        today = read_today().isoformat()
        written = update_card(
            card,
            lambda text: add_session(text, today, session),
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


def find_session_holder(cards: list[Card], entry: str) -> Card | None:
    """Return the first card whose sessions field holds the entry.

    Args:
        cards: The cards to search.
        entry: A sessions entry, such as ``opencode ses_abc``.

    Returns:
        The card, or None when no card holds the entry.
    """
    for card in cards:
        if entry in as_list(read_fields(card.text).get("sessions")):
            return card
    return None


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


def run_rekey(args: argparse.Namespace, projects: Path) -> int:
    """Give open cards with old keys new keys, or print what would change.

    Each open card with a key such as PDEV-KB-012 takes a new key such
    as PDEV-523, keeps the old one in ``old_key``, and links to it under
    Projects follow. --apply renames every card first, then points links
    at each card's name by its ``old_key``, so a stop never leaves a link
    to a key not yet given out. It backs up each note before writing it,
    and its undo.sh restores the notes it wrote; after two runs, run the
    newer undo.sh first. A second run after a stop finishes the job.

    Args:
        args: The parsed arguments.
        projects: The vault's Projects folder.

    Returns:
        The exit code, 1 when a write stops partway.

    Raises:
        CardError: --apply lacks --user, a card was written but not
            renamed, two cards hold one key, Herdr cannot list its agents
            or one works on a card to rekey, a new name is taken, or the
            backup folder cannot be made.
    """
    if args.apply and not args.user:
        raise CardError("--apply needs --user; the user runs it after review")
    # card new must not take a number while the new keys are handed out.
    with hold_lock(STATE_DIR / "new.lock"):
        cards = load_all_cards(projects)
        old_keys = [read_fields(card.text).get("old_key") for card in cards]
        stuck = [
            card.key for card, old in zip(cards, old_keys) if old == card.key
        ]
        if stuck:
            raise CardError(
                f"{', '.join(stuck)} written but not renamed; run the last"
                " undo line, then card rekey --apply --user again",
            )
        # A card made from an old template can take a rekeyed card's old key.
        counts = Counter(card.key for card in cards)
        counts.update(old for old in old_keys if isinstance(old, str))
        if shared := sorted(key for key, count in counts.items() if count > 1):
            raise CardError(
                f"two cards hold {', '.join(shared)} as key or old_key",
            )
        new_keys = number_open_cards(projects, cards)
        if workers := find_live_workers(
            [card for card in cards if card.key in new_keys],
        ):
            raise CardError(
                f"live workers on {', '.join(workers)}; wait until they end",
            )
        renames = list_card_renames(cards, new_keys)
        relinks = list_link_changes(projects, cards, new_keys)
        for old_key, new_key in new_keys.items():
            print(f"{old_key} {new_key}")
        for change in relinks:
            print(f"relink {change.path.relative_to(projects)}")
        if not args.apply or not (renames or relinks):
            print(
                f"{len(renames)} cards to rekey,"
                f" {len(relinks)} notes to relink",
            )
            return 0
        try:
            backup = Path(tempfile.mkdtemp(prefix="rekey-", dir=STATE_DIR))
        except OSError as error:
            raise CardError(f"{error}, no note changed") from error
        print(f"backup: {backup}")
        written: list[NoteChange] = []
        try:
            finished = write_rekey_changes(projects, backup, renames, written)
            if finished:
                # Read again, so each link follows its card's name by old_key.
                relinks = list_link_changes(
                    projects,
                    load_all_cards(projects),
                    {},
                )
                finished = write_rekey_changes(
                    projects,
                    backup,
                    relinks,
                    written,
                )
            if finished:
                print(
                    f"rekeyed {len(renames)} cards,"
                    f" relinked {len(relinks)} notes",
                )
        finally:
            # A stop or Ctrl-C leaves the later notes alone, so undo puts
            # back only the written ones and keeps any edit to the rest.
            undo = write_undo_script(projects, backup, written)
            print(f"undo (newest first): sh {shlex.quote(str(undo))}")
    return 0 if finished else 1


def number_open_cards(projects: Path, cards: list[Card]) -> dict[str, str]:
    """Give each open card with an old key the next free key of its code.

    Done and dropped cards keep their keys. A code's cards take numbers
    in the order of their old numbers, after the highest card or plan
    number of the code.

    Args:
        projects: The vault's Projects folder.
        cards: Every card.

    Returns:
        Each old key's new key, such as PDEV-KB-012 to PDEV-523.
    """
    key_rule = find_rule("card_key")
    by_code: dict[str, list[tuple[int, str]]] = {}
    for card in cards:
        if card.stage in (DONE, DROPPED) or "-KB-" not in card.key:
            continue
        fields = parse_name(key_rule, card.key)
        assert fields is not None, "card_file and card_key read one shape"
        numbered = by_code.setdefault(fields["code"], [])
        numbered.append((int(fields["n"]), card.key))
    new_keys = {}
    for code, numbered in sorted(by_code.items()):
        first = find_next_number(projects, code)
        for offset, (_, key) in enumerate(sorted(numbered)):
            values = {"code": code, "n": str(first + offset)}
            new_keys[key] = make_name(key_rule, values)
    return new_keys


def find_live_workers(cards: list[Card]) -> list[str]:
    """List the cards a running Herdr agent works on, as ``KEY (agent)``.

    An agent works on a card when its name holds the card's key, such
    as pdev-kb-012-build, or when its session is in the card's sessions.

    Args:
        cards: The cards to check.

    Returns:
        One entry per card and agent.

    Raises:
        CardError: Herdr gave no agent list.
    """
    try:
        proc = subprocess.run(
            ["herdr", "agent", "list"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        running = json.loads(proc.stdout)["result"]["agents"]
    except (OSError, subprocess.TimeoutExpired, ValueError, KeyError) as error:
        raise CardError(
            f"herdr agent list failed ({error}), so live workers are unknown",
        ) from error
    agent_rule = find_rule("agent")
    workers = []
    for agent in running:
        name = agent.get("name") or ""
        fields = parse_name(agent_rule, name)
        session = (agent.get("agent_session") or {}).get("value")
        entry = f"{agent.get('agent')} {session}"
        for card in cards:
            by_name = fields is not None and fields["key"].upper() == card.key
            sessions = as_list(read_fields(card.text).get("sessions"))
            if by_name or (session and entry in sessions):
                workers.append(f"{card.key} ({name or entry})")
    return workers


def list_card_renames(
    cards: list[Card],
    new_keys: dict[str, str],
) -> list[NoteChange]:
    """Work out each rekeyed card's new path, ``old_key`` and heading.

    Its links stay as they are until every card has its new name. Nothing
    is written.

    Args:
        cards: Every card.
        new_keys: Each old key's new key.

    Returns:
        One change per rekeyed card.

    Raises:
        CardError: A file already has a rekeyed card's new name.
    """
    changes = []
    for card in cards:
        if card.key not in new_keys:
            continue
        new_key = new_keys[card.key]
        new_path = card.path.with_name(
            new_key + card.path.name[len(card.key) :],
        )
        if new_path.exists():
            raise CardError(f"{new_path.name} already exists")
        mtime_ns = card.path.stat().st_mtime_ns
        text = card.path.read_text(encoding="utf-8")
        text = set_field(text, "old_key", card.key)
        text = set_title(text, new_key, card.title)
        changes.append(NoteChange(card.path, new_path, text, mtime_ns))
    return changes


def list_link_changes(
    projects: Path,
    cards: list[Card],
    new_keys: dict[str, str],
) -> list[NoteChange]:
    """Work out each note whose links change, and write nothing.

    In every note under Projects, a link to an open card's old file name
    or ``old_key`` follows the card's name, keeping any #heading or
    |alias. Links to the cards in new_keys follow their new names, as the
    dry run shows them.

    Args:
        projects: The vault's Projects folder.
        cards: Every card.
        new_keys: Each old key's new key, empty once cards are renamed.

    Returns:
        The notes whose text changes.
    """
    targets = {}
    for card in cards:
        title = card.path.stem[len(card.key) :]
        old_key = read_fields(card.text).get("old_key")
        if isinstance(old_key, str) and card.stage not in (DONE, DROPPED):
            targets[old_key + title] = card.path.stem
            targets[old_key] = card.key
        if card.key in new_keys:
            targets[card.path.stem] = new_keys[card.key] + title
            targets[card.key] = new_keys[card.key]
    changes = []
    for path in sorted(projects.rglob("*.md")):
        mtime_ns = path.stat().st_mtime_ns
        text = path.read_text(encoding="utf-8")
        new_text = LINK_TARGET_RE.sub(
            lambda link: f"[[{targets.get(link[1], link[1])}",
            text,
        )
        if new_text != text:
            changes.append(NoteChange(path, path, new_text, mtime_ns))
    return changes


def write_rekey_changes(
    projects: Path,
    backup: Path,
    changes: list[NoteChange],
    written: list[NoteChange],
) -> bool:
    """Back up, write and rename each note in order, until one fails.

    Args:
        projects: The vault's Projects folder.
        backup: The folder for the copies, which keep their paths under
            Projects.
        changes: The notes to change.
        written: Gains each note once its new text is written, before
            its rename.

    Returns:
        True when every note changed. False when it stopped, after
        saying on stderr what to do next.
    """
    for change in changes:
        try:
            # Note names such as AGENTS.md repeat across project folders.
            copy = backup / change.path.relative_to(projects)
            copy.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(change.path, copy)
            wrote = write_safely(
                change.path,
                change.path.stem,
                change.new_text,
                change.mtime_ns,
            )
        except OSError as error:
            print(f"card: {error}", file=sys.stderr)
            wrote = False
        if not wrote:
            print(
                f"card: stopped at {change.path.name}; run card rekey"
                " --apply --user again to finish, or the undo line",
                file=sys.stderr,
            )
            return False
        written.append(change)
        if change.new_path == change.path:
            continue
        try:
            os.rename(change.path, change.new_path)
        except OSError as error:
            # The card has old_key now, so a second run refuses until undo.
            print(f"card: {error}", file=sys.stderr)
            print(
                f"card: stopped at {change.path.name}; run the undo line,"
                " then card rekey --apply --user again",
                file=sys.stderr,
            )
            return False
    return True


def write_undo_script(
    projects: Path,
    backup: Path,
    written: list[NoteChange],
) -> Path:
    """Write an undo.sh in the backup folder that restores the written notes.

    It puts notes back in the reverse order of their writes, since a
    renamed card is written again when its links change. For each note,
    undo.sh removes a renamed card's new file, then copies the old note
    back.

    Args:
        projects: The vault's Projects folder.
        backup: The folder holding the copies.
        written: The notes whose new text was written, in order.

    Returns:
        The path of undo.sh.
    """
    lines = ["#!/bin/sh", "set -e"]
    for change in reversed(written):
        if change.new_path != change.path:
            lines.append(f"rm -f -- {shlex.quote(str(change.new_path))}")
        copy = shlex.quote(str(backup / change.path.relative_to(projects)))
        lines.append(f"cp -p -- {copy} {shlex.quote(str(change.path))}")
    undo = backup / "undo.sh"
    undo.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return undo


def run_check(args: argparse.Namespace, projects: Path) -> int:
    """Print cards whose layout or fields card cannot use.

    It also prints notes named like old -KB- cards that are not cards,
    and new-named notes with a card's stage field but no type.

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
    # No plan has an old -KB- name, so such a note without the card type
    # is a broken card or a misnamed note, which every other command skips.
    card_paths = {card.path for card in cards}
    for path in sorted(projects.glob("*/*.md")):
        if path in card_paths or not read_card_key(path.name):
            continue
        if "-KB-" in path.name or is_card_without_type(path):
            problems.append(f"{path.name} has a card name but no type: card")
    # The hub maps a session to one card, so a session on two cards
    # shows one worker's state on the wrong card.
    holders: dict[str, str] = {}
    for card in cards:
        for entry in as_list(read_fields(card.text).get("sessions")):
            holder = holders.setdefault(entry, card.key)
            if holder != card.key:
                problems.append(
                    f"{card.key} session also on {holder}: {entry}",
                )
    if args.json:
        print_json({"cards": len(cards), "problems": problems})
        return 1 if problems else 0
    for problem in problems:
        print(fit(problem))
    print(f"{len(problems)} problems in {len(cards)} cards")
    return 1 if problems else 0


def find_card_issues(card: Card) -> list[str]:
    """List what is wrong with one card's fields, sections and title.

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
        missing = find_heading(lines, name) is None
        if missing and name not in VERSION_2_SECTIONS:
            issues.append(f"no {name} section")
    # Closed cards keep the titles they closed with.
    if card.stage not in (DONE, DROPPED):
        issues += [str(finding) for finding in check_title(card.title)]
    return issues


def require_passing_title(title: str) -> None:
    """Refuse a title that breaks a rule of check_title.

    Args:
        title: The title without the key.

    Raises:
        CardError: A rule fails, with every finding listed.
    """
    findings = check_title(title)
    if findings:
        lines = ["title refused:", *(f"  {finding}" for finding in findings)]
        lines.append(
            f"Write it as a commit title of at most {TITLE_LIMIT}"
            ' characters, such as "Retry failed uploads".',
        )
        raise CardError("\n".join(lines))


def check_title(title: str) -> list[TitleFinding]:
    """Run the title rules T1 to T4 and return each one the title breaks.

    A title reads like a commit title, with no label or key.

    Args:
        title: The title without the key.

    Returns:
        The findings in rule order, empty for a good title.
    """
    findings = []
    if len(title) > TITLE_LIMIT:
        detail = f"{len(title)} characters, at most {TITLE_LIMIT}"
        findings.append(TitleFinding("T1", "length", detail))
    words = title.split(" ")
    labels = [word for word in words[:3] if word.endswith(":")]
    if labels:
        findings.append(TitleFinding("T2", "label", f'"{labels[0]}"'))
    keys = find_title_keys(title)
    if keys:
        detail = ", ".join(f'"{key}"' for key in keys)
        findings.append(TitleFinding("T3", "key", detail))
    form = []
    if title[:1].islower():
        form.append("starts with a lower-case letter")
    if title.endswith("."):
        form.append("ends with a period")
    # A label's colon is a T2 finding only.
    body = " ".join(
        word.removesuffix(":") if index < 3 else word
        for index, word in enumerate(words)
    )
    refused = sorted(set(BAD_NAME_CHARS_RE.findall(body)))
    if refused:
        form.append(f"holds {' '.join(refused)}")
    if form:
        findings.append(TitleFinding("T4", "form", ", ".join(form)))
    return findings


def find_title_keys(title: str) -> list[str]:
    """Return the words of a title that read as a card or plan key.

    A key in a title goes stale when the card or plan is renamed. Case
    is ignored, so ``demo-kb-004`` counts, and so does a plan-shaped
    word such as ``ISO-8601``.

    Args:
        title: The title without the key.

    Returns:
        The key words, such as ``["DEMO-KB-004", "DEMO-41"]``.
    """
    card_key = re.compile(find_rule("card_key").parse.pattern, re.I)
    plan_file = re.compile(find_rule("plan_file").parse.pattern, re.I)
    keys = []
    for word in title.split():
        word = word.strip(".,:;!?()\"'")
        # plan_file reads a whole file name, so the word gets its suffix.
        if card_key.fullmatch(word) or plan_file.fullmatch(f"{word}.md"):
            keys.append(word)
    return keys


def run_name(args: argparse.Namespace) -> int:
    """Print the name a rule makes from VALUEs.

    The VALUEs fill the template's fields in the order they first
    appear, and the last field takes the rest, joined by spaces.

    Args:
        args: The parsed arguments.

    Returns:
        The exit code.

    Raises:
        CardError: Too few VALUEs were given, or the made name does not
            fit the rule.
    """
    rule = find_rule(args.rule)
    slots = SLOT_RE.finditer(rule.make)
    fields = list(dict.fromkeys(slot[1] for slot in slots))
    if len(args.values) < len(fields):
        wanted = " ".join(field.upper() for field in fields)
        raise CardError(f"card name {rule.name} needs {wanted}")
    values = dict(zip(fields, args.values))
    if fields:
        values[fields[-1]] = " ".join(args.values[len(fields) - 1 :])
    name = make_name(rule, values)
    # A bad VALUE, such as an unknown branch type, stops here instead of
    # naming a branch or agent.
    if parse_name(rule, name) is None:
        raise CardError(
            f"{name} does not fit the {rule.name} rule {rule.make}",
        )
    print(name)
    return 0


def render_card(
    key: str,
    title: str,
    project: str,
    kind: str,
    today: str,
    stage: str,
    log_entry: str,
    sections: dict[str, str] | None = None,
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
        sections: Text to put under some section headings, by name.
            None leaves every section empty.

    Returns:
        The note text.
    """
    values: dict[str, str | list[str]] = {
        "tags": ["#Kanban"],
        "type": "card",
        "stage": stage,
        "stage_date": today,
        "template_version": "2",
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
    sections = sections or {}
    for name in SECTIONS:
        lines += [f"## {name}", "---", ""]
        if name == "Log":
            lines += [f"- {today} {log_entry}", ""]
        elif name in sections:
            lines += [sections[name], ""]
    return "\n".join(lines).rstrip("\n") + "\n"


def render_template() -> str:
    """Return the Templater template that makes a card in Obsidian.

    It asks for the project, title and kind, picks the next
    number from the card and plan notes under Projects and moves the
    note into the project folder. It takes no lock, so ``card check``
    finds a clash.

    Returns:
        The template text.
    """
    key_rule = find_rule("card_key")
    file_rule = find_rule("card_file")
    constants = {
        "KINDS_JSON": KINDS,
        "CARD_KEY_MAKE_JSON": key_rule.make,
        "CARD_FILE_MAKE_JSON": file_rule.make,
        "CARD_FILE_RE_JSON": format_js_pattern(file_rule),
        "PLAN_FILE_RE_JSON": format_js_pattern(find_rule("plan_file")),
    }
    script = TEMPLATER_SCRIPT
    for placeholder, value in constants.items():
        script = script.replace(placeholder, json.dumps(value))
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


def format_js_pattern(rule: Rule) -> str:
    """Return the rule's parse pattern in JavaScript's named-group form.

    Args:
        rule: The rule.

    Returns:
        The pattern, with each ``(?P<name>`` written ``(?<name>``.
    """
    return rule.parse.pattern.replace("(?P<", "(?<")


# Runs inside Obsidian's Templater plugin. Keep its name rule the same
# as BAD_NAME_CHARS_RE, fill() the same as SLOT_RE and FILTERS, and
# its number rule the same as find_next_number.
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
const fill = (template, values) => template.replace(
  /\{([a-z_]+)(?:\|([a-z0-9]+))?\}/g, (slot, field, filter) => {
    const value = String(values[field]);
    if (filter === "lower") return value.toLowerCase();
    if (filter === "slug") {
      return (value.toLowerCase().match(/[a-z0-9]+/g) || []).join("-");
    }
    if (filter === "pad3") return value.padStart(3, "0");
    return value;
  });
const cardFileRe = new RegExp(CARD_FILE_RE_JSON);
const planFileRe = new RegExp(PLAN_FILE_RE_JSON);
let high = 0;
for (const file of app.vault.getMarkdownFiles()) {
  if (!file.path.startsWith(`${root}/`)) continue;
  const found = file.name.match(cardFileRe) || file.name.match(planFileRe);
  const key = found?.groups.key;
  if (key?.startsWith(`${project.code}-`)) {
    const digits = key.slice(project.code.length).replace(/[^0-9]/g, "");
    high = Math.max(high, Number(digits));
  }
}
const key = fill(CARD_KEY_MAKE_JSON, { code: project.code, n: high + 1 });
const today = tp.date.now("YYYY-MM-DD");
const name = title.replace(/[\\/:*?"<>|#^\[\]]/g, "").trim();
const fileName = fill(CARD_FILE_MAKE_JSON, { key, title: name });
await tp.file.move(`${project.folder.path}/${fileName.replace(/\.md$/, "")}`);
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
    lines: list[str],
    end: int,
    name: str,
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
    path: Path,
    key: str,
    text: str,
    expected_mtime_ns: int | None = None,
) -> bool:
    """Write a note through ``.<KEY>.tmp`` and a rename.

    Args:
        path: The note to write.
        key: The temporary file's name, such as the card key.
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


def read_card_note(path: Path) -> Card | None:
    """Read a note as a card, or return None when it is not one.

    A plan made for a card has the same name shape and key, so a note
    is a card only when its front matter says ``type: card``. Other
    notes are read only to the end of their front matter, since a plan
    can be long and the vault may sit on a network drive.

    Args:
        path: The note.

    Returns:
        The card, or None when the name holds no card key or the note
        is not a card.
    """
    key = read_card_key(path.name)
    if not key:
        return None
    mtime_ns = path.stat().st_mtime_ns
    with path.open(encoding="utf-8") as note:
        head = read_head(note)
        if read_fields(head).get("type") != "card":
            return None
        return Card(path, key, head + note.read(), mtime_ns)


def read_head(note: TextIO) -> str:
    """Read an open note's first line, and its front matter if it has one.

    Args:
        note: The note, open at its start.

    Returns:
        The text read, which leaves the note open after it.
    """
    head = note.readline()
    if head.strip() == "---":
        while line := note.readline():
            head += line
            if line.strip() == "---":
                break
    return head


def is_card_without_type(path: Path) -> bool:
    """Tell whether a note's front matter has a card's stage but no type.

    A plan's name can hold a card key too, but a plan has a type and no
    stage, and old plans have no front matter at all.

    Args:
        path: The note.

    Returns:
        True when the note looks like a card that lost its type line.
    """
    with path.open(encoding="utf-8") as note:
        fields = read_fields(read_head(note))
    return "type" not in fields and "stage" in fields


def load_card(path: Path) -> Card:
    """Read a card note from disk.

    Args:
        path: The note.

    Returns:
        The card.

    Raises:
        CardError: The note is not a card.
    """
    card = read_card_note(path)
    if not card:
        raise CardError(f"{path.name} is not a card note")
    return card


def load_all_cards(projects: Path) -> list[Card]:
    """Read every card note in every project folder.

    Args:
        projects: The vault's Projects folder.

    Returns:
        The cards.
    """
    notes = (read_card_note(path) for path in sorted(projects.glob("*/*.md")))
    return [card for card in notes if card]


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
    """Find the one card note that holds a key, or had it before a rekey.

    Args:
        projects: The vault's Projects folder.
        key: The card key, in any case.

    Returns:
        The card.

    Raises:
        CardError: No card note, or more than one, holds the key.
    """
    key = key.upper()
    paths = [
        path
        for path in projects.glob(f"*/{key}*.md")
        if read_card_key(path.name) == key
    ]
    cards = [card for path in paths if (card := read_card_note(path))]
    if not cards:
        # card rekey keeps a card's old key in old_key.
        cards = [
            card
            for card in load_all_cards(projects)
            if read_fields(card.text).get("old_key") == key
        ]
    if not cards:
        raise CardError(f"no card {key}")
    if len(cards) > 1:
        names = ", ".join(card.path.name for card in cards)
        raise CardError(f"two files hold {key}, fix by hand: {names}")
    return cards[0]


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
        if read_card_key(path.name) == key
    ]


def find_rule(name: str) -> Rule:
    """Return one naming rule from the rules file.

    Args:
        name: The rule name, such as card_key.

    Returns:
        The rule.

    Raises:
        CardError: No rule has the name, or the rules cannot be read.
    """
    path = find_rules_file()
    rules = read_rules_file(path)
    if name not in rules:
        raise CardError(f"{path} has no naming rule {name}")
    return rules[name]


def make_name(rule: Rule, values: dict[str, str]) -> str:
    """Fill the rule's template with values, running each field's filter.

    Args:
        rule: The rule.
        values: A value for each field the template names.

    Returns:
        The name, such as ``pdev-kb-012-build``.
    """

    def fill(slot: re.Match[str]) -> str:
        """Return one field's value, filtered."""
        value = values[slot[1]]
        return FILTERS[slot[2]](value) if slot[2] else value

    return SLOT_RE.sub(fill, rule.make)


def parse_name(rule: Rule, name: str) -> dict[str, str] | None:
    """Read a name's fields with the rule's parse pattern.

    Args:
        rule: The rule.
        name: The name, which the pattern must match whole.

    Returns:
        The fields, leaving out groups that matched nothing, or None
        when the pattern does not read the name.
    """
    match = rule.parse.fullmatch(name)
    if not match:
        return None
    groups = match.groupdict().items()
    return {field: text for field, text in groups if text is not None}


def read_card_key(file_name: str) -> str | None:
    """Return the card key a file name holds, under the card_file rule.

    Args:
        file_name: A file name, such as ``PDEV-KB-012 Retry uploads.md``.

    Returns:
        The key, or None when the name holds no card key. A plan's
        name can hold one too, so read_card_note also checks the type.
    """
    fields = parse_name(find_rule("card_file"), file_name)
    return fields.get("key") if fields else None


def find_rules_file() -> Path:
    """Return ``$PDE_NAMES_FILE``, else the checkout's copy, else RULES_FILE.

    ~/.local/bin/card links into its checkout, so the checkout's copy
    matches card's code between a git pull and the next install.

    Returns:
        The rules file.

    Raises:
        CardError: The file is missing, as when card runs outside a
            checkout before the installer has run.
    """
    shown = os.environ.get("PDE_NAMES_FILE")
    if not shown:
        found = CHECKOUT_RULES_FILE.is_file()
        shown = str(CHECKOUT_RULES_FILE) if found else RULES_FILE
    path = Path(shown).expanduser()
    if not path.is_file():
        raise CardError(
            f"no naming rules at {shown}: run pde-installer install",
        )
    return path


@functools.cache
def read_rules_file(path: Path) -> dict[str, Rule]:
    """Read the rules file.

    card reads a rule for every note it lists, so each process reads
    the file once.

    Args:
        path: The rules file.

    Returns:
        The rules, by name, in the file's order.

    Raises:
        CardError: The file cannot be read, is not TOML, has another
            version, or holds a rule card cannot use.
    """
    try:
        tables = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CardError(f"cannot read {path}: {error}") from error
    version = tables.pop("version", "unset")
    if version != RULES_VERSION:
        raise CardError(
            f"{path} is version {version}; this card reads version"
            f" {RULES_VERSION}",
        )
    for name, table in tables.items():
        if not isinstance(table, dict):
            raise CardError(f"{path}: {name} is not a [{name}] table")
    return {
        name: build_rule(path, name, table) for name, table in tables.items()
    }


def build_rule(path: Path, name: str, table: dict[str, Any]) -> Rule:
    """Make a Rule from its table in the rules file.

    Args:
        path: The rules file, for messages.
        name: The rule name.
        table: The table, with make and parse.

    Returns:
        The rule.

    Raises:
        CardError: The table lacks make or parse, the pattern does not
            compile, or the template names an unknown filter.
    """
    make = table.get("make")
    parse = table.get("parse")
    if not isinstance(make, str) or not isinstance(parse, str):
        raise CardError(f"{path}: rule {name} needs make and parse")
    for _field, filter_name in SLOT_RE.findall(make):
        if filter_name and filter_name not in FILTERS:
            raise CardError(
                f"{path}: rule {name} has unknown filter {filter_name};"
                f" filters are {', '.join(FILTERS)}",
            )
    try:
        pattern = re.compile(parse)
    except re.error as error:
        raise CardError(
            f"{path}: rule {name} has a bad pattern {parse!r}: {error}",
        ) from error
    return Rule(name, make, pattern)


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
            " Ask the user for one.",
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
            command,
            capture_output=True,
            text=True,
            timeout=20,
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
            f"herdr agent list failed ({error}), so no author. {hint}",
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
            command,
            capture_output=True,
            text=True,
            timeout=10,
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
            f" {(proc.stderr or proc.stdout).strip()}",
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
