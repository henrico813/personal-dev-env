"""Print the numbers a session review starts from.

Reads one OpenCode session from the local database, read-only, and
prints about thirty plain lines. Read ``report_session`` first.
``--context`` prints one line instead, from ``describe_context``, which
the ``pde-context`` command runs. ``--usage`` adds the Claude plan line
and the subagent lines to it.

Example:
    python3 ~/.agents/skills/session-review/scripts/session_stats.py
    python3 session_stats.py --session ses_abc123
    python3 session_stats.py --context
    python3 session_stats.py --usage
"""

import argparse
import json
import os
import re
import sqlite3
import sys
import time
from collections import Counter
from contextlib import closing
from datetime import datetime
from pathlib import Path

DB = Path.home() / ".local/share/opencode/opencode.db"
SKILLS = Path.home() / ".agents/skills"
MODELS = Path.home() / ".cache/opencode/models.json"
# The OpenCode Claude plugin saves the latest plan limit event here.
PLAN_USAGE = Path(
    os.environ.get("OPENCODE_CLAUDE_RATE_LIMIT_STORE")
    or Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
    / "opencode-claude/rate-limit.json"
)
# Providers whose list prices are the makers' own API rates.
PRICE_MAKERS = ("anthropic", "openai")
API_RATES = ("input", "output", "cache_read", "cache_write")
PLAN_WINDOWS = {
    "five_hour": "5-hour",
    "seven_day": "weekly",
    "seven_day_opus": "weekly Opus",
    "seven_day_sonnet": "weekly Sonnet",
}
# Agents hand the rest of their work to a new session above this context.
HANDOFF_LEVEL = 200_000
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
CARRIED_SHOWN = 5


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
        *describe_carry(messages),
    ]


def describe_context(db: sqlite3.Connection, session: str) -> str | None:
    """Describe the session's latest context and its share of the window.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID.

    Returns:
        A line such as ``250k of 1M (25%), above the 200k handoff
        level``, or None when the session is not in the database.
    """
    row = db.execute(
        "select model from session_v2 where id = ?", (session,)
    ).fetchone()
    if row is None:
        return None
    steps = [
        (seq, json.loads(data))
        for seq, data in db.execute(
            "select seq, data from session_message"
            " where session_id = ? and type = 'assistant' order by seq",
            (session,),
        )
    ]
    sizes = list_trusted_sizes(steps)
    context = sizes[-1][1] if sizes else 0
    window = find_context_window(row[0])
    line = format_tokens(context)
    if window:
        share = round(context * 100 / window)
        line += f" of {format_tokens(window)} ({share}%)"
    else:
        line += ", window unknown"
    if context > HANDOFF_LEVEL:
        line += f", above the {format_tokens(HANDOFF_LEVEL)} handoff level"
    return line


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


def list_trusted_sizes(steps: list[tuple[int, dict]]) -> list[tuple[int, int]]:
    """Return the context of each step whose token count can be trusted.

    The claude-code provider logs some steps with zero tokens and adds
    them to the next step, so that next step can read twice the real
    context. Zero steps and the steps right after them are left out.

    Args:
        steps: Sequence number and data of each assistant message.

    Returns:
        Sequence number and context tokens of each step kept, in order.
    """
    sizes, after_zero = [], False
    for seq, step in steps:
        size = count_context_tokens(step)
        if size and not after_zero:
            sizes.append((seq, size))
        after_zero = size == 0
    return sizes


def describe_peak(steps: list[tuple[int, dict]]) -> str:
    """Describe the largest context one model step read.

    Args:
        steps: Sequence number and data of each assistant message.

    Returns:
        One line with the approximate peak and the step's sequence
        number.
    """
    sizes = list_trusted_sizes(steps)
    if not sizes:
        return "peak context none"
    peak_seq, peak = max(sizes, key=lambda item: item[1])
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
            for skill in find_skill_loads(join_tool_output(call)):
                first.setdefault(skill, seq)
    loaded = ", ".join(f"{skill} (seq {seq})" for skill, seq in first.items())
    return f"skills loaded {loaded or 'none'}"


