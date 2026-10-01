"""Dependency replacement at the lookup site."""

from collections.abc import Callable
from os import listdir

ListEntries = Callable[[str], list[str]]


def visible_entries(path: str) -> list[str]:
    """Return sorted non-hidden directory entries."""
    return sorted(name for name in listdir(path) if not name.startswith("."))


def visible_entries_with(path: str, list_entries: ListEntries) -> list[str]:
    """Return visible entries using an explicitly supplied dependency."""
    return sorted(
        name for name in list_entries(path) if not name.startswith(".")
    )
