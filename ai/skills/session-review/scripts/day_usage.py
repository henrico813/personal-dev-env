"""Print one day's tokens and list-price cost across all sessions.

Reads every model step created on one local day and prints a total, one
line per model and the costliest sessions. Today the steps come from
OpenCode, read-only, through ``OpenCodeSource``; ``report_day`` works
with any ``UsageSource``. Read ``report_day`` first.

Example:
    python3 ~/.agents/skills/session-review/scripts/day_usage.py
    python3 day_usage.py --date 2026-11-24
"""

import argparse
import sqlite3
import sys
from contextlib import closing
from datetime import date, datetime, time, timedelta

from opencode_source import DB, OpenCodeSource
from usage_cost import UsageSource, calculate_step_cost, describe_cost

SESSIONS_SHOWN = 10


def report_day(source: UsageSource, day: date) -> list[str]:
    """Build the usage lines for every model step created on ``day``.

    Args:
        source: Where the model steps and their prices come from.
        day: The local calendar day to sum.

    Returns:
        A day line, the cost line, one line per model, then the
        costliest sessions.
    """
    start = datetime.combine(day, time()).astimezone()
    end = datetime.combine(day + timedelta(days=1), time()).astimezone()
    usages = source.list_day_usage(start, end)
    models: dict[str, list] = {}
    sessions: dict[str, list] = {}
    for usage in usages:
        priced = calculate_step_cost(usage, source)
        dollars = sum(priced.values()) if priced else 0.0
        totals = models.setdefault(
            f"{usage.provider}/{usage.model}", [0, 0, 0.0, False]
        )
        totals[0] += 1
        totals[1] += sum(
            usage.tokens.get(kind, 0)
            for kind in ("cache write", "cache read", "uncached")
        )
        totals[2] += dollars
        totals[3] = totals[3] or priced is None
        sessions.setdefault(usage.session, [usage.title, 0.0])[1] += dollars
    lines = [
        f"day {day.isoformat()}, model steps {len(usages):,},"
        f" sessions {len(sessions)}",
        describe_cost(usages, source),
        "by model",
    ]
    for name, (count, read, dollars, unpriced) in sorted(
        models.items(), key=lambda item: -item[1][2]
    ):
        cost = "unpriced" if unpriced else f"${dollars:,.2f}"
        lines.append(
            f"  {name} steps {count:,} tokens read {read:,} cost {cost}"
        )
    lines.append("costliest sessions")
    ranked = sorted(sessions.items(), key=lambda item: -item[1][1])
    for session, (title, dollars) in ranked[:SESSIONS_SHOWN]:
        lines.append(f"  {session} ${dollars:,.2f} {title[:60]}".rstrip())
    return lines


def main() -> None:
    """Print the day's lines, or exit 1 with a reason on stderr."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--date",
        type=date.fromisoformat,
        default=date.today(),
        help="local day as YYYY-MM-DD; defaults to today",
    )
    args = parser.parse_args()
    if not DB.is_file():
        sys.exit(f"no OpenCode database at {DB}")
    with closing(sqlite3.connect(f"file:{DB}?mode=ro", uri=True)) as db:
        print("\n".join(report_day(OpenCodeSource(db), args.date)))


if __name__ == "__main__":
    main()
