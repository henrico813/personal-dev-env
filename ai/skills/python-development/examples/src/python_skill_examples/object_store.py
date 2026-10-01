"""Memory and filesystem implementations of a small object store."""

from pathlib import Path
from typing import Protocol


class ObjectStore(Protocol):
    """Store and retrieve bytes by key."""

    def put(self, key: str, value: bytes) -> None: ...

    def get(self, key: str) -> bytes: ...


class MemoryObjectStore:
    """Store objects in process memory."""

    def __init__(self) -> None:
        self._objects: dict[str, bytes] = {}

    def put(self, key: str, value: bytes) -> None:
        self._objects[key] = value

    def get(self, key: str) -> bytes:
        return self._objects[key]


class LocalObjectStore:
    """Store objects as files beneath one directory."""

    def __init__(self, root: Path) -> None:
        self._root = root
        self._root.mkdir(parents=True, exist_ok=True)

    def put(self, key: str, value: bytes) -> None:
        (self._root / key).write_bytes(value)

    def get(self, key: str) -> bytes:
        try:
            return (self._root / key).read_bytes()
        except FileNotFoundError as error:
            raise KeyError(key) from error
