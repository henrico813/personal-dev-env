"""Check one model answer against its form's rules.

``check_answer`` is the entry point. Each reason names its rule ID and
quotes up to 50 characters of the line that broke it, such as
``number-not-in-fact: 3.9``.
"""

from __future__ import annotations

import json
import re
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from dream.forms import Fact

FORM_KEYS = {
    "A": frozenset({"kind", "text", "based_on"}),
    "B": frozenset({"fact", "finding", "fix"}),
}
FORM_RULES = {
    "A": (
        "not-json",
        "unknown-key",
        "pretend-tool-call",
        "too-few-criteria",
        "too-many-criteria",
        "too-few-questions",
        "too-many-questions",
        "quote-too-short",
        "quote-not-in-input",
        "too-long",
    ),
    "B": (
        "not-json",
        "unknown-key",
        "pretend-tool-call",
        "too-few-findings",
        "too-many-findings",
        "unknown-fact",
        "same-fact-twice",
        "number-not-in-fact",
        "too-long",
    ),
}
MAX_REASONS = 5
QUOTE_CHARS = 50
PRETEND_TOOL_CALL = re.compile(r"<tool_call>|<function=")
# The lookbehind skips digits inside words, such as the 1 in F1, so a
# fact ID or a model name never counts as a number.
NUMBER = re.compile(r"(?<![\w.])\d[\d,]*(?:\.\d+)?")
SPACES = re.compile(r"\s+")


def check_answer(form: str, answer: str, given: str | list[Fact]) -> list[str]:
    """List why an answer breaks its form's rules, empty when it passes.

    Args:
        form: "A" or "B".
        answer: The model's reply, one JSON object per line.
        given: The item text for form A, the fact list for form B.

    Returns:
        Up to five reasons, each starting with a rule ID.

    Example:
        >>> row = '{"fact": "F2", "finding": "About 3.9 million", "fix": "x"}'
        >>> check_answer("B", row, facts)
        ['number-not-in-fact: 3.9']
    """
    rows, reasons = parse_rows(answer, FORM_KEYS[form])
    if form == "A":
        assert isinstance(given, str)
        reasons += count_proposal_rows(rows)
        source = join_spaces(given)
        for row in rows:
            reasons += check_quote(row, source)
            if row.get("kind") == "criterion":
                reasons += check_length(row, "text", 30)
    else:
        assert isinstance(given, list)
        reasons += count_digest_rows(rows)
        facts = {fact.id: fact for fact in given}
        seen: set[str] = set()
        for row in rows:
            reasons += check_fact(row, facts, seen)
            reasons += check_length(row, "finding", 25)
            reasons += check_length(row, "fix", 25)
    return reasons[:MAX_REASONS]


def whole_numbers(text: str) -> list[int | None]:
    """Read each number in text; None for one with a decimal point.

    Example:
        >>> whole_numbers("3,908,032 tokens, about 3.9 million")
        [3908032, None]
    """
    numbers: list[int | None] = []
    for match in NUMBER.findall(text):
        match = match.rstrip(",")
        numbers.append(None if "." in match else int(match.replace(",", "")))
    return numbers


def parse_rows(
    answer: str, keys: frozenset[str]
) -> tuple[list[dict[str, Any]], list[str]]:
    """Read the answer's JSON lines, skipping blank lines and code fences."""
    reasons = []
    if PRETEND_TOOL_CALL.search(answer):
        reasons.append("pretend-tool-call")
    rows = []
    for line in answer.splitlines():
        line = line.strip()
        if not line or line.startswith("```"):
            continue
        try:
            row = json.loads(line)
        except ValueError:
            row = None
        if not isinstance(row, dict):
            reasons.append(f"not-json: {line[:QUOTE_CHARS]}")
            continue
        extra = sorted(set(row) - keys)
        if extra:
            reasons.append(f"unknown-key: {', '.join(extra)[:QUOTE_CHARS]}")
            continue
        rows.append(row)
    return rows, reasons


def count_proposal_rows(rows: list[dict[str, Any]]) -> list[str]:
    """Hold form A to 2-4 criteria and 1-3 questions."""
    criteria = sum(row.get("kind") == "criterion" for row in rows)
    questions = sum(row.get("kind") == "question" for row in rows)
    reasons = []
    if criteria < 2:
        reasons.append(f"too-few-criteria: {criteria}")
    if criteria > 4:
        reasons.append(f"too-many-criteria: {criteria}")
    if questions < 1:
        reasons.append(f"too-few-questions: {questions}")
    if questions > 3:
        reasons.append(f"too-many-questions: {questions}")
    return reasons


def count_digest_rows(rows: list[dict[str, Any]]) -> list[str]:
    """Hold form B to 1-3 findings."""
    if not rows:
        return ["too-few-findings: 0"]
    if len(rows) > 3:
        return [f"too-many-findings: {len(rows)}"]
    return []


def check_quote(row: dict[str, Any], source: str) -> list[str]:
    """Require a form A quote of 10 or more characters found in the item."""
    quote = join_spaces(str(row.get("based_on", "")))
    if len(quote) < 10:
        return [f"quote-too-short: {quote[:QUOTE_CHARS]}"]
    if quote not in source:
        return [f"quote-not-in-input: {quote[:QUOTE_CHARS]}"]
    return []


def check_fact(
    row: dict[str, Any], facts: dict[str, Fact], seen: set[str]
) -> list[str]:
    """Require a known, unused fact and only that fact's whole numbers."""
    fact_id = str(row.get("fact", ""))
    fact = facts.get(fact_id)
    if fact is None:
        return [f"unknown-fact: {fact_id[:QUOTE_CHARS]}"]
    if fact_id in seen:
        return [f"same-fact-twice: {fact_id}"]
    seen.add(fact_id)
    reasons = []
    text = f"{row.get('finding', '')} {row.get('fix', '')}"
    for raw, number in zip(NUMBER.findall(text), whole_numbers(text)):
        if number is None or number not in fact.numbers:
            reasons.append(f"number-not-in-fact: {raw.rstrip(',')}")
    return reasons


def check_length(row: dict[str, Any], key: str, words: int) -> list[str]:
    """Fail a field with more than ``words`` words."""
    text = str(row.get(key, ""))
    if len(text.split()) > words:
        return [f"too-long: {text[:QUOTE_CHARS]}"]
    return []


def join_spaces(text: str) -> str:
    """Join runs of whitespace to one space, so line wraps do not matter."""
    return SPACES.sub(" ", text).strip()
