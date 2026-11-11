"""Read sessions from OpenCode's database, which only this machine has.

``hub status`` reads each session's context size, and ``hub alive``
reads whether a session still works.
"""

from __future__ import annotations

import json
import sqlite3
from contextlib import closing
from pathlib import Path


def read_context_tokens(session: str) -> int | None:
    """Read the context the session's latest trusted model step read.

    The claude-code provider logs some steps with zero tokens and adds
    them to the next step, so that next step reads about twice the real
    context. Zero steps and the steps right after them are left out, as
    ``pde-context`` does.

    Args:
        session: The OpenCode session ID.

    Returns:
        Uncached input plus cache reads and writes, or None when the
        database has no trusted step for the session.

    Raises:
        sqlite3.Error: The database could not be read, such as when it
            is locked or its tables changed.
    """
    context, after_zero = None, False
    for (text,) in query_session(
        "select json_extract(data, '$.tokens') from session_message"
        " where session_id = ? and type = 'assistant' order by seq",
        session,
    ):
        tokens = json.loads(text) if text else {}
        cache = tokens.get("cache") or {}
        size = (
            (tokens.get("input") or 0)
            + (cache.get("read") or 0)
            + (cache.get("write") or 0)
        )
        if size and not after_zero:
            context = size
        after_zero = size == 0
    return context


def query_session(query: str, session: str) -> list[tuple]:
    """Run one query on OpenCode's database, opened read-only.

    Args:
        query: SQL with one ``?``, which takes the session ID.
        session: The OpenCode session ID.

    Returns:
        The rows, or no rows when the database is missing.

    Raises:
        sqlite3.Error: The database could not be read, such as when it
            is locked or its tables changed.
    """
    database = Path.home() / ".local" / "share" / "opencode" / "opencode.db"
    if not database.exists():
        return []
    with closing(
        sqlite3.connect(f"{database.as_uri()}?mode=ro", uri=True)
    ) as connection:
        return connection.execute(query, (session,)).fetchall()
