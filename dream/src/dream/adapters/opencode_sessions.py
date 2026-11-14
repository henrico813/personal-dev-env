"""Sessions from the OpenCode database and ``session_stats.py``.

The adapter parses the script's text instead of computing carry from
the database itself, so ``dream`` and ``session-review`` report the
same numbers.
"""

from __future__ import annotations

import json
import re
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

from dream.ports import AdapterError, Cost, SessionStats, SessionSummary

STATS_SECONDS = 300
N = r"(\d[\d,]*)"
TOOL_LINE = re.compile(rf"^  (\S+) calls {N} tokens {N} carry {N}$")
OUTPUT_LINE = re.compile(
    rf"^  carry {N} tokens {N} later steps {N} seq \d+ (\S+) ?(.*)$"
)
SUBAGENT_LINE = re.compile(rf"^  (ses_\S+) (\S+) read {N} written {N} ?(.*)$")
REPEAT_LINE = re.compile(r"^  (\d+)x (.+)$")
MORE_LINE = re.compile(r"^  \d+ more$")
BLOCKS = (
    "carry by tool",
    "largest carried outputs",
    "largest subagents by tokens read",
    "repeated commands",
)


class OpenCodeSessions:
    """Large root sessions from one OpenCode database, read-only."""

    name = "opencode"

    def __init__(
        self, db: Path, stats_script: Path, local_providers: tuple[str, ...]
    ):
        """Keep the database, the stats script and the free providers."""
        self.db = db
        self.stats_script = stats_script
        self.local_providers = local_providers

    def list_large(
        self, since_hours: int, min_cloud_tokens: int
    ) -> list[SessionSummary]:
        """Sum each root session's tree from ``session_message``, read-only.

        A step counts as cloud when its ``providerID`` is not in
        ``local_providers``, such as "goog". Tokens read are input plus
        cache reads and writes, as ``session_stats.py`` counts them.

        Raises:
            AdapterError: The database could not be read.
        """
        now_ms = int(time.time() * 1000)
        since_ms = now_ms - since_hours * 3600 * 1000
        try:
            con = sqlite3.connect(f"file:{self.db}?mode=ro", uri=True)
            with con:
                sessions = con.execute(
                    "select id, parent_id, title from session_v2"
                ).fetchall()
                messages = con.execute(
                    "select session_id, time_created, data"
                    " from session_message"
                    " where type = 'assistant' and time_created >= ?",
                    (since_ms,),
                ).fetchall()
            con.close()
        except sqlite3.Error as error:
            raise AdapterError(f"opencode db: {error}") from error
        parent = {sid: pid for sid, pid, _ in sessions}
        title = {sid: name for sid, _, name in sessions}
        cloud: dict[str, int] = {}
        total: dict[str, int] = {}
        last: dict[str, int] = {}
        for sid, created, data in messages:
            root = sid
            while parent.get(root):
                root = parent[root]
            step = json.loads(data)
            tokens = step.get("tokens") or {}
            cache = tokens.get("cache") or {}
            read = (
                (tokens.get("input") or 0)
                + (cache.get("read") or 0)
                + (cache.get("write") or 0)
            )
            provider = (step.get("model") or {}).get("providerID")
            total[root] = total.get(root, 0) + read
            if provider not in self.local_providers:
                cloud[root] = cloud.get(root, 0) + read
            last[root] = max(last.get(root, 0), created)
        return [
            SessionSummary(
                id=root,
                title=title.get(root) or "",
                cloud_tokens=cloud.get(root, 0),
                tokens_read=total[root],
                idle_minutes=(now_ms - last[root]) // 60000,
            )
            for root in total
            if cloud.get(root, 0) >= min_cloud_tokens
        ]

    def read_stats(self, summary: SessionSummary) -> SessionStats:
        """Run ``session_stats.py --session ID`` and parse its blocks.

        Raises:
            AdapterError: The script failed, or a block line did not parse.
        """
        try:
            done = subprocess.run(
                [
                    sys.executable,
                    str(self.stats_script),
                    "--session",
                    summary.id,
                ],
                check=False,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=STATS_SECONDS,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise AdapterError(f"session_stats: {error}") from error
        if done.returncode != 0:
            reason = done.stderr.strip().splitlines()[-1:] or ["no message"]
            raise AdapterError(f"session_stats: {reason[0]}")
        root_model, costs = parse_stats(done.stdout)
        return SessionStats(summary, root_model, tuple(costs))


def parse_stats(text: str) -> tuple[str, list[Cost]]:
    """Read the root model and the costs from session_stats.py's lines.

    Reads four blocks and skips the rest. A line inside a block that
    matches no known shape raises, so a format change never yields
    wrong numbers.

    Raises:
        AdapterError: A block line did not parse.

    Example:
        >>> line = "  carry 384,946 tokens 6,637 later steps 58 seq 5 shell ls"
        >>> parse_stats(f"largest carried outputs\\n{line}")[1][0].reread
        384946
    """
    root_model = ""
    block = ""
    costs = []
    for line in text.splitlines():
        if not line.startswith("  "):
            block = next((b for b in BLOCKS if line.startswith(b)), "")
            if line.startswith("model "):
                root_model = line[len("model ") :]
            continue
        if not block or MORE_LINE.match(line):
            continue
        cost = parse_block_line(block, line)
        if cost is None:
            raise AdapterError(f"session_stats line: {line.strip()[:60]}")
        costs.append(cost)
    return root_model, costs


def parse_block_line(block: str, line: str) -> Cost | None:
    """Turn one indented block line into a Cost, or None when it differs."""
    if block == "carry by tool":
        match = TOOL_LINE.match(line)
        if match:
            tool, calls, tokens, carry = match.groups()
            return Cost("tool", tool, num(tokens), num(carry), 0, num(calls))
    elif block == "largest carried outputs":
        match = OUTPUT_LINE.match(line)
        if match:
            carry, tokens, steps, tool, what = match.groups()
            kind = "skill" if tool == "skill" else "output"
            label = what if kind == "skill" else f"{tool}: {what}".rstrip()
            return Cost(kind, label, num(tokens), num(carry), num(steps), 1)
    elif block == "largest subagents by tokens read":
        match = SUBAGENT_LINE.match(line)
        if match:
            _, model, read, _, title = match.groups()
            label = f"{title} on {model}" if title else model
            return Cost("subagent", label, num(read), 0, 0, 1)
    else:
        match = REPEAT_LINE.match(line)
        if match:
            count, command = match.groups()
            return Cost("repeat", command, 0, 0, 0, int(count))
    return None


def num(text: str) -> int:
    """Read a number with thousands commas."""
    return int(text.replace(",", ""))
