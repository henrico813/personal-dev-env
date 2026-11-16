"""Tests for the rules that decide whether a dream is saved."""

import json

import pytest

from dream.checks import check_answer
from dream.forms import Fact

CARD = (
    "export crashes when the board is empty."
    " the file is always called export.csv"
)
FACTS = [
    Fact(
        "F1",
        "F1: one shell output was re-read, 1,316,820 tokens in total",
        frozenset({1316820}),
    ),
    Fact(
        "F2",
        "F2: shell calls returned 52,993 tokens in 116 calls;"
        " later steps re-read them, 3,908,032 tokens in total",
        frozenset({52993, 116, 3908032}),
    ),
]


def rows(*objects: dict) -> str:
    """Join objects into an answer of JSON lines."""
    return "\n".join(json.dumps(o) for o in objects)


def criterion(quote: str = "export crashes when the board is empty") -> dict:
    """A form A criterion line based on ``quote``."""
    return {
        "kind": "criterion",
        "text": "An empty board exports a header row.",
        "based_on": quote,
    }


QUESTION = {
    "kind": "question",
    "text": "What is the file called?",
    "based_on": "the file is always called export.csv",
}
GOOD_A = rows(criterion(), criterion(), QUESTION)


def finding(fact: str, text: str) -> dict:
    """A form B finding line about ``fact``."""
    return {"fact": fact, "finding": text, "fix": "Pipe it through head."}


class TestFormA:
    """Form A rules over a proposal and its item text."""

    def test_good_proposal_passes(self) -> None:
        """A proposal quoting the item passes.

        A rule that rejects good answers would waste every night's
        proposals before anyone saw them.
        """
        assert check_answer("A", GOOD_A, CARD) == []

    @pytest.mark.parametrize(
        ("answer", "rule"),
        [
            pytest.param(
                rows(
                    criterion("the worker installs it"), criterion(), QUESTION
                ),
                "quote-not-in-input",
                id="made-up-quote",
            ),
            pytest.param(
                rows(*[criterion()] * 5, QUESTION),
                "too-many-criteria: 5",
                id="five-criteria",
            ),
            pytest.param(
                rows(
                    criterion(), criterion(), dict(QUESTION, based_on="export")
                ),
                "quote-too-short",
                id="short-quote",
            ),
            pytest.param(
                rows(criterion(), criterion(), dict(QUESTION, why="x")),
                "unknown-key",
                id="extra-key",
            ),
            pytest.param(
                GOOD_A + "\n<tool_call>read</tool_call>",
                "pretend-tool-call",
                id="pretend-tool",
            ),
        ],
    )
    def test_broken_proposal_fails_with_rule(
        self, answer: str, rule: str
    ) -> None:
        """Each broken proposal fails and names its rule.

        Each case is a way the local model has failed or could invent a
        claim; a proposal that slips through misleads the card's reader.
        """
        reasons = check_answer("A", answer, CARD)

        assert any(r.startswith(rule) for r in reasons), reasons


class TestFormB:
    """Form B rules over a digest and the session's facts."""

    @pytest.mark.parametrize(
        "text",
        [
            pytest.param(
                "Shell output was re-read for 3,908,032 tokens in total.",
                id="commas",
            ),
            pytest.param(
                "Shell output was re-read for 3908032 tokens in total.",
                id="no-commas",
            ),
        ],
    )
    def test_exact_number_from_its_fact_passes(self, text: str) -> None:
        """A finding with its fact's exact number passes, commas or not.

        The model may drop commas; failing that would throw away true
        digests.
        """
        assert check_answer("B", rows(finding("F2", text)), FACTS) == []

    @pytest.mark.parametrize(
        ("answer", "reason"),
        [
            pytest.param(
                rows(
                    finding(
                        "F2", "Shell output cost nearly 3.9 million tokens."
                    )
                ),
                "number-not-in-fact: 3.9",
                id="rounded",
            ),
            pytest.param(
                rows(finding("F2", "Re-read for 1,316,820 tokens.")),
                "number-not-in-fact: 1,316,820",
                id="other-fact",
            ),
            pytest.param(
                rows(finding("F7", "Shell output.")),
                "unknown-fact: F7",
                id="unknown-fact",
            ),
            pytest.param(
                rows(
                    finding("F1", "Strings output."), finding("F1", "Again.")
                ),
                "same-fact-twice: F1",
                id="same-fact",
            ),
        ],
    )
    def test_number_or_fact_mismatch_fails(
        self, answer: str, reason: str
    ) -> None:
        """A rounded, borrowed or unknown number or fact fails.

        The trial check matched digit substrings and passed "3.9
        million"; the quality review found models pull numbers from the
        wrong line. Either would put a false number in front of a reader
        who will not check it by hand.
        """
        assert reason in check_answer("B", answer, FACTS)
