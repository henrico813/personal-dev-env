from pathlib import Path

import pytest

from python_skill_examples.object_store import (
    LocalObjectStore,
    MemoryObjectStore,
    ObjectStore,
)


@pytest.fixture(
    params=[
        pytest.param("memory", id="memory"),
        pytest.param("local", id="local"),
    ]
)
def store(request: pytest.FixtureRequest, tmp_path: Path) -> ObjectStore:
    if request.param == "memory":
        return MemoryObjectStore()
    return LocalObjectStore(tmp_path / "objects")


def test_stores_round_trip(store: ObjectStore) -> None:
    store.put("report.bin", b"passed")

    assert store.get("report.bin") == b"passed"


def test_missing_objects_raise_key_error(store: ObjectStore) -> None:
    with pytest.raises(KeyError, match="missing.bin"):
        store.get("missing.bin")
