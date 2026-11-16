"""Tests that run the packed file ``build.py`` writes."""

import json
import os
import re
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path

import pytest

MARKER = "<!-- skill-lessons: filled from the local store; do not edit -->"
# The section after Lessons checks that rendering stops at the next
# heading and keeps the blank line before it.
SOURCE = (
    "---\nname: demo\n---\n\n# Demo\n\nDo the work.\n\n"
    f"## Lessons\n{MARKER}\n\n## After\n\nKeep this.\n"
)
LESSON = {
    "id": "L1",
    "text": "Children are Tasks; match the siblings' type.",
    "epic": "ABC-12",
    "repeats": 0,
}
LESSON_LINE = (
    "- (epic ABC-12) Children are Tasks; match the siblings' type. [L1]\n"
)
BUILD = Path(__file__).parents[1] / "build.py"
INIT = Path(__file__).parents[1] / "src" / "skill_lessons" / "__init__.py"


def render(*lessons: str) -> str:
    """Return ``SOURCE`` with ``lessons`` filled in under the marker."""
    return SOURCE.replace(MARKER + "\n", MARKER + "\n" + "".join(lessons))


Run = Callable[..., subprocess.CompletedProcess]


@pytest.fixture(scope="session")
def packed(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Return the file ``build.py`` packs, built once per test session.

    Every test runs this file rather than the source, so a packing
    mistake, such as losing the exit code, fails the whole suite.
    """
    output = tmp_path_factory.mktemp("build") / "skill-lessons"
    subprocess.run([sys.executable, "-I", str(BUILD), str(output)], check=True)
    return output


@pytest.fixture
def skills(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Return an empty skills folder, with the store in another folder.

    Both overrides are set so a test never reads or writes the real
    skills or the real lesson store.
    """
    monkeypatch.setenv("SKILL_LESSONS_SKILLS_DIR", str(tmp_path / "skills"))
    monkeypatch.setenv("SKILL_LESSONS_HOME", str(tmp_path / "store"))
    (tmp_path / "skills").mkdir()
    return tmp_path / "skills"


def install(skills: Path, name: str, text: str) -> Path:
    """Write ``text`` as the installed ``SKILL.md`` of ``name``."""
    path = skills / name / "SKILL.md"
    path.parent.mkdir(exist_ok=True)
    path.write_text(text)
    return path


@pytest.fixture
def run(packed: Path, skills: Path) -> Run:
    """Return a function that runs the packed file with given arguments.

    It runs under the test's Python with ``-I``, as the file's
    ``#!/usr/bin/python3 -I`` line does, so CI checks the oldest
    supported version. Requiring ``skills`` keeps every run off the real
    store.
    """

    def run_packed(*args: str) -> subprocess.CompletedProcess:
        """Run the packed file and capture its output."""
        return subprocess.run(
            [sys.executable, "-I", str(packed), *args],
            capture_output=True,
            text=True,
        )

    return run_packed


def write_store(skills: Path, name: str, *lessons: dict) -> None:
    """Save ``lessons`` as the lesson store of ``name``."""
    path = skills.parent / "store" / f"{name}.json"
    path.parent.mkdir(exist_ok=True)
    store = {"next_id": len(lessons) + 1, "lessons": list(lessons)}
    path.write_text(json.dumps(store, indent=2))


class TestBuild:
    """The packed file and the script that builds it."""

    def test_packed_file_prints_version_ignoring_pythonpath(
        self, packed: Path, tmp_path: Path
    ) -> None:
        """The packed file runs by its ``#!`` line and ignores ``PYTHON*``.

        ``skill-lessons --version`` is how a caller checks that the tool
        works, without naming an interpreter. The other tests run the
        file with the test's Python, so only this one runs its ``#!``
        line. A ``PYTHONPATH`` holding a broken ``json.py`` fails the run
        unless that line passes ``-I``.
        """
        version = re.search(
            r'^__version__ = "([^"]+)"', INIT.read_text(), re.MULTILINE
        )
        assert version
        (tmp_path / "json.py").write_text("raise RuntimeError('PYTHONPATH')")
        env = {**os.environ, "PYTHONPATH": str(tmp_path)}

        result = subprocess.run(
            [str(packed), "--version"], capture_output=True, text=True, env=env
        )

        assert result.returncode == 0, result.stderr
        assert result.stdout == f"skill-lessons {version.group(1)}\n"


class TestRetire:
    """Removing lessons with ``retire``."""

    def test_retire_removes_lesson_from_installed_skill(
        self, run: Run, skills: Path
    ) -> None:
        """Retiring a lesson takes it out of the installed skill.

        Retiring is how a lesson leaves once it is folded into the skill
        text or turns out wrong; if it stayed rendered, agents would
        keep following it.
        """
        skill = install(skills, "demo", render(LESSON_LINE))
        write_store(skills, "demo", LESSON)

        result = run("retire", "demo", "L1")

        assert result.returncode == 0, result.stderr
        assert skill.read_text() == SOURCE

class TestRender:
    """Refilling installed skills with ``render``."""

    def test_render_refills_lessons_keeping_later_sections(
        self, run: Run, skills: Path
    ) -> None:
        """Rendering a fresh copy refills every lesson and keeps what follows.

        An install replaces each installed skill with its empty source
        copy, so without this every reinstall would silently drop saved
        lessons. Two lessons check that none is lost and the order holds,
        and ``SOURCE``'s ``## After`` section must survive unchanged.
        """
        skill = install(skills, "demo", SOURCE)
        second = {"id": "L2", "text": "Ask before you link.", "repeats": 0}
        write_store(skills, "demo", LESSON, second)

        result = run("render")

        assert result.returncode == 0, result.stderr
        assert skill.read_text() == render(
            LESSON_LINE, "- Ask before you link. [L2]\n"
        )

    def test_render_fills_marked_skills_past_unmarked(
        self, run: Run, skills: Path
    ) -> None:
        """``render`` skips an unmarked skill and fills each marked one.

        Most installed skills take no lessons, and render runs after
        every install, so it must neither edit them nor stop at the
        first one. ``alpha`` sorts before ``demo``, and ``zeta`` has its
        own lesson, so each skill must get its own store.
        """
        unmarked = "---\nname: alpha\n---\n\n# Alpha\n\n## Lessons\n"
        alpha = install(skills, "alpha", unmarked)
        demo = install(skills, "demo", SOURCE)
        zeta = install(skills, "zeta", SOURCE)
        write_store(skills, "demo", LESSON)
        other = {"id": "L1", "text": "Ask before you link.", "repeats": 0}
        write_store(skills, "zeta", other)

        result = run("render")

        assert result.returncode == 0, result.stderr
        assert alpha.read_text() == unmarked
        assert demo.read_text() == render(LESSON_LINE)
        assert zeta.read_text() == render("- Ask before you link. [L1]\n")
