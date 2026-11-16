"""Render saved lessons into installed skills.

A skill opts in with a ``## Lessons`` heading followed directly by
``MARKER``. Lessons for a skill are stored in
``$SKILL_LESSONS_HOME/<skill>.json`` and rendered under the marker in
``$SKILL_LESSONS_SKILLS_DIR/<skill>/SKILL.md``.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path

from skill_lessons import __version__

MARKER = "<!-- skill-lessons: filled from the local store; do not edit -->"
HEADING = "## Lessons"
SCOPE_KEYS = ("project", "epic", "repo")
FOLD_WARNING_COUNT = 15
SKILL_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")


class LessonError(Exception):
    """A refused operation whose message can be shown to the user."""


def find_skills_dir() -> Path:
    """Return the folder that holds installed skills."""
    default = "~/.agents/skills"
    return Path(
        os.environ.get("SKILL_LESSONS_SKILLS_DIR", default)
    ).expanduser()


def find_store_dir() -> Path:
    """Return the folder that holds one lesson store per skill."""
    default = "~/.local/share/skill-lessons"
    return Path(os.environ.get("SKILL_LESSONS_HOME", default)).expanduser()


def check_skill_name(skill: object) -> str:
    """Return ``skill`` if it is a plain folder name.

    The name becomes part of two file paths, so separators and leading
    dots are refused.

    Raises:
        LessonError: The name is not a string or not a plain name.
    """
    if not isinstance(skill, str) or not SKILL_NAME.fullmatch(skill):
        raise LessonError(f"{skill!r} is not a valid skill name.")
    return skill


def write_atomic(path: Path, text: str) -> None:
    """Replace ``path`` with ``text`` so readers never see a partial file.

    An existing file keeps its permission bits.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temp_name = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as file:
            file.write(text)
        if path.exists():
            shutil.copymode(path, temp_name)
        os.replace(temp_name, path)
    except BaseException:
        Path(temp_name).unlink(missing_ok=True)
        raise


def load_store(skill: str) -> dict:
    """Return the lesson store for ``skill``, or an empty one.

    Raises:
        LessonError: The store file exists but is not valid JSON.
    """
    path = find_store_dir() / f"{skill}.json"
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return {"next_id": 1, "lessons": []}
    try:
        return json.loads(text)
    except json.JSONDecodeError as error:
        raise LessonError(f"The store {path} is not valid JSON.") from error


def save_store(skill: str, store: dict) -> None:
    """Write the lesson store for ``skill``."""
    path = find_store_dir() / f"{skill}.json"
    write_atomic(path, json.dumps(store, indent=2) + "\n")


def find_lesson(store: dict, skill: str, lesson_id: str) -> dict:
    """Return the lesson with ``lesson_id``.

    Raises:
        LessonError: No lesson has that ID.
    """
    for lesson in store["lessons"]:
        if lesson["id"] == lesson_id:
            return lesson
    raise LessonError(f"{skill} has no lesson {lesson_id}.")


def find_marker(lines: list[str]) -> int | None:
    """Return the index of the marker line under the Lessons heading.

    Returns ``None`` when there is none.
    """
    for index in range(1, len(lines)):
        if (
            lines[index].rstrip() == MARKER
            and lines[index - 1].rstrip() == HEADING
        ):
            return index
    return None


def read_marked_skill(skill: str) -> tuple[Path, str]:
    """Return the installed ``SKILL.md`` path and text for ``skill``.

    Raises:
        LessonError: The skill is not installed or has no marker.
    """
    path = find_skills_dir() / skill / "SKILL.md"
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as error:
        raise LessonError(f"{skill} is not installed at {path}.") from error
    if find_marker(text.splitlines()) is None:
        raise LessonError(f"{skill} does not take lessons; it has no marker.")
    return path, text


def describe(lesson: dict) -> str:
    """Return a lesson's text, led by its scope when one is set.

    Example: ``(epic ABC-12) Children are Tasks.``
    """
    scope = ", ".join(
        f"{key} {lesson[key]}" for key in SCOPE_KEYS if lesson.get(key)
    )
    prefix = f"({scope}) " if scope else ""
    return prefix + lesson["text"]


def format_lesson(lesson: dict) -> str:
    """Return a lesson as a Markdown list item ending in its ID."""
    return f"- {describe(lesson)} [{lesson['id']}]"


