"""Shared setup for the checker tests."""

import os
import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path

import pytest

BUILD = Path(__file__).parents[1] / "build.py"


@pytest.fixture(scope="session", autouse=True)
def packed_checker(tmp_path_factory: pytest.TempPathFactory) -> Iterator[Path]:
    """Build ``pde-comment-checker`` once and put it first on ``PATH``.

    Tests run the file pde-installer installs, so a module the build
    leaves out fails the suite instead of a machine's first check.
    """
    folder = tmp_path_factory.mktemp("bin")
    packed = folder / "pde-comment-checker"
    subprocess.run([sys.executable, "-I", str(BUILD), str(packed)], check=True)
    with pytest.MonkeyPatch.context() as patch:
        patch.setenv("PATH", f"{folder}{os.pathsep}{os.environ['PATH']}")
        yield packed
