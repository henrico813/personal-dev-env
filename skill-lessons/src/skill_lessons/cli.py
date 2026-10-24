"""Save corrections as lessons and render them into installed skills.

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
import subprocess
import sys
import tempfile
from pathlib import Path

from skill_lessons import __version__

MARKER = "<!-- skill-lessons: filled from the local store; do not edit -->"
HEADING = "## Lessons"
SCOPE_KEYS = ("project", "epic", "repo")
LESSON_KEYS = {"skill", "text", "repeat_of", *SCOPE_KEYS}
FOLD_WARNING_COUNT = 15
SKILL_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
THIS_EPIC = re.compile(r"\bthis epic\b", re.IGNORECASE)
THIS_EPIC_OR_TICKET = re.compile(r"\bthis (epic|ticket)\b", re.IGNORECASE)
# Placeholders such as KEY-<n> do not match.
TICKET_KEY = re.compile(r"\b[A-Z][A-Z0-9]+-[0-9]+\b")
WORD = re.compile(r"[a-z0-9']+")
COPIED_WORDS = 5


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


def read_request(file: str) -> dict:
    """Return the lesson request in ``file`` after checking its keys.

    ``-`` reads the request from standard input.

    Raises:
        LessonError: The file is unreadable, not a JSON object, has an
            unknown key or a non-string value, or lacks a skill or text.
    """
    try:
        if file == "-":
            file = "standard input"
            request = json.loads(sys.stdin.read())
        else:
            request = json.loads(Path(file).read_text(encoding="utf-8"))
    except OSError as error:
        raise LessonError(f"Cannot read {file}: {error.strerror}.") from error
    except json.JSONDecodeError as error:
        raise LessonError(f"{file} is not valid JSON.") from error
    if not isinstance(request, dict):
        raise LessonError(f"{file} must hold a JSON object.")
    unknown = sorted(set(request) - LESSON_KEYS)
    if unknown:
        raise LessonError(f"Unknown keys in {file}: {', '.join(unknown)}.")
    for key, value in request.items():
        if not isinstance(value, str):
            raise LessonError(f"{key} must be a string.")
    if "skill" not in request:
        raise LessonError(f"{file} needs a skill.")
    check_skill_name(request["skill"])
    request["text"] = " ".join(request.get("text", "").split())
    if not request["text"] and not request.get("repeat_of"):
        raise LessonError(f"{file} needs text, or repeat_of with a lesson ID.")
    return request


def add(file: str) -> str:
    """Save the lesson in ``file`` and re-render its skill.

    With ``repeat_of``, the existing lesson's repeat count goes up by
    one, and any given text or scope replaces the old value.

    Returns:
        A one-line result for the user.

    Raises:
        LessonError: The request is invalid, the skill is not installed
            or has no marker, ``repeat_of`` names no lesson, the text
            says "this epic" with no ``epic`` set, or its store is not
            valid JSON.
    """
    request = read_request(file)
    skill = request["skill"]
    path, text = read_marked_skill(skill)
    store = load_store(skill)
    scope = {key: request[key] for key in SCOPE_KEYS if request.get(key)}
    if request.get("repeat_of"):
        lesson = find_lesson(store, skill, request["repeat_of"])
        lesson["repeats"] += 1
        lesson.update(scope)
        if request["text"]:
            lesson["text"] = request["text"]
        message = (
            f"Counted a repeat of {lesson['id']} in {skill}; "
            f"its repeat count is now {lesson['repeats']}."
        )
    else:
        lesson = {
            "id": f"L{store['next_id']}",
            "text": request["text"],
            **scope,
            "repeats": 0,
        }
        store["next_id"] += 1
        store["lessons"].append(lesson)
        message = f"Saved {lesson['id']} to {skill}."
    # Without the key, the lesson would read as a rule for every epic.
    if THIS_EPIC.search(lesson["text"]) and not lesson.get("epic"):
        raise LessonError(
            'The text says "this epic" but no epic is set; ask the user '
            "for the epic key and add it as epic."
        )
    save_store(skill, store)
    render_skill(path, text, store)
    return message


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
            "whether to fold the stable ones into the skill's source; do "
            "not edit the skill on your own."
        )
    return "\n".join(lines)


def list_line(lesson: dict) -> str:
    """Return a lesson as ``list`` prints it.

    Example: ``L1 [repeats 0] (epic ABC-12) Children are Tasks.``
    """
    return f"{lesson['id']} [repeats {lesson['repeats']}] {describe(lesson)}"


def run_git(checkout: Path, *args: str) -> str:
    """Return the output of a git command run in ``checkout``.

    Raises:
        LessonError: git exits nonzero; the message has its last
            error line.
    """
    result = subprocess.run(
        ["git", "-C", str(checkout), *args], capture_output=True, text=True
    )
    if result.returncode != 0:
        detail = result.stderr.strip().splitlines() or ["no message"]
        raise LessonError(f"git {args[0]} failed in {checkout}: {detail[-1]}")
    return result.stdout


def require_pushed(checkout: Path) -> None:
    """Refuse unless the checkout's HEAD is on its remote branch.

    Fetches ``origin`` first. The remote branch is the upstream of the
    current branch, or ``origin/main`` when it has none.

    Raises:
        LessonError: The fetch failed or HEAD is not on that branch.
    """
    run_git(checkout, "fetch", "--quiet", "origin")
    try:
        remote = run_git(checkout, "rev-parse", "--abbrev-ref", "@{upstream}")
    except LessonError:
        remote = "origin/main"
    remote = remote.strip()
    try:
        run_git(checkout, "merge-base", "--is-ancestor", "HEAD", remote)
    except LessonError as error:
        raise LessonError(
            f"HEAD in {checkout} is not on {remote}; ask the user to push "
            "it, then retire again."
        ) from error


def retire(
    skill: str, lesson_ids: list[str], checkout: str | None = None
) -> str:
    """Remove lessons and re-render their skill.

    Every ID is checked before any lesson is removed, so a typo retires
    nothing. With ``checkout``, the git checkout at that path must
    have pushed its HEAD first; see ``require_pushed``.

    Returns:
        A one-line result for the user.

    Raises:
        LessonError: The skill is not installed or has no marker, an ID
            names no lesson, or the commit is not pushed.
    """
    check_skill_name(skill)
    path, text = read_marked_skill(skill)
    store = load_store(skill)
    lesson_ids = list(dict.fromkeys(lesson_ids))
    lessons = [find_lesson(store, skill, each) for each in lesson_ids]
    if checkout is not None:
        require_pushed(Path(checkout).expanduser())
    for lesson in lessons:
        store["lessons"].remove(lesson)
    save_store(skill, store)
    render_skill(path, text, store)
    return f"Retired {', '.join(lesson_ids)} from {skill}."


def make_source_path(skill: str) -> str:
    """Return the path of a skill's source, relative to the checkout."""
    return f"skills/{check_skill_name(skill)}/SKILL.md"


