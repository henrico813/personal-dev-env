import pytest

from python_skill_examples import directory


def test_patch_replaces_lookup(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        directory,
        "listdir",
        lambda path: ["report.txt", ".cache", "notes.txt"],
    )

    result = directory.visible_entries("ignored")

    assert result == ["notes.txt", "report.txt"]


def test_dependency_avoids_patch() -> None:
    entries = ["report.txt", ".cache", "notes.txt"]

    result = directory.visible_entries_with("ignored", lambda path: entries)

    assert result == ["notes.txt", "report.txt"]
