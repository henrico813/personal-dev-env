"""Print the numbers a session review starts from.

Reads one OpenCode session from the local database, read-only, and
prints about twenty plain lines. Read ``report_session`` first.

Example:
    python3 ~/.agents/skills/session-review/scripts/session_stats.py
    python3 session_stats.py --session ses_abc123
"""

import argparse
import json
import os
import re
import sqlite3
import sys
from collections import Counter
from contextlib import closing
from pathlib import Path

DB = Path.home() / ".local/share/opencode/opencode.db"
SKILLS = Path.home() / ".agents/skills"
# A successful read prints the skill's unindented frontmatter, maybe after a
# line number from the read tool or cat -n. Skill tool output starts with its
# tag, so a tag quoted inside other output does not count.
SKILL_LOAD = re.compile(
    r"^(?:\s*\d+(?::\s|\t))?name: ([\w-]+)\s*$"
    r'|^<skill_content name="([\w-]+)"',
    re.M,
)
REPEATS_SHOWN = 5
SUBAGENTS_SHOWN = 5


def report_session(db: sqlite3.Connection, session: str) -> list[str]:
    """Build the review lines for one session and its direct subagents.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID.

    Returns:
        The lines to print, or an empty list when the session is not
        in the database.
    """
    row = db.execute(
        "select title, model, tokens_input, tokens_output,"
        " tokens_reasoning, tokens_cache_read, tokens_cache_write"
        " from session_v2 where id = ?",
        (session,),
    ).fetchone()
    if row is None:
        return []
    title, model, uncached, output, reasoning, read, write = row
    # Message times, not the session row's, since the row can lag behind.
    (span,) = db.execute(
        "select max(time_created) - min(time_created) from session_message"
        " where session_id = ?",
        (session,),
    ).fetchone()
    messages = db.execute(
        "select type, seq, data from session_message"
        " where session_id = ? order by seq",
        (session,),
    ).fetchall()
    steps = [
        (seq, json.loads(data))
        for kind, seq, data in messages
        if kind == "assistant"
    ]
    users = sum(kind == "user" for kind, _, _ in messages)
    zero = sum(count_context_tokens(step) == 0 for _, step in steps)
    return [
        f"session {session} {title or ''}".rstrip(),
        f"model {format_model(model)}",
        f"minutes open {(span or 0) // 60000}",
        f"tokens read {uncached + read + write:,} (cache read {read:,},"
        f" cache write {write:,}, uncached {uncached:,})",
        f"tokens written {output + reasoning:,} (output {output:,},"
        f" reasoning {reasoning:,})",
        f"user messages {users}, model steps {len(steps)},"
        f" steps logged with zero tokens {zero}",
        describe_peak(steps),
        describe_compactions(messages),
        describe_skills(steps),
        *describe_subagents(db, session),
        *describe_repeats(steps),
    ]


def count_context_tokens(step: dict) -> int:
    """Return the tokens a model step read, or 0 when none were logged.

    Args:
        step: Data of one assistant message.

    Returns:
        Uncached input plus cache reads and writes.
    """
    tokens = step.get("tokens") or {}
    cache = tokens.get("cache") or {}
    return (
        (tokens.get("input") or 0)
        + (cache.get("read") or 0)
        + (cache.get("write") or 0)
    )


def describe_peak(steps: list[tuple[int, dict]]) -> str:
    """Describe the largest context one model step read.

    The claude-code provider logs some steps with zero tokens and adds
    them to the next step, so that next step can read twice the real
    context. Steps right after a zero step are left out.

    Args:
        steps: Sequence number and data of each assistant message.

    Returns:
        One line with the approximate peak and the step's sequence
        number.
    """
    peak, peak_seq, after_zero = 0, None, False
    for seq, step in steps:
        size = count_context_tokens(step)
        if size > peak and not after_zero:
            peak, peak_seq = size, seq
        after_zero = size == 0
    if peak_seq is None:
        return "peak context none"
    return f"peak context about {peak:,} tokens at seq {peak_seq}"


def describe_compactions(messages: list[tuple[str, int, str]]) -> str:
    """List each compaction with its sequence number and reason.

    Args:
        messages: Type, sequence number and data of each message.

    Returns:
        One line with the count and each compaction.
    """
    found = [
        f"seq {seq} {json.loads(data).get('reason') or 'unknown'}"
        for kind, seq, data in messages
        if kind == "compaction"
    ]
    return f"compactions {len(found)}" + (
        f" ({', '.join(found)})" if found else ""
    )