def read_source(checkout: Path, skill: str) -> str:
    """Return the source ``SKILL.md`` of ``skill`` in ``checkout``.

    Raises:
        LessonError: The file is missing or has no marker.
    """
    path = checkout / make_source_path(skill)
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as error:
        raise LessonError(f"{skill} has no source at {path}.") from error
    if find_marker(text.splitlines()) is None:
        raise LessonError(f"{skill} does not take lessons; it has no marker.")
    return text


def promote_start(skill: str, checkout: str) -> str:
    """Check that a promotion can start, then list the skill's lessons.

    Lessons with an ``epic`` scope or text saying "this epic" or "this
    ticket" are marked "likely stays a lesson": such a lesson rarely
    becomes skill text.

    Returns:
        One line per lesson, then the source line count.

    Raises:
        LessonError: ``checkout`` is not a clean git checkout, the
            source skill is missing or has no marker, or
            ``user.email`` differs from the last commit's author.
    """
    root = Path(checkout).expanduser()
    try:
        run_git(root, "rev-parse", "--is-inside-work-tree")
    except LessonError as error:
        raise LessonError(f"{root} is not a git checkout.") from error
    if run_git(root, "status", "--porcelain"):
        raise LessonError(f"{root} has uncommitted changes.")
    text = read_source(root, skill)
    try:
        email = run_git(root, "config", "user.email").strip()
    except LessonError:
        email = ""
    author = run_git(root, "log", "-1", "--format=%ae").strip()
    if email != author:
        raise LessonError(
            f"git user.email is {email or 'unset'}, but the last commit's "
            f"author is {author}; set this checkout's user.email."
        )
    lines = []
    for lesson in load_store(skill)["lessons"]:
        line = list_line(lesson)
        if lesson.get("epic") or THIS_EPIC_OR_TICKET.search(lesson["text"]):
            line += " (likely stays a lesson)"
        lines.append(line)
    if not lines:
        lines.append(f"{skill} has no lessons.")
    lines.append(
        f"{make_source_path(skill)} has {len(text.splitlines())} lines."
    )
    return "\n".join(lines)


def find_copied_words(added: str, lesson: str) -> str | None:
    """Return the first run of words ``added`` copies from ``lesson``.

    Words are lowercase runs of letters, digits and apostrophes, and a
    copy is ``COPIED_WORDS`` of them in a row.

    Example: "Show the full draft first" copies "show the full draft
    first" from "Always show the full draft first."
    """
    added_words = WORD.findall(added.lower())
    lesson_words = WORD.findall(lesson.lower())
    lesson_runs = {
        tuple(lesson_words[i : i + COPIED_WORDS])
        for i in range(len(lesson_words) - COPIED_WORDS + 1)
    }
    for i in range(len(added_words) - COPIED_WORDS + 1):
        run = tuple(added_words[i : i + COPIED_WORDS])
        if run in lesson_runs:
            return " ".join(run)
    return None