def render_text(text: str, lessons: list[dict]) -> str:
    """Return ``text`` with the marked Lessons section filled.

    Everything from the marker to the next ``## `` heading, or the end
    of the file, is replaced. One blank line is written before that heading.
    """
    lines = text.splitlines(keepends=True)
    index = find_marker([line.rstrip("\n") for line in lines])
    # Callers read the skill with read_marked_skill or check the marker.
    assert index is not None
    end = next(
        (
            i
            for i in range(index + 1, len(lines))
            if lines[i].startswith("## ")
        ),
        len(lines),
    )
    section = [MARKER + "\n"]
    section += [format_lesson(lesson) + "\n" for lesson in lessons]
    if end < len(lines):
        section.append("\n")
    return "".join(lines[:index] + section + lines[end:])


def render_skill(path: Path, text: str, store: dict) -> None:
    """Fill the Lessons section of the installed skill at ``path``."""
    rendered = render_text(text, store["lessons"])
    if rendered != text:
        write_atomic(path, rendered)


def list_lessons(skill: str) -> str:
    """List the lessons of ``skill`` with their repeat counts.

    Returns:
        One line per lesson, plus a warning line at 15 or more.

    Raises:
        LessonError: The skill name is invalid or its store is not
            valid JSON.
    """
    check_skill_name(skill)
    lessons = load_store(skill)["lessons"]
    if not lessons:
        return f"{skill} has no lessons."
    lines = [list_line(lesson) for lesson in lessons]
    if len(lessons) >= FOLD_WARNING_COUNT:
        lines.append(
            f"Warning: {skill} has {len(lessons)} lessons. Ask the user "
            "whether to move the stable ones into the skill's text with "
            "skill-fix; do not edit the skill on your own."
        )
    return "\n".join(lines)


def list_line(lesson: dict) -> str:
    """Return a lesson as ``list`` prints it.

    Example: ``L1 [repeats 0] (epic ABC-12) Children are Tasks.``
    """
    return f"{lesson['id']} [repeats {lesson['repeats']}] {describe(lesson)}"


def retire(skill: str, lesson_ids: list[str]) -> str:
    """Remove lessons and re-render their skill.

    Every ID is checked before any lesson is removed, so a typo retires
    nothing.

    Returns:
        A one-line result for the user.

    Raises:
        LessonError: The skill is not installed or has no marker, or an
            ID names no lesson.
    """
    check_skill_name(skill)
    path, text = read_marked_skill(skill)
    store = load_store(skill)
    lesson_ids = list(dict.fromkeys(lesson_ids))
    lessons = [find_lesson(store, skill, each) for each in lesson_ids]
    for lesson in lessons:
        store["lessons"].remove(lesson)
    save_store(skill, store)
    render_skill(path, text, store)
    return f"Retired {', '.join(lesson_ids)} from {skill}."


def render_all() -> str:
    """Re-fill every installed skill that has the marker.

    Skills without the marker are not written.

    Returns:
        A one-line count of the skills rendered.

    Raises:
        LessonError: The skills folder does not exist, or a store is
            not valid JSON; skills before it are already rendered.
    """
    root = find_skills_dir()
    if not root.is_dir():
        raise LessonError(f"There is no skills folder at {root}.")
    count = 0
    for path in sorted(root.glob("*/SKILL.md")):
        text = path.read_text(encoding="utf-8")
        if find_marker(text.splitlines()) is None:
            continue
        render_skill(path, text, load_store(path.parent.name))
        count += 1
    noun = "skill" if count == 1 else "skills"
    return f"Rendered lessons into {count} {noun}."


def build_parser() -> argparse.ArgumentParser:
    """Return the argument parser for the three commands."""
    parser = argparse.ArgumentParser(
        prog="skill-lessons",
        description="Show, retire and render saved lessons in skills.",
    )
    parser.add_argument(
        "--version", action="version", version=f"%(prog)s {__version__}"
    )
    commands = parser.add_subparsers(dest="command", required=True)
    list_parser = commands.add_parser(
        "list", help="show lessons and repeat counts"
    )
    list_parser.add_argument("skill")
    retire_parser = commands.add_parser(
        "retire", help="remove lessons, re-render"
    )
    retire_parser.add_argument("skill")
    retire_parser.add_argument("ids", nargs="+", metavar="id")
    commands.add_parser("render", help="re-fill every marked skill")
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the command line and return the exit code.

    Returns 0 on success and 1 when an operation is refused or fails.
    argparse exits with 2 on bad arguments.
    """
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "list":
            message = list_lessons(args.skill)
        elif args.command == "retire":
            message = retire(args.skill, args.ids)
        else:
            message = render_all()
    except LessonError as error:
        print(error, file=sys.stderr)
        return 1
    except OSError as error:
        print(f"{error.strerror}: {error.filename}.", file=sys.stderr)
        return 1
    print(message)
    return 0


if __name__ == "__main__":
    sys.exit(main())