def describe_subagents(db: sqlite3.Connection, session: str) -> list[str]:
    """Sum direct subagents by model and list the largest few.

    Some sessions start hundreds of subagents, so the script prints one
    line per model and only the subagents that read the most tokens.
    Each line adds an API price when OpenCode's model list has one.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID of the parent.

    Returns:
        A count line, one line per model, then the largest subagents
        and how many more there are.
    """
    rows = list_subagents(db, session)
    if not rows:
        return ["subagents 0"]
    lines = [f"subagents {len(rows)}"]
    for model, count, read, written, dollars in sum_by_model(rows):
        lines.append(
            f"  {model} count {count} read {read:,} written {written:,}"
            + format_dollars(dollars, " API price ")
        )
    lines.append("largest subagents by tokens read")
    for child, model, title, read, written, dollars in rows[:SUBAGENTS_SHOWN]:
        lines.append(
            f"  {child} {model} read {read:,} written {written:,}"
            f"{format_dollars(dollars, ' API price ')} {(title or '')[:60]}"
        )
    if len(rows) > SUBAGENTS_SHOWN:
        lines.append(f"  {len(rows) - SUBAGENTS_SHOWN} more")
    return lines


def describe_usage(db: sqlite3.Connection, session: str) -> list[str]:
    """Sum subagents by model with their API price, in lines a phone fits.

    The lines hold what ``describe_subagents`` holds, in short numbers
    such as ``8.42M``. Each large subagent gets its title on one line
    and its model, tokens and price on the next.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID of the parent.

    Returns:
        A count line, one line per model, then two lines for each of
        the largest subagents and how many more there are.
    """
    rows = list_subagents(db, session)
    if not rows:
        return ["subagents 0"]
    lines = [f"subagents {len(rows)}, prices at API rates"]
    for model, count, read, written, dollars in sum_by_model(rows):
        lines.append(
            f"  {model} count {count} read {format_tokens(read)}"
            f" wrote {format_tokens(written)}{format_dollars(dollars, ' ')}"
        )
    lines.append("largest subagents")
    for child, model, title, read, written, dollars in rows[:SUBAGENTS_SHOWN]:
        lines.append(f"  {(title or child)[:68]}")
        lines.append(
            f"    {model} read {format_tokens(read)}"
            f" wrote {format_tokens(written)}{format_dollars(dollars, ' ')}"
        )
    if len(rows) > SUBAGENTS_SHOWN:
        lines.append(f"  {len(rows) - SUBAGENTS_SHOWN} more")
    return lines


def list_subagents(
    db: sqlite3.Connection, session: str
) -> list[tuple[str, str, str | None, int, int, float | None]]:
    """Return each direct subagent with its tokens and API price.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID of the parent.

    Returns:
        ID, model as ``provider/model``, title, tokens read, tokens
        written, and dollars or None when the model has no price. The
        subagent that read the most comes first.
    """
    found = db.execute(
        "select id, model, title, tokens_input, tokens_output,"
        " tokens_reasoning, tokens_cache_read, tokens_cache_write"
        " from session_v2 where parent_id = ? order by"
        " tokens_input + tokens_cache_read + tokens_cache_write desc, id",
        (session,),
    ).fetchall()
    providers = load_model_list()
    rows = []
    for child, model, title, *tokens in found:
        uncached, output, reasoning, read, write = tokens
        price = find_api_price(providers, model)
        rows.append(
            (
                child,
                format_model(model),
                title,
                uncached + read + write,
                output + reasoning,
                count_api_dollars(price, *tokens) if price else None,
            )
        )
    return rows


def sum_by_model(
    rows: list[tuple[str, str, str | None, int, int, float | None]],
) -> list[tuple[str, int, int, int, float | None]]:
    """Sum subagents by model, the model that read the most first.

    Args:
        rows: Subagents from ``list_subagents``.

    Returns:
        Model, count, tokens read, tokens written, and dollars or None
        when the model has no price.
    """
    models: dict[str, list] = {}
    for _, model, _, read, written, dollars in rows:
        totals = models.setdefault(model, [0, 0, 0, None])
        totals[0] += 1
        totals[1] += read
        totals[2] += written
        if dollars is not None:
            totals[3] = (totals[3] or 0) + dollars
    return sorted(
        ((model, *totals) for model, totals in models.items()),
        key=lambda item: -item[2],
    )