def find_section_problem(text: str) -> str | None:
    """Return why the source's Lessons section is wrong, or ``None``.

    The source must keep the marker and nothing under it, because
    rendering fills that section in the installed copy.
    """
    lines = text.splitlines()
    index = find_marker(lines)
    if index is None:
        return "The Lessons heading or marker is gone; restore it from HEAD."
    for line in lines[index + 1 :]:
        if line.startswith("## "):
            break
        if line.strip():
            return "The Lessons section in the source must stay empty."
    return None


def promote_check(
    skill: str, checkout: str, growth_reason: str | None = None
) -> str:
    """Check an edited source skill against its lessons and ``HEAD``.

    Collects every problem: a ticket key in added lines, a filled or
    missing Lessons section, a run of words copied from a lesson, and
    more lines than at ``HEAD`` without ``growth_reason``. Added lines
    are read as one run of words, so a copy wrapped over two lines is
    still found.

    Returns:
        The ``git diff -U1`` of the source and its line counts, for the
        agent to show the user unchanged.

    Raises:
        LessonError: One line per problem, or the source is missing.
    """
    root = Path(checkout).expanduser()
    relative = make_source_path(skill)
    path = root / relative
    try:
        text = path.read_text(encoding="utf-8")
    except FileNotFoundError as error:
        raise LessonError(f"{skill} has no source at {path}.") from error
    before = len(run_git(root, "show", f"HEAD:{relative}").splitlines())
    after = len(text.splitlines())
    diff_lines = run_git(
        root, "diff", "-U0", "HEAD", "--", relative
    ).splitlines()
    added = "\n".join(
        line[1:]
        for line in diff_lines
        if line.startswith("+") and not line.startswith("+++")
    )
    problems = []
    for key in dict.fromkeys(TICKET_KEY.findall(added)):
        problems.append(
            f"Added text has the ticket key {key}; use a placeholder such "
            "as KEY-<n>."
        )
    section = find_section_problem(text)
    if section:
        problems.append(section)
    for lesson in load_store(skill)["lessons"]:
        copied = find_copied_words(added, lesson["text"])
        if copied:
            problems.append(
                f'Added text copies "{copied}" from {lesson["id"]}; '
                "use your own words."
            )
    if after > before and not growth_reason:
        problems.append(
            f"The skill grew from {before} to {after} lines; tighten it, "
            "or pass --growth-reason."
        )
    if problems:
        raise LessonError("\n".join(problems))
    diff = run_git(root, "diff", "-U1", "HEAD", "--", relative)
    lines = [diff.rstrip("\n") or f"No changes to {relative}."]
    lines.append(f"Lines: {before} before, {after} after.")
    if growth_reason:
        lines.append(f"Growth reason: {growth_reason}")
    return "\n".join(lines)


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
    """Return the argument parser for the six commands."""
    parser = argparse.ArgumentParser(
        prog="skill-lessons",
        description="Save corrections as lessons in installed skills.",
    )
    parser.add_argument(
        "--version", action="version", version=f"%(prog)s {__version__}"
    )
    commands = parser.add_subparsers(dest="command", required=True)
    add_parser = commands.add_parser(
        "add", help="save a lesson, re-render its skill"
    )
    add_parser.add_argument("file", help="lesson JSON file, or - for stdin")
    list_parser = commands.add_parser(
        "list", help="show lessons and repeat counts"
    )
    list_parser.add_argument("skill")
    retire_parser = commands.add_parser(
        "retire", help="remove lessons, re-render"
    )
    retire_parser.add_argument("skill")
    retire_parser.add_argument("ids", nargs="+", metavar="id")
    retire_parser.add_argument(
        "--after-push",
        action="store_true",
        help="first check that --checkout pushed its HEAD",
    )
    retire_parser.add_argument("--checkout")
    commands.add_parser("render", help="re-fill every marked skill")
    start_parser = commands.add_parser(
        "promote-start", help="check a checkout, list lessons to promote"
    )
    start_parser.add_argument("skill")
    start_parser.add_argument("--checkout", required=True)
    check_parser = commands.add_parser(
        "promote-check", help="check an edited source skill, show its diff"
    )
    check_parser.add_argument("skill")
    check_parser.add_argument("--checkout", required=True)
    check_parser.add_argument("--growth-reason")
    return parser


def main(argv: list[str] | None = None) -> int:
    """Run the command line and return the exit code.

    Returns 0 on success and 1 when an operation is refused or fails.
    argparse exits with 2 on bad arguments.
    """
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.command == "retire" and args.after_push != bool(args.checkout):
        parser.error("retire needs --after-push and --checkout together")
    try:
        if args.command == "add":
            message = add(args.file)
        elif args.command == "list":
            message = list_lessons(args.skill)
        elif args.command == "retire":
            message = retire(args.skill, args.ids, args.checkout)
        elif args.command == "promote-start":
            message = promote_start(args.skill, args.checkout)
        elif args.command == "promote-check":
            message = promote_check(
                args.skill, args.checkout, args.growth_reason
            )
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
