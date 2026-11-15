"""Work items from the pde ``card`` command, read-only."""

from __future__ import annotations

import datetime
import re
import subprocess

from dream.ports import AdapterError, WorkItem

CARD_SECONDS = 60
CHECKBOX = re.compile(r"^\s*\d*\.?\s*- \[[ xX]\]", re.MULTILINE)
DATE_CREATED = re.compile(
    r"^date_created:\s*(\d{4}-\d{2}-\d{2})", re.MULTILINE
)
UNSAFE = re.compile(r"[^A-Za-z0-9._-]+")


class PdeCards:
    """Draft pde cards, read through ``card list`` and ``card show``."""

    name = "pde-cards"

    def list_drafts(self) -> list[WorkItem]:
        """Run ``card list --stage draft --tabs``, then ``card show`` per card.

        ``has_criteria`` is True when the card's Acceptance Criteria
        section holds a checkbox line.

        Raises:
            AdapterError: A ``card`` call failed or timed out.
        """
        listing = run_card("list", "--stage", "draft", "--tabs")
        items = []
        for line in listing.splitlines():
            fields = line.split("\t")
            if len(fields) < 5:
                continue
            key, title = fields[0], fields[4]
            text = run_card("show", key)
            criteria = run_card(
                "show", key, "--section", "Acceptance Criteria"
            )
            items.append(
                WorkItem(
                    key=key,
                    file_key=UNSAFE.sub("-", key),
                    title=title,
                    created=read_created(text),
                    text=text,
                    has_criteria=bool(CHECKBOX.search(criteria)),
                )
            )
        return items


def run_card(*args: str) -> str:
    """Run ``card`` with stdin closed and return its output."""
    try:
        done = subprocess.run(
            ["card", *args],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=CARD_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise AdapterError(f"card {args[0]}: {error}") from error
    if done.returncode != 0:
        reason = done.stderr.strip().splitlines()[-1:] or ["no message"]
        raise AdapterError(f"card {' '.join(args[:2])}: {reason[0]}")
    return done.stdout


def read_created(text: str) -> datetime.date:
    """Return the card's ``date_created``, or the latest date when absent."""
    match = DATE_CREATED.search(text)
    if match is None:
        return datetime.date.max
    return datetime.date.fromisoformat(match[1])