def describe_plan_usage() -> str:
    """Describe the Claude plan usage the OpenCode Claude plugin saved last.

    The plugin saves only the window the latest Claude reply reported,
    such as the weekly one, so the line says when it was saved.

    Returns:
        A line such as ``claude plan weekly 96% used, resets Nov 18
        02:00, as of Nov 12 08:03``, or ``claude plan usage unknown``
        when the file is missing or unreadable.
    """
    try:
        saved = json.loads(PLAN_USAGE.read_text())
    except (OSError, ValueError):
        saved = None
    if not isinstance(saved, dict):
        return "claude plan usage unknown"
    kind = saved.get("rateLimitType") or "window unknown"
    line = f"claude plan {PLAN_WINDOWS.get(kind, kind)}"
    utilization = saved.get("utilization")
    limited_until = saved.get("limitedUntil") or 0
    # The plugin leaves an old limitedUntil behind after a limit ends.
    if saved.get("limited") and limited_until > time.time() * 1000:
        line += f", limit reached until {format_time(limited_until)}"
    elif isinstance(utilization, (int, float)):
        line += f" {round(utilization * 100)}% used"
    # The plugin keeps the last reset time when an event leaves it out.
    if (saved.get("resetsAt") or 0) > time.time() * 1000:
        line += f", resets {format_time(saved['resetsAt'])}"
    if saved.get("updatedAt"):
        line += f", as of {format_time(saved['updatedAt'])}"
    return line


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


def describe_carry(messages: list[tuple[str, int, str]]) -> list[str]:
    """Sum the carry of each tool and list the calls carried most.

    A call's carry is its tokens times the later model steps that read
    it again before the next compaction. Steps logged with zero tokens
    do not count. Tokens are characters divided by 4, which undercounts
    tool output, so carry is a low estimate.

    Args:
        messages: Type, sequence number and data of each message.

    Returns:
        A heading with one line per tool, most carry first, then a
        heading with the calls that have the most carry.
    """
    tools: dict[str, list[int]] = {}
    calls = []
    # Walk backwards so each step knows how many later steps read it.
    later = 0
    for kind, seq, data in reversed(messages):
        if kind == "compaction":
            later = 0
        if kind != "assistant":
            continue
        step = json.loads(data)
        for call in list_tool_calls(step):
            given = call["state"]["input"]
            tokens = (
                len(join_tool_output(call)) // 4 + len(json.dumps(given)) // 4
            )
            name = call.get("name") or "?"
            totals = tools.setdefault(name, [0, 0, 0])
            totals[0] += 1
            totals[1] += tokens
            totals[2] += tokens * later
            label = str(
                given.get("command")
                or given.get("path")
                or given.get("id")
                or ""
            ).partition("\n")[0][:100]
            calls.append((tokens * later, tokens, later, seq, name, label))
        later += count_context_tokens(step) > 0
    lines = ["carry by tool"]
    for name, (count, tokens, carry) in sorted(
        tools.items(), key=lambda item: (-item[1][2], item[0])
    ):
        lines.append(
            f"  {name} calls {count} tokens {tokens:,} carry {carry:,}"
        )
    lines.append("largest carried outputs")
    calls.sort(key=lambda call: (-call[0], call[3]))
    for carry, tokens, later, seq, name, label in calls[:CARRIED_SHOWN]:
        lines.append(
            f"  carry {carry:,} tokens {tokens:,} later steps {later}"
            f" seq {seq} {name} {label}".rstrip()
        )
    return lines


def join_tool_output(call: dict) -> str:
    """Return the text a tool call returned, from its content parts.

    Args:
        call: One tool call from ``list_tool_calls``.

    Returns:
        The text parts joined, or an empty string when there are none.
    """
    return "".join(
        part.get("text", "") for part in call["state"].get("content") or []
    )


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


def load_model_list() -> dict:
    """Return OpenCode's model list, or an empty one when it is missing.

    Returns:
        Providers by ID, each with its ``models`` by ID.
    """
    if not MODELS.is_file():
        return {}
    return json.loads(MODELS.read_text())


def find_api_price(providers: dict, model: str | None) -> dict | None:
    """Return the API price of a session's model in dollars per million tokens.

    The price comes from the session's own provider, else from the
    model's maker. A reseller in the list may charge less, so it never
    sets the price. As for the context window, an ID such as
    ``claude-opus-5-5[1m]`` also tries ``claude-opus-5-5``.

    Args:
        providers: OpenCode's model list from ``load_model_list``.
        model: The JSON the database stores, or None.

    Returns:
        Such as ``{"input": 4, "output": 20, "cache_read": 0.2}``, or
        None when no listed provider prices the model.
    """
    data = json.loads(model or "{}")
    if not data.get("id"):
        return None
    bare = data["id"].split("[")[0]
    for provider in (data.get("providerID"), *PRICE_MAKERS):
        listed = (providers.get(provider) or {}).get("models") or {}
        for model_id in (data["id"], bare):
            price = (listed.get(model_id) or {}).get("cost") or {}
            # A free tier lists zero rates, which are not the API price.
            if any(price.get(rate) for rate in API_RATES):
                return price
    return None


