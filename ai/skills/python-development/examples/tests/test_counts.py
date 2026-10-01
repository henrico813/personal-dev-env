from pathlib import Path

import pytest

from python_skill_examples.counts import (
    CountParseError,
    CountReadError,
    load_worker_count,
    parse_worker_count,
)


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        pytest.param("1", 1, id="minimum"),
        pytest.param(" 4 \n", 4, id="whitespace"),
        pytest.param("32", 32, id="maximum"),
    ],
)
def test_parses_supported_counts(text: str, expected: int) -> None:
    assert parse_worker_count(text) == expected


@pytest.mark.parametrize(
    "text",
    [
        pytest.param("0", id="below-range"),
        pytest.param("33", id="above-range"),
        pytest.param("many", id="not-integer"),
    ],
)
def test_rejects_unsupported_counts(text: str) -> None:
    with pytest.raises(ValueError):
        parse_worker_count(text)


def test_missing_file_keeps_read_cause(tmp_path: Path) -> None:
    with pytest.raises(CountReadError) as error:
        load_worker_count(tmp_path / "missing.txt")

    assert isinstance(error.value.__cause__, FileNotFoundError)


def test_invalid_text_keeps_parse_cause(tmp_path: Path) -> None:
    path = tmp_path / "invalid.txt"
    path.write_text("many", encoding="utf-8")

    with pytest.raises(CountParseError) as error:
        load_worker_count(path)

    assert isinstance(error.value.__cause__, ValueError)
