"""Neutral types and the ports each machine's adapters fill.

The core reads only these types, so a new place to read work items,
sessions or a model needs one adapter and no core change.
"""

from __future__ import annotations

import datetime
from dataclasses import dataclass
from typing import Protocol


class AdapterError(Exception):
    """A port failed; the message is one line for the morning report."""


@dataclass(frozen=True)
class WorkItem:
    """One draft work item, such as a pde card or a GitHub issue.

    Attributes:
        key: The tracker's key, such as "PDEV-KB-184" or "owner/repo#12".
        file_key: ``key`` made safe for a file name, such as "owner-repo-12".
        title: The item's title.
        created: When the item was created, for oldest-first order.
        text: The full text the model reads and the check quotes from.
        has_criteria: True when the item already lists acceptance criteria.
    """

    key: str
    file_key: str
    title: str
    created: datetime.date
    text: str
    has_criteria: bool


@dataclass(frozen=True)
class SessionSummary:
    """One root session and its tree's tokens in the window.

    Attributes:
        id: The root session's ID.
        title: The root session's title.
        cloud_tokens: Tokens read by cloud models, subagents included.
        tokens_read: All tokens read, local models included.
        idle_minutes: Minutes since the tree's last step.
    """

    id: str
    title: str
    cloud_tokens: int
    tokens_read: int
    idle_minutes: int


@dataclass(frozen=True)
class Cost:
    """One thing in a session that cost tokens.

    Attributes:
        kind: "output", "tool", "skill", "subagent" or "repeat".
        what: What a person would recognise, such as "shell: git diff".
        tokens: The tokens it produced, or a subagent's tokens read.
        reread: Tokens later steps re-read because of it; 0 when unknown.
        later_steps: Steps that re-read it; 0 when unknown.
        count: Calls or repeats; 1 for a single output.
    """

    kind: str
    what: str
    tokens: int
    reread: int
    later_steps: int
    count: int


@dataclass(frozen=True)
class SessionStats:
    """A session's costs, as a harness adapter reads them."""

    summary: SessionSummary
    root_model: str
    costs: tuple[Cost, ...]


@dataclass(frozen=True)
class Answer:
    """One model call: the text, tokens read, seconds, or why it failed."""

    text: str
    tokens_read: int
    seconds: float
    failure: str


class WorkItems(Protocol):
    """Lists draft work items; never changes one."""

    name: str

    def list_drafts(self) -> list[WorkItem]:
        """Return every draft item the adapter sees."""
        ...


class Sessions(Protocol):
    """Lists large sessions and reads their costs; never changes one."""

    name: str

    def list_large(
        self, since_hours: int, min_cloud_tokens: int
    ) -> list[SessionSummary]:
        """Return root sessions that read at least ``min_cloud_tokens``."""
        ...

    def read_stats(self, summary: SessionSummary) -> SessionStats:
        """Return the costs of one session tree."""
        ...


class Model(Protocol):
    """Answers one prompt with no tools."""

    name: str

    def ask(self, prompt: str, timeout: int) -> Answer:
        """Return the reply, with ``failure`` set instead of raising."""
        ...