def count_api_dollars(
    price: dict,
    uncached: int,
    output: int,
    reasoning: int,
    read: int,
    write: int,
) -> float:
    """Return what a session's tokens would cost at the price's base rates.

    Reasoning tokens bill as output. A rate the price leaves out counts
    as free.

    Args:
        price: Dollars per million tokens, from ``find_api_price``.
        uncached: Input tokens not read from the cache.
        output: Output tokens.
        reasoning: Reasoning tokens.
        read: Tokens read from the cache.
        write: Tokens written to the cache.

    Returns:
        The cost in dollars.
    """
    rates = {
        "input": uncached,
        "output": output + reasoning,
        "cache_read": read,
        "cache_write": write,
    }
    return (
        sum(tokens * (price.get(rate) or 0) for rate, tokens in rates.items())
        / 1_000_000
    )


def find_context_window(model: str | None) -> int | None:
    """Return a session's context window from OpenCode's model list.

    The claude-code provider is not in the list, and its IDs add a
    suffix such as ``[1m]``. So when the exact ID is missing, the lookup
    tries the ID without the suffix, under the session's provider first.

    Args:
        model: The JSON the database stores, or None.

    Returns:
        The window in tokens, or None when the list or model is missing.
    """
    data = json.loads(model or "{}")
    providers = load_model_list()
    if not data.get("id") or not providers:
        return None
    own = providers.get(data.get("providerID")) or {}
    bare = data["id"].split("[")[0]
    candidates = [
        (provider.get("models") or {}).get(model_id)
        for model_id in (data["id"], bare)
        for provider in [own, *providers.values()]
    ]
    found = next((entry for entry in candidates if entry), {})
    return (found.get("limit") or {}).get("context")


def format_dollars(dollars: float | None, opening: str) -> str:
    """Return a price to add to a subagent line, or nothing when unknown.

    Args:
        dollars: Cost in dollars, or None when the model has no price.
        opening: Text before the price, such as ``" API price "``.

    Returns:
        Such as `` API price $1.25``, or ``""`` when unknown.
    """
    return "" if dollars is None else f"{opening}${dollars:,.2f}"


def format_time(ms: int) -> str:
    """Return a time in milliseconds as local time, such as ``Nov 18 02:00``.

    Args:
        ms: Milliseconds since the epoch.

    Returns:
        Month, day, hour and minute in the local time zone.
    """
    return datetime.fromtimestamp(ms / 1000).strftime("%b %d %H:%M")


def format_tokens(count: int) -> str:
    """Return a token count rounded for reading, such as ``245k`` or ``1M``.

    Args:
        count: Number of tokens.

    Returns:
        Thousands below a million, else millions to three digits.
    """
    if count >= 1_000_000:
        return f"{count / 1_000_000:.3g}M"
    return f"{round(count / 1000)}k"


def main() -> None:
    """Print the review lines, or exit 1 with a reason on stderr."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--session",
        default=os.environ.get("OPENCODE_SESSION_ID"),
        help="session ID; defaults to $OPENCODE_SESSION_ID",
    )
    parser.add_argument(
        "--context",
        action="store_true",
        help="print only the latest context and its share of the window",
    )
    parser.add_argument(
        "--usage",
        action="store_true",
        help="print the context, Claude plan usage and subagent prices",
    )
    args = parser.parse_args()
    session = args.session
    if not session:
        sys.exit("no session ID; pass --session or run inside OpenCode")
    if not DB.is_file():
        sys.exit(f"no OpenCode database at {DB}")
    with closing(sqlite3.connect(f"file:{DB}?mode=ro", uri=True)) as db:
        if args.context or args.usage:
            line = describe_context(db, session)
            lines = [line] if line else []
            if line and args.usage:
                lines += [describe_plan_usage()]
                lines += describe_usage(db, session)
        else:
            lines = report_session(db, session)
    if not lines:
        sys.exit(f"session {session} is not in {DB}")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
