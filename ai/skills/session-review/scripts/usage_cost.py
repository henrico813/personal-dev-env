"""Add up the list-price cost of model steps from any session source.

A source, such as ``OpenCodeSource`` in ``opencode_source.py``, reads
its own records into ``Usage`` rows and looks up prices. The functions
here only add up those rows, so a new source, such as another coding
agent's session files, needs no change to them. Read ``describe_cost``
first.
"""

from dataclasses import dataclass, field
from datetime import datetime
from typing import Protocol

# Token kinds in the cost line, in print order. Rates use the same keys.
TOKEN_KINDS = ("cache write", "cache read", "uncached", "output")


@dataclass(frozen=True)
class Usage:
    """Tokens of one model step, keyed by ``TOKEN_KINDS``.

    Output includes reasoning, since both are billed at the output
    price.
    """

    provider: str
    model: str
    tokens: dict[str, int] = field(default_factory=dict)
    session: str = ""
    title: str = ""


class UsageSource(Protocol):
    """Where model steps and their prices come from, such as OpenCode."""

    def find_rates(self, provider: str, model: str) -> dict[str, float] | None:
        """Return dollars per million tokens keyed by ``TOKEN_KINDS``.

        Returns None when the source knows no price for the model.
        """
        ...

    def list_subagent_usage(self, session: str) -> list[Usage]:
        """Return the model steps of a session's direct subagents."""
        ...

    def list_day_usage(self, start: datetime, end: datetime) -> list[Usage]:
        """Return every model step created from ``start`` up to ``end``."""
        ...


def describe_cost(usages: list[Usage], source: UsageSource) -> str:
    """Sum the list-price dollars of model steps by token kind.

    Args:
        usages: The model steps to price.
        source: Where each step's prices come from.

    Returns:
        A line such as ``list-price cost $6.25 (cache write $6.25, cache
        read $0.00, uncached $0.00, output $0.00)``, ending with
        ``, unpriced steps 2`` when some steps had no price.
    """
    totals = dict.fromkeys(TOKEN_KINDS, 0.0)
    unpriced = 0
    for usage in usages:
        dollars = calculate_step_cost(usage, source)
        if dollars is None:
            unpriced += 1
            continue
        for kind, amount in dollars.items():
            totals[kind] += amount
    parts = ", ".join(
        f"{kind} ${amount:,.2f}" for kind, amount in totals.items()
    )
    line = f"list-price cost ${sum(totals.values()):,.2f} ({parts})"
    return line + (f", unpriced steps {unpriced}" if unpriced else "")


def calculate_step_cost(
    usage: Usage, source: UsageSource
) -> dict[str, float] | None:
    """Return one step's list-price dollars by token kind.

    Context-size price tiers are ignored.

    Args:
        usage: One model step.
        source: Where the step's prices come from.

    Returns:
        Dollars keyed by ``TOKEN_KINDS``, or None when unpriced.
    """
    rates = source.find_rates(usage.provider, usage.model)
    if rates is None:
        return None
    return {
        kind: usage.tokens.get(kind, 0) * rates[kind] / 1_000_000
        for kind in TOKEN_KINDS
    }


def describe_rates(provider: str, model: str, source: UsageSource) -> str:
    """Describe a model's prices, to turn token savings into dollars.

    Args:
        provider: Provider ID, such as ``claude-code``.
        model: Model ID, such as ``claude-opus-5-5``.
        source: Where the prices come from.

    Returns:
        A line such as ``rates per M tokens for claude-code/opus: input
        $5, output $25, cache read $0.5, cache write $6.25``.
    """
    rates = source.find_rates(provider, model)
    if rates is None:
        return f"rates unknown for {provider}/{model}"
    return (
        f"rates per M tokens for {provider}/{model}:"
        f" input ${rates['uncached']:g}, output ${rates['output']:g},"
        f" cache read ${rates['cache read']:g},"
        f" cache write ${rates['cache write']:g}"
    )
