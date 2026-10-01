"""Bounded worker-count parsing and file input."""

from pathlib import Path

MIN_WORKERS = 1
MAX_WORKERS = 32


class CountFileError(Exception):
    """Base class for failures at the count-file boundary."""


class CountReadError(CountFileError):
    """The count file could not be read."""


class CountParseError(CountFileError):
    """The count file did not contain a supported value."""


def parse_worker_count(text: str) -> int:
    """Parse a worker count from 1 through 32, ignoring outer whitespace.

    >>> parse_worker_count(" 4 ")
    4
    """
    count = int(text.strip())
    if not MIN_WORKERS <= count <= MAX_WORKERS:
        raise ValueError(
            f"worker count must be between {MIN_WORKERS} and {MAX_WORKERS}"
        )
    return count


def load_worker_count(path: Path) -> int:
    """Read and parse a count while preserving its lower-level cause."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise CountReadError(
            f"could not read worker count from {path}"
        ) from error

    try:
        return parse_worker_count(text)
    except ValueError as error:
        raise CountParseError(f"invalid worker count in {path}") from error
