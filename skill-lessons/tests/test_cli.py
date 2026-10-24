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


@pytest.fixture
def checkout(
    skills: Path, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> Path:
    """Return a clean checkout holding the demo source, pushed to origin.

    Origin is a local bare repository. The demo skill is also installed,
    and the user's git config is ignored so hooks or signing cannot
    change what a test sees.
    """
    monkeypatch.setenv("GIT_CONFIG_GLOBAL", str(tmp_path / "gitconfig"))
    monkeypatch.setenv("GIT_CONFIG_NOSYSTEM", "1")
    origin = tmp_path / "origin.git"
    root = tmp_path / "checkout"
    run_git(tmp_path, "init", "--quiet", "--bare", "-b", "main", str(origin))
    run_git(tmp_path, "init", "--quiet", "-b", "main", str(root))
    run_git(root, "config", "user.email", "dev@example.com")
    run_git(root, "config", "user.name", "Dev")
    (root / "skills" / "demo").mkdir(parents=True)
    (root / "skills" / "demo" / "SKILL.md").write_text(SOURCE)
    run_git(root, "add", ".")
    run_git(root, "commit", "--quiet", "-m", "Add demo")
    run_git(root, "remote", "add", "origin", str(origin))
    run_git(root, "push", "--quiet", "-u", "origin", "main")
    install(skills, "demo", SOURCE)
    return root


def run_git(where: Path, *args: str) -> None:
    """Run git in ``where`` and fail the test if it fails."""
    subprocess.run(["git", "-C", str(where), *args], check=True)


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

    def run_packed(
        *args: str, stdin: str | None = None
    ) -> subprocess.CompletedProcess:
        """Run the packed file and capture its output."""
        return subprocess.run(
            [sys.executable, "-I", str(packed), *args],
            capture_output=True,
            text=True,
            input=stdin,
        )

    return run_packed


def write_store(skills: Path, name: str, store: dict) -> Path:
    """Write ``store`` as the lesson store of ``name`` and return its path."""
    path = skills.parent / "store" / f"{name}.json"
    path.parent.mkdir(exist_ok=True)
    path.write_text(json.dumps(store, indent=2))
    return path


def add_lesson(run: Run, tmp_path: Path) -> subprocess.CompletedProcess:
    """Add the scoped lesson that ``LESSON_LINE`` renders."""
    lesson = tmp_path / "lesson.json"
    lesson.write_text(
        json.dumps(
            {
                "skill": "demo",
                "epic": "ABC-12",
                "text": "Children are Tasks; match the siblings' type.",
            }
        )
    )
    return run("add", str(lesson))


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


class TestAdd:
    """Saving lessons into installed skills with ``add``."""

    def test_add_writes_lesson_into_installed_skill(
        self, run: Run, skills: Path, tmp_path: Path
    ) -> None:
        """Adding a lesson puts it under the marker of the installed skill.

        Harnesses read only the installed SKILL.md, so a lesson that
        lands anywhere else is lost to the next session. The rest of the
        skill must stay byte for byte as it was.
        """
        skill = install(skills, "demo", SOURCE)

        result = add_lesson(run, tmp_path)

        assert result.returncode == 0, result.stderr
        assert skill.read_text() == render(LESSON_LINE)

    def test_add_refuses_skill_without_marker(
        self, run: Run, skills: Path, tmp_path: Path
    ) -> None:
        """A skill without the marker is refused and left unchanged.

        Only skills that opt in may collect lessons, so an agent cannot
        write lessons into an unrelated skill by naming it.
        """
        unmarked = "---\nname: demo\n---\n\n# Demo\n\n## Lessons\n"
        skill = install(skills, "demo", unmarked)

        result = add_lesson(run, tmp_path)

        assert result.returncode == 1
        assert "no marker" in result.stderr
        assert skill.read_text() == unmarked

    def test_add_dash_reads_standard_input(
        self, run: Run, skills: Path
    ) -> None:
        """``add -`` saves the lesson piped in, as skills send it.

        Skills save lessons through a pipe or heredoc, so if standard
        input were not read, every "remember" would fail.
        """
        skill = install(skills, "demo", SOURCE)
        lesson = {"skill": "demo", "text": "Ask before you link."}

        result = run("add", "-", stdin=json.dumps(lesson))

        assert result.returncode == 0, result.stderr
        assert skill.read_text() == render("- Ask before you link. [L1]\n")

    def test_add_refuses_this_epic_without_key(
        self, run: Run, skills: Path
    ) -> None:
        """A lesson about "this epic" needs the epic key.

        Saved without it, the lesson reads as a rule for every epic and
        could be promoted into the skill for all of them.
        """
        skill = install(skills, "demo", SOURCE)
        lesson = {"skill": "demo", "text": "In this epic, children are Tasks."}

        result = run("add", "-", stdin=json.dumps(lesson))

        assert result.returncode == 1
        assert "epic" in result.stderr
        assert skill.read_text() == SOURCE


class TestRetire:
    """Removing lessons with ``retire``."""

    def test_retire_removes_lesson_from_installed_skill(
        self, run: Run, skills: Path, tmp_path: Path
    ) -> None:
        """Retiring a lesson takes it out of the installed skill.

        Retiring is how a lesson leaves once it is folded into the skill
        text or turns out wrong; if it stayed rendered, agents would
        keep following it.
        """
        skill = install(skills, "demo", SOURCE)
        assert add_lesson(run, tmp_path).returncode == 0

        result = run("retire", "demo", "L1")

        assert result.returncode == 0, result.stderr
        assert skill.read_text() == SOURCE

    @pytest.mark.parametrize(
        ("pushed", "returncode", "installed"),
        [
            pytest.param(True, 0, SOURCE, id="pushed"),
            pytest.param(False, 1, render(LESSON_LINE), id="not-pushed"),
        ],
    )
    def test_after_push_retires_only_when_pushed(
        self,
        run: Run,
        checkout: Path,
        skills: Path,
        tmp_path: Path,
        pushed: bool,
        returncode: int,
        installed: str,
    ) -> None:
        """``retire --after-push`` keeps lessons until origin has HEAD.

        The promotion commit is the only other copy of a promoted rule,
        so retiring before the push lands could lose the rule.
        """
        assert add_lesson(run, tmp_path).returncode == 0
        source = checkout / "skills" / "demo" / "SKILL.md"
        source.write_text(SOURCE.replace("work.", "work well."))
        run_git(checkout, "commit", "--quiet", "-am", "Promote")
        if pushed:
            run_git(checkout, "push", "--quiet")

        result = run(
            "retire", "demo", "L1", "--after-push", "--checkout", str(checkout)
        )

        assert result.returncode == returncode, result.stderr
        assert (skills / "demo" / "SKILL.md").read_text() == installed
        assert run("render").returncode == 0
        assert (skills / "demo" / "SKILL.md").read_text() == installed


class TestRender:
    """Refilling installed skills with ``render``."""

    def test_render_refills_lessons_keeping_later_sections(
        self, run: Run, skills: Path, tmp_path: Path
    ) -> None:
        """Rendering a fresh copy refills every lesson and keeps what follows.

        An install replaces each installed skill with its empty source
        copy, so without this every reinstall would silently drop saved
        lessons. Two lessons check that none is lost and the order holds,
        and ``SOURCE``'s ``## After`` section must survive unchanged.
        """
        skill = install(skills, "demo", SOURCE)
        assert add_lesson(run, tmp_path).returncode == 0
        second = {"skill": "demo", "text": "Ask before you link."}
        assert run("add", "-", stdin=json.dumps(second)).returncode == 0
        install(skills, "demo", SOURCE)

        result = run("render")

        assert result.returncode == 0, result.stderr
        assert skill.read_text() == render(
            LESSON_LINE, "- Ask before you link. [L2]\n"
        )

    def test_render_fills_marked_skills_past_unmarked(
        self, run: Run, skills: Path, tmp_path: Path
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
        assert add_lesson(run, tmp_path).returncode == 0
        other = {"skill": "zeta", "text": "Ask before you link."}
        assert run("add", "-", stdin=json.dumps(other)).returncode == 0
        install(skills, "demo", SOURCE)
        install(skills, "zeta", SOURCE)

        result = run("render")

        assert result.returncode == 0, result.stderr
        assert alpha.read_text() == unmarked
        assert demo.read_text() == render(LESSON_LINE)
        assert zeta.read_text() == render("- Ask before you link. [L1]\n")


class TestStore:
    """Adding to a store that already has lessons."""

    def test_repeat_updates_old_lesson_in_place(
        self, run: Run, skills: Path
    ) -> None:
        """``repeat_of`` updates a saved lesson in place.

        A repeated correction must raise the count and take the new
        wording on the lesson the user named; adding a second lesson
        would show the rule twice.
        """
        store = write_store(
            skills,
            "demo",
            {
                "next_id": 2,
                "lessons": [
                    {
                        "id": "L1",
                        "text": "Ask before you link.",
                        "epic": "ABC-12",
                        "repeats": 1,
                    }
                ],
            },
        )
        install(skills, "demo", SOURCE)
        repeat = {
            "skill": "demo",
            "repeat_of": "L1",
            "text": "Ask before you add a link.",
        }

        result = run("add", "-", stdin=json.dumps(repeat))

        assert result.returncode == 0, result.stderr
        assert json.loads(store.read_text()) == {
            "next_id": 2,
            "lessons": [
                {
                    "id": "L1",
                    "text": "Ask before you add a link.",
                    "epic": "ABC-12",
                    "repeats": 2,
                }
            ],
        }


class TestPromoteStart:
    """Starting a promotion with ``promote-start``."""

    def test_marks_epic_lessons_as_likely_staying(
        self, run: Run, checkout: Path, tmp_path: Path
    ) -> None:
        """``promote-start`` tags an epic-scoped lesson, not a general one.

        An agent folding lessons into a skill leaves tagged ones as
        lessons, so a missing or changed tag would let a lesson tied to
        one epic become a rule for every task.
        """
        assert add_lesson(run, tmp_path).returncode == 0
        general = {"skill": "demo", "text": "Ask before you link."}
        assert run("add", "-", stdin=json.dumps(general)).returncode == 0

        result = run("promote-start", "demo", "--checkout", str(checkout))

        assert result.returncode == 0, result.stderr
        lines = result.stdout.splitlines()
        assert lines[0].startswith("L1 ")
        assert lines[0].endswith(" (likely stays a lesson)")
        assert lines[1] == "L2 [repeats 0] Ask before you link."


class TestPromoteCheck:
    """Checking an edited source with ``promote-check``."""

    @pytest.mark.parametrize(
        ("new_text", "problem"),
        [
            pytest.param(
                "Show the full draft before you create.",
                'copies "show the full draft before" from L1',
                id="copied-wording",
            ),
            pytest.param(
                "Link DEMO-7 first.",
                "ticket key DEMO-7",
                id="ticket-key",
            ),
            pytest.param(
                "Do the work.\nThen check it.",
                "grew from 14 to 15 lines",
                id="unexplained-growth",
            ),
        ],
    )
    def test_reports_mechanical_rule_breaks(
        self, run: Run, checkout: Path, new_text: str, problem: str
    ) -> None:
        """Each mechanical promotion rule fails the check when broken.

        Lesson wording and ticket keys may carry private details that
        must not reach git. Unexplained growth means rules were appended
        instead of folded in.
        """
        lesson = {
            "skill": "demo",
            "text": "Always show the full draft before you create a ticket.",
        }
        assert run("add", "-", stdin=json.dumps(lesson)).returncode == 0
        source = checkout / "skills" / "demo" / "SKILL.md"
        source.write_text(SOURCE.replace("Do the work.", new_text))

        result = run("promote-check", "demo", "--checkout", str(checkout))

        assert result.returncode == 1
        assert problem in result.stderr

    def test_clean_edit_prints_diff_and_counts(
        self, run: Run, checkout: Path
    ) -> None:
        """A clean edit passes and prints what the user approves.

        The agent relays this output as is, so it must hold the real
        diff and both line counts.
        """
        source = checkout / "skills" / "demo" / "SKILL.md"
        source.write_text(SOURCE.replace("work.", "work well."))

        result = run("promote-check", "demo", "--checkout", str(checkout))

        assert result.returncode == 0, result.stderr
        assert "-Do the work.\n+Do the work well.\n" in result.stdout
        assert result.stdout.endswith("Lines: 14 before, 14 after.\n")
