"""Read model steps and prices from OpenCode, for the cost lines.

``OpenCodeSource`` is the ``UsageSource`` for OpenCode. It reads the
local session database and prices models from OpenCode's models.json,
so every OpenCode detail the cost lines need stays in this file.
"""

import json
import sqlite3
from datetime import datetime
from functools import cache
from pathlib import Path

from usage_cost import Usage

DB = Path.home() / ".local/share/opencode/opencode.db"
MODELS = Path.home() / ".cache/opencode/models.json"
# The claude-code provider has no prices of its own; Anthropic's apply.
PRICE_FALLBACK = {"claude-code": "anthropic"}


class OpenCodeSource:
    """Model steps from OpenCode's database, priced by models.json."""

    def __init__(self, db: sqlite3.Connection) -> None:
        """Keep a read-only connection to the OpenCode database."""
        self.db = db

    def find_rates(self, provider: str, model: str) -> dict[str, float] | None:
        """Return a model's list prices per million tokens.

        Only the model's own provider counts, plus Anthropic for
        claude-code, since other providers list the same ID at other
        prices. An ID suffix such as ``[1m]`` is tried without it too.

        Args:
            provider: Provider ID, such as ``claude-code``.
            model: Model ID, such as ``claude-opus-5-5[1m]``.

        Returns:
            Prices keyed by ``TOKEN_KINDS``, or None when the model has
            no price.
        """
        for name in (provider, PRICE_FALLBACK.get(provider)):
            listed = (load_models().get(name) or {}).get("models") or {}
            for candidate in (model, model.split("[")[0]):
                cost = (listed.get(candidate) or {}).get("cost")
                if cost:
                    return {
                        "cache write": cost.get("cache_write", 0),
                        # Some models list no cache read price; reads
                        # then cost the same as uncached input.
                        "cache read": cost.get(
                            "cache_read", cost.get("input", 0)
                        ),
                        "uncached": cost.get("input", 0),
                        "output": cost.get("output", 0),
                    }
        return None

    def list_subagent_usage(self, session: str) -> list[Usage]:
        """Return the model steps of a session's direct subagents."""
        rows = self.db.execute(
            "select m.data from session_message m"
            " join session_v2 s on s.id = m.session_id"
            " where s.parent_id = ? and m.type = 'assistant'",
            (session,),
        )
        return [read_step_usage(json.loads(data)) for (data,) in rows]

    def list_day_usage(self, start: datetime, end: datetime) -> list[Usage]:
        """Return every model step created from ``start`` up to ``end``."""
        rows = self.db.execute(
            "select m.session_id, s.title, m.data from session_message m"
            " join session_v2 s on s.id = m.session_id"
            " where m.type = 'assistant'"
            " and m.time_created >= ? and m.time_created < ?",
            (int(start.timestamp() * 1000), int(end.timestamp() * 1000)),
        )
        return [
            read_step_usage(json.loads(data), session, title or "")
            for session, title, data in rows
        ]


def read_step_usage(step: dict, session: str = "", title: str = "") -> Usage:
    """Turn the data of one assistant message into a ``Usage`` row.

    Args:
        step: Data of one assistant message.
        session: OpenCode session ID the step belongs to.
        title: That session's title.

    Returns:
        The step's tokens, with reasoning counted as output.
    """
    model = step.get("model") or {}
    tokens = step.get("tokens") or {}
    cache_tokens = tokens.get("cache") or {}
    return Usage(
        provider=model.get("providerID") or "?",
        model=model.get("id") or "?",
        tokens={
            "cache write": cache_tokens.get("write") or 0,
            "cache read": cache_tokens.get("read") or 0,
            "uncached": tokens.get("input") or 0,
            "output": (tokens.get("output") or 0)
            + (tokens.get("reasoning") or 0),
        },
        session=session,
        title=title,
    )


@cache
def load_models() -> dict:
    """Return OpenCode's model list, or an empty dict when it is missing.

    The file is about 5 MB, so it is parsed once per run.
    """
    if not MODELS.is_file():
        return {}
    return json.loads(MODELS.read_text())
