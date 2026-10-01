import io
from pathlib import Path
from types import TracebackType

import pytest

from python_skill_examples.resources import (
    TextOpener,
    open_text_files,
    write_lines,
)


class TrackedOutput(io.StringIO):
    def __init__(self, name: str, events: list[str], *, fail: bool) -> None:
        super().__init__()
        self.name = name
        self.events = events
        self.fail = fail

    def __enter__(self) -> "TrackedOutput":
        self.events.append(f"enter:{self.name}")
        if self.fail:
            raise OSError("setup failed")
        return self

    def __exit__(
        self,
        exception_type: type[BaseException] | None,
        exception: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self.events.append(f"exit:{self.name}")
        self.close()


@pytest.fixture
def failing_opener() -> tuple[list[str], TextOpener]:
    events: list[str] = []

    def opener(path: Path) -> TrackedOutput:
        return TrackedOutput(path.name, events, fail=path.name == "second")

    return events, opener


def test_partial_setup_closes_earlier_resource(
    failing_opener: tuple[list[str], TextOpener],
) -> None:
    events, opener = failing_opener

    with pytest.raises(OSError, match="setup failed"):
        with open_text_files([Path("first"), Path("second")], opener=opener):
            pytest.fail("setup should not complete")

    assert events == ["enter:first", "enter:second", "exit:first"]


class RejectFlush(io.StringIO):
    def flush(self) -> None:
        raise OSError("output disconnected")


def test_write_reports_flush_failure() -> None:
    output = RejectFlush()

    with pytest.raises(OSError, match="output disconnected"):
        write_lines(output, ["build passed", "tests passed"])

    assert output.getvalue() == "build passed\ntests passed\n"