def find_skill_loads(output: str) -> list[str]:
    """Return the installed skills a tool's output shows were loaded.

    The skill tool's ``<skill_content name="create-plan">`` and a read
    of ``create-plan/SKILL.md`` both give ``["create-plan"]``.

    Args:
        output: Text a tool call returned.

    Returns:
        Skill names in the order they appear, installed skills only.
    """
    names = (match[1] or match[2] for match in SKILL_LOAD.finditer(output))
    return [name for name in names if (SKILLS / name / "SKILL.md").is_file()]


def describe_skills(steps: list[tuple[int, dict]]) -> str:
    """List each skill loaded and the step that first loaded it.

    Args:
        steps: Sequence number and data of each assistant message.

    Returns:
        One line naming each skill with its first sequence number.
    """
    first: dict[str, int] = {}
    for seq, step in steps:
        for call in list_tool_calls(step):
            output = "".join(
                part.get("text", "")
                for part in call["state"].get("content") or []
            )
            for skill in find_skill_loads(output):
                first.setdefault(skill, seq)
    loaded = ", ".join(f"{skill} (seq {seq})" for skill, seq in first.items())
    return f"skills loaded {loaded or 'none'}"


def describe_subagents(db: sqlite3.Connection, session: str) -> list[str]:
    """Sum direct subagents by model and list the largest few.

    Some sessions start hundreds of subagents, so the script prints one
    line per model and only the subagents that read the most tokens.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID of the parent.

    Returns:
        A count line, one line per model, then the largest subagents
        and how many more there are.
    """
    rows = db.execute(
        "select id, model, title, tokens_input + tokens_cache_read"
        " + tokens_cache_write, tokens_output + tokens_reasoning"
        " from session_v2 where parent_id = ? order by 4 desc, id",
        (session,),
    ).fetchall()
    if not rows:
        return ["subagents 0"]
    models: dict[str, list[int]] = {}
    for _, model, _, read, written in rows:
        totals = models.setdefault(format_model(model), [0, 0, 0])
        totals[0] += 1
        totals[1] += read
        totals[2] += written
    lines = [f"subagents {len(rows)}"]
    for model, (count, read, written) in sorted(
        models.items(), key=lambda item: -item[1][1]
    ):
        lines.append(
            f"  {model} count {count} read {read:,} written {written:,}"
        )
    lines.append("largest subagents by tokens read")
    for child, model, title, read, written in rows[:SUBAGENTS_SHOWN]:
        lines.append(
            f"  {child} {format_model(model)} read {read:,}"
            f" written {written:,} {(title or '')[:60]}"
        )
    if len(rows) > SUBAGENTS_SHOWN:
        lines.append(f"  {len(rows) - SUBAGENTS_SHOWN} more")
    return lines


def describe_repeats(steps: list[tuple[int, dict]]) -> list[str]:
    """List shell commands run more than once with the same full text.

    Args:
        steps: Sequence number and data of each assistant message.

    Returns:
        A count line, then the most repeated commands, first line only.
    """
    counts = Counter(
        command
        for _, step in steps
        for call in list_tool_calls(step)
        if isinstance(command := call["state"]["input"].get("command"), str)
    )
    repeats = [(n, cmd) for cmd, n in counts.most_common() if n > 1]
    shown = [
        f"  {n}x {cmd.splitlines()[0][:100]}"
        for n, cmd in repeats[:REPEATS_SHOWN]
    ]
    return [f"repeated commands {len(repeats)}", *shown]


def list_tool_calls(step: dict) -> list[dict]:
    """Return the tool calls in one assistant message.

    Args:
        step: Data of one assistant message.

    Returns:
        Each tool call, with ``state`` and its ``input`` always dicts.
    """
    calls = []
    for part in step.get("content") or []:
        if part.get("type") != "tool":
            continue
        state = dict(part.get("state") or {})
        state["input"] = state.get("input") or {}
        calls.append(part | {"state": state})
    return calls


def format_model(model: str | None) -> str:
    """Return a session's model as ``provider/model``.

    Args:
        model: The JSON the database stores, or None.

    Returns:
        Such as ``claude-code/claude-opus-5-5[1m]``, or ``unknown``.
    """
    data = json.loads(model or "{}")
    if not data.get("id"):
        return "unknown"
    return f"{data.get('providerID', '?')}/{data['id']}"


def main() -> None:
    """Print the review lines, or exit 1 with a reason on stderr."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--session",
        default=os.environ.get("OPENCODE_SESSION_ID"),
        help="session ID; defaults to $OPENCODE_SESSION_ID",
    )
    session = parser.parse_args().session
    if not session:
        sys.exit("no session ID; pass --session or run inside OpenCode")
    if not DB.is_file():
        sys.exit(f"no OpenCode database at {DB}")
    with closing(sqlite3.connect(f"file:{DB}?mode=ro", uri=True)) as db:
        lines = report_session(db, session)
    if not lines:
        sys.exit(f"session {session} is not in {DB}")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
