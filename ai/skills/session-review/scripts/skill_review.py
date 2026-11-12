"""Find skills due for a skill-lesson review and check the review's inputs.

A skill is due when two or more top-level sessions loaded it since its
last review. The script reads the OpenCode database read-only and keeps
review times in ``~/.local/state/pde/skill-reviews.json``. Read
``count_skill_sessions`` first.

Example:
    python3 skill_review.py due
    python3 skill_review.py corrections create-plan > corrections.txt
    python3 skill_review.py check create-plan candidates.txt
    python3 skill_review.py mark create-plan
"""

import argparse
import json
import re
import sqlite3
import sys
import time
from contextlib import closing
from pathlib import Path

from session_stats import (
    DB,
    find_skill_loads,
    join_tool_output,
    list_tool_calls,
)

STATE = Path.home() / ".local/state/pde/skill-reviews.json"
DUE_AT = 2
MESSAGE_CHARS = 200
CANDIDATE = re.compile(r"^(\S+) (\d+): \S")


def count_skill_sessions(
    db: sqlite3.Connection, state: dict
) -> dict[str, list[str]]:
    """Return the top-level sessions that loaded each skill since its review.

    Args:
        db: Read-only connection to the OpenCode database.
        state: Review times from ``load_state``.

    Returns:
        Each skill's session IDs, oldest first.
    """
    # Skills never reviewed count from ``started``, the earliest cutoff.
    rows = db.execute(
        "select s.id, s.time_created, m.data from session_v2 s"
        " join session_message m on m.session_id = s.id"
        " where s.parent_id is null and s.time_created > ?"
        " and m.type = 'assistant'"
        " and (m.data like '%skill_content name=%' or m.data like '%name: %')"
        " order by s.time_created, m.seq",
        (state["started"],),
    )
    sessions: dict[str, list[str]] = {}
    for session, created, data in rows:
        for call in list_tool_calls(json.loads(data)):
            for skill in find_skill_loads(join_tool_output(call)):
                cutoff = state["reviewed"].get(skill, state["started"])
                found = sessions.setdefault(skill, [])
                if created > cutoff and session not in found:
                    found.append(session)
    return {skill: found for skill, found in sessions.items() if found}


def list_corrections(db: sqlite3.Connection, sessions: list[str]) -> list[str]:
    """Return each user message of the sessions as one short line.

    Args:
        db: Read-only connection to the OpenCode database.
        sessions: Session IDs from ``count_skill_sessions``.

    Returns:
        Lines such as ``ses_abc 12: no, use rg``, cut to 200
        characters of message text.
    """
    lines = []
    for session in sessions:
        for seq, data in db.execute(
            "select seq, data from session_message"
            " where session_id = ? and type = 'user' order by seq",
            (session,),
        ):
            text = " ".join((json.loads(data).get("text") or "").split())
            lines.append(f"{session} {seq}: {text[:MESSAGE_CHARS]}")
    return lines


def find_bad_candidates(text: str, corrections: list[str]) -> list[str]:
    """Return the candidate lines that cite no printed correction.

    A file holding only ``none`` passes, and so does one where every
    line is ``<session> <seq>: <rule>`` for a session and seq that
    ``list_corrections`` printed.

    Args:
        text: The candidate file a model wrote.
        corrections: Lines from ``list_corrections``.

    Returns:
        Each bad line, or ``(empty file)`` when there is no line.
    """
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    if lines == ["none"]:
        return []
    if not lines:
        return ["(empty file)"]
    printed = {tuple(line.split(":", 1)[0].split(" ")) for line in corrections}
    bad = []
    for line in lines:
        match = CANDIDATE.match(line)
        if not match or match.groups() not in printed:
            bad.append(line)
    return bad


def load_state() -> dict:
    """Return the review times, writing a new file when there is none.

    A new file starts counting now, so no skill is due on the first run.

    Returns:
        ``{"started": <ms>, "reviewed": {<skill>: <ms>}}``, in epoch
        milliseconds like OpenCode's ``time_created``.
    """
    if STATE.is_file():
        return json.loads(STATE.read_text())
    state = {"started": read_clock_ms(), "reviewed": {}}
    save_state(state)
    return state


def save_state(state: dict) -> None:
    """Write the review times to the state file."""
    STATE.parent.mkdir(parents=True, exist_ok=True)
    STATE.write_text(json.dumps(state, indent=2) + "\n")


def read_clock_ms() -> int:
    """Return the current time in epoch milliseconds."""
    return int(time.time() * 1000)


def main() -> None:
    """Run one command, or exit non-zero with a reason on stderr."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("due", help="list skills due for a review")
    for name, text in (
        ("corrections", "print user messages of the counted sessions"),
        ("mark", "record now as the skill's last review"),
    ):
        commands.add_parser(name, help=text).add_argument("skill")
    check = commands.add_parser("check", help="check a candidate file")
    check.add_argument("skill")
    check.add_argument("file", type=Path)
    args = parser.parse_args()
    state = load_state()
    if args.command == "mark":
        state["reviewed"][args.skill] = read_clock_ms()
        save_state(state)
        print(f"marked {args.skill}")
        return
    if not DB.is_file():
        sys.exit(f"no OpenCode database at {DB}")
    with closing(sqlite3.connect(f"file:{DB}?mode=ro", uri=True)) as db:
        sessions = count_skill_sessions(db, state)
        if args.command == "due":
            for skill, found in sorted(sessions.items()):
                if len(found) >= DUE_AT:
                    print(f"{skill} {len(found)}")
            return
        corrections = list_corrections(db, sessions.get(args.skill, []))
    if args.command == "corrections":
        if corrections:
            print("\n".join(corrections))
        return
    bad = find_bad_candidates(args.file.read_text(), corrections)
    if bad:
        sys.exit("lines that cite no printed message:\n" + "\n".join(bad))


if __name__ == "__main__":
    main()
