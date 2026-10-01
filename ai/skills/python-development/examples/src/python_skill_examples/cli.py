"""Command-line entry point for worker-count parsing."""

import sys
from collections.abc import Sequence

from python_skill_examples.counts import parse_worker_count


def main(argv: Sequence[str]) -> int:
    """Print one valid count, or report invalid input and return failure."""
    if len(argv) != 1:
        print("usage: worker-count COUNT", file=sys.stderr)
        return 2

    try:
        count = parse_worker_count(argv[0])
    except ValueError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    print(count)
    return 0


def run() -> int:
    """Run the console script with process arguments."""
    return main(sys.argv[1:])


if __name__ == "__main__":
    raise SystemExit(run())
