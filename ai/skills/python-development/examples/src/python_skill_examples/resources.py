"""Resource setup and explicit fallible completion."""

from collections.abc import Callable, Iterable, Iterator
from contextlib import AbstractContextManager, ExitStack, contextmanager
from pathlib import Path
from typing import TextIO

TextOpener = Callable[[Path], AbstractContextManager[TextIO]]


def open_text(path: Path) -> AbstractContextManager[TextIO]:
    """Open one UTF-8 text output."""
    return path.open("w", encoding="utf-8")


@contextmanager
def open_text_files(
    paths: Iterable[Path], *, opener: TextOpener = open_text
) -> Iterator[list[TextIO]]:
    """Close every acquired file, including when later setup fails."""
    with ExitStack() as stack:
        outputs = [stack.enter_context(opener(path)) for path in paths]
        yield outputs


def write_lines(output: TextIO, lines: Iterable[str]) -> None:
    """Write complete lines and report a flush failure to the caller."""
    for line in lines:
        output.write(f"{line}\n")
    output.flush()
