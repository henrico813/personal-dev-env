"""Check a checkout's skills for mistakes that break them.

Prints up to ten problems of each kind, then a count line, and exits
1 when any problem is an error. Files Git ignores are skipped. Read
``check_skills`` first. Its docstring lists each check in the order
it runs.

Example:
    python3 ~/.agents/skills/skill-development/scripts/skill_check.py
    python3 skill_check.py --base main
"""

import argparse
import os
import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

INSTALLED_SKILLS = Path.home() / ".agents/skills"
MAX_WIDTH = 72
REPEAT_WORDS = 8
MAX_SKILL_LINES = 500
MAX_REFERENCE_LINES = 100
MAX_SHOWN = 10
REFERENCE_FOLDERS = ("references", "reference")
LESSONS_MARKER = "<!-- skill-lessons:"
# Remembering corrections and Lessons hold the same words in every
# skill on purpose.
SHARED_SECTIONS = ("## Remembering corrections", "## Lessons")
# A link target may start on the next line, as in code-documentation.
LINK = re.compile(r"\]\(\s*([^)\s]+)\s*\)")
SKILL_NAME = re.compile(
    r"`([a-z0-9]+(?:-[a-z0-9]+)*)`\s+skill\b"
    r"|\bskill\s+`([a-z0-9]+(?:-[a-z0-9]+)*)`"
    r"|\]\(\.\./([a-z0-9-]+)/SKILL\.md"
)
LESSONS_NAME = re.compile(r"with\s+the\s+skill\s+name\s+`([^`]+)`")
INLINE_CODE = re.compile(r"`[^`\n]*`")
FENCE = re.compile(r"^\s*(```|~~~)")
LIST_MARKER = re.compile(r"^\s*(?:[-*+]|\d+[.)])?\s*")
WORD = re.compile(r"[a-z0-9][a-z0-9'_./-]*")
HUNK = re.compile(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@")
HEADING = re.compile(r"^#{1,6}\s+(?:\d+\.\s+)?(.+?)[\s#]*$")
CONTENTS = re.compile(r"^#{1,6}\s+(?:table of\s+)?contents\s*$", re.IGNORECASE)
# "step 2 of 3" counts steps, so a number after "of" is not a pointer.
STEP_POINTER = re.compile(
    r"\bsteps?\s+\d+(?:\s+(?:to|and|or)\s+\d+)?\s+of\b(?!\s+\d)",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class Problem:
    """One finding, printed as ``path:line: kind: message``.

    Its kind is "error", which fails the check, "repeat", which the
    author judges, or "grown", which the pull request explains.
    """

    path: Path
    line: int
    kind: str
    message: str


@dataclass(frozen=True)
class Skills:
    """The skills in a skills folder and the Markdown files to check.

    The checked files are each skill's Markdown files and the
    ``AGENTS.md`` next to the skills folder.

    Attributes:
        folders: Each skill's folder, sorted.
        files: Every file from ``list_skill_files``, Markdown or not.
        names: Skills in the checkout or installed in
            ``~/.agents/skills``.
        prose: Lines of each checked file from ``blank_code_lines``.
        owners: The skill, or ``AGENTS.md``, each checked file belongs
            to.
    """

    folders: list[Path]
    files: list[Path]
    names: set[str]
    prose: dict[Path, list[str]]
    owners: dict[Path, str]


def check_skills(skills_dir: Path, base: str | None) -> list[Problem]:
    """Run every check on the skills in skills_dir, in this order.

    1. A link to a file that does not exist is an error.
    2. A skill name with no skill folder is an error.
    3. A reference file that SKILL.md does not link is an error.
    4. A frontmatter name that is missing or differs from the folder
       is an error.
    5. A Lessons section that skill-lessons cannot fill, or that is
       missing from a skill whose Remembering corrections line names
       a skill, is an error.
    6. A Remembering corrections line that names another skill, or
       that is missing from a skill with a Lessons section, is an
       error.

    With base, more checks report only on files and lines changed
    since base, so a problem already on main fails no unrelated
    change.

    7. An added line over 72 columns that could wrap is an error.
    8. An added line that points at a step number, such as "step 6
       of", is an error.
    9. A SKILL.md body that the change made longer than 500 lines is
       an error.
    10. A changed reference file over 100 lines with no Contents
        heading is an error.
    11. A quote of a heading that the change removed or renamed is an
        error, unless some file still has that heading.
    12. Eight words in a row that another skill or ``AGENTS.md`` also
        has are a repeat, for the author to judge.
    13. A SKILL.md whose body has more lines than in base is reported
        as grown, for the pull request to give a reason.

    Checks 1, 2, 7, 8, 11 and 12 also read the ``AGENTS.md`` next to
    skills_dir.

    Args:
        skills_dir: Folder with one subfolder per skill.
        base: Commit to compare with, or None to skip the checks of
            changes.

    Returns:
        The problems in the order of the checks, so errors come before
        repeats and grown skills.

    Raises:
        subprocess.CalledProcessError: When git fails, such as outside
            a checkout or for a base that does not exist.
    """
    skills = read_skills(skills_dir)
    problems = (
        check_links(skills)
        + check_skill_names(skills)
        + check_references_linked(skills)
        + check_frontmatter_names(skills)
        + check_lessons_sections(skills)
        + check_corrections_names(skills)
    )
    if base is None:
        return problems
    added = find_added_lines(skills_dir, base, list(skills.prose))
    before = read_changed_base_texts(skills_dir, base)
    return (
        problems
        + check_line_widths(skills, added)
        + check_step_pointers(skills, added)
        + check_skill_sizes(skills, before, added)
        + check_reference_contents(skills, added)
        + check_removed_headings(skills, before)
        + find_repeats(skills, added)
        + find_grown_skills(skills, before)
    )


def read_skills(skills_dir: Path) -> Skills:
    """Read the skills in skills_dir and the prose of their files.

    Only files from ``list_skill_files`` and the ``AGENTS.md`` beside
    skills_dir are read. Skills installed in ``~/.agents/skills`` count
    as known names, since a skill may name one that another repository
    holds.

    Args:
        skills_dir: Folder with one subfolder per skill.

    Returns:
        The skills, with each checked file's lines from
        ``blank_code_lines`` and the skill, or ``AGENTS.md``, it
        belongs to.

    Raises:
        subprocess.CalledProcessError: When skills_dir is not in a Git
            checkout.
    """
    files = list_skill_files(skills_dir)
    folders = sorted(
        path.parent
        for path in files
        if path.name == "SKILL.md" and path.parent.parent == skills_dir
    )
    names = {folder.name for folder in folders}
    names |= {p.parent.name for p in INSTALLED_SKILLS.glob("*/SKILL.md")}
    owners = {}
    for path in files:
        folder = skills_dir / path.relative_to(skills_dir).parts[0]
        if path.suffix == ".md" and folder in folders:
            owners[path] = folder.name
    agents = skills_dir.parent / "AGENTS.md"
    if agents.is_file():
        owners[agents] = "AGENTS.md"
    prose = {
        path: blank_code_lines(path.read_text(encoding="utf-8"))
        for path in owners
    }
    return Skills(folders, files, names, prose, owners)


def list_skill_files(skills_dir: Path) -> list[Path]:
    """List the files in skills_dir that Git tracks or does not ignore.

    Ignored files, such as packages installed for a skill's example,
    are not part of any skill.

    Args:
        skills_dir: Folder with one subfolder per skill.

    Returns:
        Each listed file that exists, sorted.

    Raises:
        subprocess.CalledProcessError: When skills_dir is not in a Git
            checkout.
    """
    output = run_git(
        skills_dir,
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
    )
    paths = {skills_dir / name for name in output.split("\0") if name}
    return sorted(path for path in paths if path.is_file())


def blank_code_lines(text: str) -> list[str]:
    """Return the file's lines with frontmatter and code blocks blanked.

    Blanked lines stay in the list, so each kept line keeps its number.
    An indented block counts as code only after a blank line, so a
    list item's wrapped lines stay prose.

    Args:
        text: Contents of a Markdown file.

    Returns:
        One string per line, empty where the line is not prose.
    """
    lines = text.splitlines()
    kept = []
    frontmatter = bool(lines) and lines[0] == "---"
    fence = None
    indented = False
    previous = ""
    for number, line in enumerate(lines):
        opening = FENCE.match(line)
        if frontmatter:
            frontmatter = number == 0 or line != "---"
            kept.append("")
        elif fence:
            if opening and opening.group(1) == fence:
                fence = None
            kept.append("")
        elif opening:
            fence = opening.group(1)
            kept.append("")
        else:
            if line.startswith("    ") and (indented or not previous.strip()):
                indented = True
            elif line.strip():
                indented = False
            kept.append("" if indented else line)
        previous = line
    return kept


def check_links(skills: Skills) -> list[Problem]:
    """Report each relative link whose file does not exist.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per broken link.
    """
    problems = []
    for path, lines in skills.prose.items():
        for target, line in find_link_targets(lines):
            if not (path.parent / target).exists():
                message = f"links to missing {target}"
                problems.append(Problem(path, line, "error", message))
    return problems


def find_link_targets(lines: list[str]) -> list[tuple[str, int]]:
    """List the relative link targets in prose, without their anchors.

    Inline code is skipped, so a link quoted as an example is not read.
    Web links, mail links and links within the same file are skipped.

    Args:
        lines: Prose lines from ``blank_code_lines``.

    Returns:
        Each target path with the line number the link starts on.
    """
    text = join_prose(lines)
    targets = []
    for match in LINK.finditer(text):
        target = match.group(1).split("#")[0]
        if target and not re.match(r"[a-z]+:", target):
            line = text.count("\n", 0, match.start()) + 1
            targets.append((target, line))
    return targets


def join_prose(lines: list[str]) -> str:
    """Join prose lines into one text with inline code blanked.

    Blanking keeps each line's length, so a match's line number is one
    more than the newlines before it.

    Args:
        lines: Prose lines from ``blank_code_lines``.

    Returns:
        The lines joined with newlines.
    """
    return INLINE_CODE.sub(
        lambda match: " " * len(match.group()),
        "\n".join(lines),
    )


def check_skill_names(skills: Skills) -> list[Problem]:
    """Report each skill the prose names that has no skill folder.

    A name is known when it is in the checkout or installed.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per unknown name.
    """
    problems = []
    for path, lines in skills.prose.items():
        for name, line in find_skill_names(lines):
            if name not in skills.names:
                message = f"no skill named {name}"
                problems.append(Problem(path, line, "error", message))
    return problems


def find_skill_names(lines: list[str]) -> list[tuple[str, int]]:
    """List the skill names in prose.

    Only names written as skill names count, such as "the
    `code-documentation` skill" or a link to ``../<name>/SKILL.md``.

    Args:
        lines: Prose lines from ``blank_code_lines``.

    Returns:
        Each name with its line number.
    """
    names = []
    for number, line in enumerate(lines, 1):
        for match in SKILL_NAME.finditer(line):
            name = next(group for group in match.groups() if group)
            names.append((name, number))
    return names


def check_references_linked(skills: Skills) -> list[Problem]:
    """Report each reference file that its SKILL.md does not link.

    An agent reads only what SKILL.md points to, so an unlinked
    reference is never read.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per unlinked file in ``references/`` or
        ``reference/``.
    """
    problems = []
    for skill in skills.folders:
        targets = find_link_targets(skills.prose[skill / "SKILL.md"])
        linked = {(skill / target).resolve() for target, _ in targets}
        folders = [skill / name for name in REFERENCE_FOLDERS]
        for path in skills.files:
            in_folder = any(path.is_relative_to(folder) for folder in folders)
            if in_folder and path.resolve() not in linked:
                message = "SKILL.md does not link this reference"
                problems.append(Problem(path, 1, "error", message))
    return problems


def check_frontmatter_names(skills: Skills) -> list[Problem]:
    """Report each frontmatter name that is missing or differs from its folder.

    Agents load a skill by its frontmatter name, and the installer and
    skill-lessons find it by its folder.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per skill whose name differs or is missing.
    """
    problems = []
    for skill in skills.folders:
        path = skill / "SKILL.md"
        name = read_frontmatter_name(path.read_text(encoding="utf-8"))
        if name != skill.name:
            message = f"frontmatter name must be {skill.name}"
            problems.append(Problem(path, 1, "error", message))
    return problems


def read_frontmatter_name(text: str) -> str | None:
    """Return the first ``name:`` value in a SKILL.md's frontmatter.

    Args:
        text: Contents of a SKILL.md.

    Returns:
        The name, or None when the frontmatter is missing, is not
        closed, or has no name.
    """
    lines = text.splitlines()
    if lines[:1] != ["---"] or "---" not in lines[1:]:
        return None
    for line in lines[1 : lines.index("---", 1)]:
        if line.startswith("name:"):
            return line.removeprefix("name:").strip()
    return None


def check_lessons_sections(skills: Skills) -> list[Problem]:
    """Report each Lessons section that skill-lessons cannot fill.

    skill-lessons refuses a skill without the marker line under its
    Lessons heading, and the installer would overwrite text written
    under it. A missing section is reported only for a skill whose
    Remembering corrections line names a skill.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        The errors of ``check_lessons_section`` for every skill.
    """
    problems = []
    for skill in skills.folders:
        path = skill / "SKILL.md"
        text = path.read_text(encoding="utf-8")
        problems += check_lessons_section(path, text)
    return problems


def check_lessons_section(path: Path, text: str) -> list[Problem]:
    """Report why one SKILL.md's Lessons section cannot be filled.

    A skill whose Remembering corrections line names a skill needs a
    Lessons section. A skill with neither is fine.

    Args:
        path: The SKILL.md.
        text: Its contents.

    Returns:
        One error per problem found.
    """
    lines = text.splitlines()
    if "## Lessons" not in lines:
        if LESSONS_NAME.search(text):
            return [Problem(path, 1, "error", "no ## Lessons section")]
        return []
    problems = []
    start = lines.index("## Lessons") + 1
    if not lines[start:] or not lines[start].startswith(LESSONS_MARKER):
        message = "the skill-lessons marker must follow ## Lessons"
        problems.append(Problem(path, start, "error", message))
    for number, line in enumerate(lines[start + 1 :], start + 2):
        if line.startswith("## "):
            break
        if line.strip():
            message = "Lessons must stay empty; skill-lessons fills it"
            problems.append(Problem(path, number, "error", message))
            break
    return problems


def check_corrections_names(skills: Skills) -> list[Problem]:
    """Report each Remembering corrections line that is wrong or missing.

    skill-lessons saves the user's corrections to the skill this line
    names, so a line copied from another skill sends them there. A
    skill with a Lessons section needs this line. A skill with neither
    is fine.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per skill whose line is missing or names another
        skill.
    """
    problems = []
    for skill in skills.folders:
        path = skill / "SKILL.md"
        text = path.read_text(encoding="utf-8")
        named = set(LESSONS_NAME.findall(text))
        has_lessons = "## Lessons" in text.splitlines()
        if (has_lessons or named) and named != {skill.name}:
            message = (
                "Remembering corrections must say "
                f'"with the skill name `{skill.name}`"'
            )
            problems.append(Problem(path, 1, "error", message))
    return problems


def find_added_lines(
    skills_dir: Path, base: str, paths: list[Path]
) -> dict[Path, set[int]]:
    """Return the line numbers each file has that base does not.

    Compares the working tree, so uncommitted edits count. Files Git
    does not track yet count as added in full.

    Args:
        skills_dir: A folder inside the checkout.
        base: Commit to compare with.
        paths: Files to compare.

    Returns:
        Added line numbers by file, for files with any.

    Raises:
        subprocess.CalledProcessError: When git fails, such as for a
            base that does not exist.
    """
    top = Path(run_git(skills_dir, "rev-parse", "--show-toplevel").strip())
    diff = run_git(
        top,
        "diff",
        "-U0",
        "--no-color",
        "--no-ext-diff",
        "--dst-prefix=b/",
        base,
        "--",
        *map(str, paths),
    )
    added: defaultdict[Path, set[int]] = defaultdict(set)
    current: Path | None = None
    for line in diff.splitlines():
        hunk = HUNK.match(line)
        if line.startswith("+++ "):
            name = line[4:]
            current = None if name == "/dev/null" else top / name[2:]
        elif hunk and current:
            start = int(hunk.group(1))
            count = int(hunk.group(2) or 1)
            added[current].update(range(start, start + count))
    untracked = run_git(
        top,
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
        "--",
        *map(str, paths),
    )
    for name in filter(None, untracked.split("\0")):
        path = top / name
        count = len(path.read_text(encoding="utf-8").splitlines())
        added[path].update(range(1, count + 1))
    return added


def read_changed_base_texts(skills_dir: Path, base: str) -> dict[Path, str]:
    """Return the base text of each Markdown file changed since base.

    Covers the skills in skills_dir and the ``AGENTS.md`` beside it.
    Deleted files are included. Files added since base are not, and a
    renamed file counts as deleted at its old path.

    Args:
        skills_dir: Folder with one subfolder per skill.
        base: Commit to compare with.

    Returns:
        The text each file had in base, by its path in the checkout.

    Raises:
        subprocess.CalledProcessError: When git fails, such as for a
            base that does not exist.
    """
    top = Path(run_git(skills_dir, "rev-parse", "--show-toplevel").strip())
    names = run_git(
        top,
        "diff",
        "--name-only",
        "--no-renames",
        "--diff-filter=MD",
        "-z",
        base,
        "--",
        str(skills_dir),
        str(skills_dir.parent / "AGENTS.md"),
    )
    return {
        top / name: run_git(top, "show", f"{base}:{name}")
        for name in names.split("\0")
        if name.endswith(".md")
    }


def check_line_widths(
    skills: Skills, added: dict[Path, set[int]]
) -> list[Problem]:
    """Report each added prose line over 72 columns that could wrap.

    Args:
        skills: The skills from ``read_skills``.
        added: Line numbers added since the base commit, by file.

    Returns:
        One error per line to wrap.
    """
    problems = []
    for path, lines in skills.prose.items():
        for number in sorted(added.get(path, set())):
            line = lines[number - 1]
            if needs_wrapping(line):
                message = f"{len(line)} columns; wrap at {MAX_WIDTH}"
                problems.append(Problem(path, number, "error", message))
    return problems


def needs_wrapping(line: str) -> bool:
    """Return whether a prose line is over 72 columns and could wrap.

    Tables and headings cannot wrap, and neither can a line whose only
    space before column 72 is in its indent or list marker, such as
    one long link.

    Args:
        line: A prose line from ``blank_code_lines``.

    Returns:
        True when the line should be wrapped.
    """
    if len(line) <= MAX_WIDTH or line.lstrip().startswith(("|", "#")):
        return False
    head = LIST_MARKER.sub("", line[: MAX_WIDTH + 1], count=1)
    return " " in head


def check_step_pointers(
    skills: Skills, added: dict[Path, set[int]]
) -> list[Problem]:
    """Report each added pointer to a step by its number.

    A pointer such as "as step 6 of Change a skill says" sends the
    reader to the wrong step once steps are added or moved. A pointer
    that wraps onto a second line is found too.

    Args:
        skills: The skills from ``read_skills``.
        added: Line numbers added since the base commit, by file.

    Returns:
        One error per pointer that touches an added line.
    """
    problems = []
    for path, lines in skills.prose.items():
        text = join_prose(lines)
        for match in STEP_POINTER.finditer(text):
            first = text.count("\n", 0, match.start()) + 1
            last = first + match.group().count("\n")
            if added.get(path, set()) & set(range(first, last + 1)):
                pointer = " ".join(match.group().split())
                message = f'"{pointer}" points at a number; name the step'
                problems.append(Problem(path, first, "error", message))
    return problems


def check_skill_sizes(
    skills: Skills, before: dict[Path, str], added: dict[Path, set[int]]
) -> list[Problem]:
    """Report each SKILL.md whose body the change made longer than 500 lines.

    Every run of a skill loads its whole SKILL.md, so detail that only
    some runs need belongs in a reference file. A change that keeps or
    shortens a long body passes, so a skill over the limit can still
    be fixed in steps. The frontmatter does not count.

    Args:
        skills: The skills from ``read_skills``.
        before: Base text of each changed file, from
            ``read_changed_base_texts``.
        added: Line numbers added since the base commit, by file.

    Returns:
        One error per long SKILL.md whose body is new or grew.
    """
    problems = []
    for skill in skills.folders:
        path = skill / "SKILL.md"
        if path not in added:
            continue
        count = count_body_lines(path.read_text(encoding="utf-8"))
        old = count_body_lines(before[path]) if path in before else 0
        if count > MAX_SKILL_LINES and count > old:
            message = (
                f"body has {count} lines, over {MAX_SKILL_LINES}; move "
                "detail to references/"
            )
            problems.append(Problem(path, 1, "error", message))
    return problems


def count_body_lines(text: str) -> int:
    """Return the number of lines after a SKILL.md's frontmatter.

    Args:
        text: Contents of a SKILL.md.

    Returns:
        The line count, or every line when the frontmatter is missing
        or not closed.
    """
    lines = text.splitlines()
    if lines[:1] != ["---"] or "---" not in lines[1:]:
        return len(lines)
    return len(lines) - lines.index("---", 1) - 1


def check_reference_contents(
    skills: Skills, added: dict[Path, set[int]]
) -> list[Problem]:
    """Report each changed reference file over 100 lines with no Contents.

    An agent often reads only the start of a long file, so a Contents
    list there shows what the rest holds.

    Args:
        skills: The skills from ``read_skills``.
        added: Line numbers added since the base commit, by file.

    Returns:
        One error per long reference file with added lines and no
        Contents heading.
    """
    problems = []
    for path, lines in skills.prose.items():
        is_reference = (
            path.parent.name in REFERENCE_FOLDERS
            and path.parent.parent in skills.folders
        )
        is_long = path in added and len(lines) > MAX_REFERENCE_LINES
        has_contents = any(CONTENTS.match(line) for line in lines)
        if is_reference and is_long and not has_contents:
            message = (
                f"{len(lines)} lines, over {MAX_REFERENCE_LINES}, "
                "with no ## Contents list"
            )
            problems.append(Problem(path, 1, "error", message))
    return problems


def check_removed_headings(
    skills: Skills, before: dict[Path, str]
) -> list[Problem]:
    """Report each quote of a heading that the change removed.

    A pointer such as 'as "Change a skill" says' leads nowhere once
    that heading is renamed. Only a quote of a whole heading counts,
    so quoted text that names no section is never reported. A title
    still used as a heading in any checked file is skipped, since the
    quote may point there, as when a file moves.

    Args:
        skills: The skills from ``read_skills``.
        before: Base text of each changed file, from
            ``read_changed_base_texts``.

    Returns:
        One error per quote of a removed heading, in any checked file.
    """
    removed = set()
    for text in before.values():
        removed |= list_heading_titles(blank_code_lines(text))
    for lines in skills.prose.values():
        removed -= list_heading_titles(lines)
    texts = {path: join_prose(lines) for path, lines in skills.prose.items()}
    problems = []
    for title in sorted(removed):
        # A quote may wrap, so any run of spaces matches one space.
        words = r"\s+".join(re.escape(word) for word in title.split())
        quote = re.compile(f'"{words}"')
        for path, text in texts.items():
            for match in quote.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                message = f'quotes "{title}", a heading this change removed'
                problems.append(Problem(path, line, "error", message))
    return problems


def list_heading_titles(lines: list[str]) -> set[str]:
    """Return the title of each heading, without a step number.

    Args:
        lines: Prose lines from ``blank_code_lines``.

    Returns:
        Titles such as "Change a skill", or "Lessons" for a heading
        "### 3. Lessons".
    """
    titles = set()
    for line in lines:
        match = HEADING.match(line)
        if match:
            titles.add(match.group(1))
    return titles


def find_repeats(skills: Skills, added: dict[Path, set[int]]) -> list[Problem]:
    """Report each added run of words another skill also has.

    A run is eight or more words in a row that another skill or
    ``AGENTS.md`` also has, compared in lowercase. A rule stated twice
    drifts apart when one copy changes.

    Args:
        skills: The skills from ``read_skills``.
        added: Line numbers added since the base commit, by file.

    Returns:
        One repeat per run that touches an added line.
    """
    owners = skills.owners
    words = {path: read_words(lines) for path, lines in skills.prose.items()}
    runs = {path: list_runs(items) for path, items in words.items()}
    found_in: defaultdict[tuple[str, ...], set[str]] = defaultdict(set)
    for path, path_runs in runs.items():
        for run in path_runs:
            found_in[run].add(owners[path])
    problems = []
    for path, items in words.items():
        others = [found_in[run] - {owners[path]} for run in runs[path]]
        start = 0
        while start < len(others):
            if not others[start]:
                start += 1
                continue
            # Shared runs that start on back-to-back words make one repeat.
            end = start
            while end < len(others) and others[end]:
                end += 1
            span = items[start : end + REPEAT_WORDS - 1]
            new_lines = [n for _, n in span if n in added.get(path, ())]
            if new_lines:
                where = ", ".join(sorted(set().union(*others[start:end])))
                quote = " ".join(word for word, _ in span[:12])
                message = f'"{quote}" is also in {where}'
                problems.append(Problem(path, new_lines[0], "repeat", message))
            start = end
    return problems


def list_runs(words: list[tuple[str, int]]) -> list[tuple[str, ...]]:
    """List every run of eight words in a row, in order.

    Args:
        words: Words with their line numbers from ``read_words``.

    Returns:
        One tuple of words per starting word.
    """
    texts = [word for word, _ in words]
    return [
        tuple(texts[start : start + REPEAT_WORDS])
        for start in range(len(texts) - REPEAT_WORDS + 1)
    ]


def read_words(lines: list[str]) -> list[tuple[str, int]]:
    """List the words of the prose lines outside the shared sections.

    Args:
        lines: Prose lines from ``blank_code_lines``.

    Returns:
        Each lowercase word with its line number.
    """
    words = []
    shared = False
    for number, line in enumerate(lines, 1):
        if line.startswith("## "):
            shared = line.strip() in SHARED_SECTIONS
        if not shared:
            words += [(word, number) for word in WORD.findall(line.lower())]
    return words


def find_grown_skills(
    skills: Skills, before: dict[Path, str]
) -> list[Problem]:
    """Report each SKILL.md whose body has more lines than in base.

    Each run loads the whole SKILL.md, so a skill should grow only for
    a reason that the pull request gives. A new skill is not reported,
    and neither is a line added to the frontmatter only.

    Args:
        skills: The skills from ``read_skills``.
        before: Base text of each changed file, from
            ``read_changed_base_texts``.

    Returns:
        One grown problem per longer SKILL.md.
    """
    problems = []
    for skill in skills.folders:
        path = skill / "SKILL.md"
        if path not in before:
            continue
        old = count_body_lines(before[path])
        new = count_body_lines(path.read_text(encoding="utf-8"))
        if new > old:
            message = f"body grew from {old} to {new} lines; give the reason"
            problems.append(Problem(path, 1, "grown", message))
    return problems


def run_git(folder: Path, *args: str) -> str:
    """Run git in folder and return its output.

    Args:
        folder: Folder to run in.
        args: Git subcommand and options.

    Returns:
        The command's standard output.

    Raises:
        subprocess.CalledProcessError: When git exits nonzero.
    """
    return subprocess.run(
        ["git", *args],
        cwd=folder,
        check=True,
        capture_output=True,
        text=True,
    ).stdout


def find_skills_dir(given: str | None) -> Path:
    """Return the skills folder to check.

    Args:
        given: The ``--skills`` value, or None to look in the checkout.

    Returns:
        given, else ``ai/skills`` or ``skills`` at the checkout's top.

    Raises:
        subprocess.CalledProcessError: When the current folder is not
            in a Git checkout.
        FileNotFoundError: When the checkout has neither folder.
    """
    if given:
        return Path(given).resolve()
    top = Path(run_git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
    for folder in (top / "ai/skills", top / "skills"):
        if folder.is_dir():
            return folder
    raise FileNotFoundError(f"no ai/skills or skills folder in {top}")


def main() -> int:
    """Print the problems and a count line.

    Returns:
        0 when there is no error, 1 when there is, and 2 when the check
        could not run.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--skills", help="skills folder to check")
    parser.add_argument(
        "--base",
        help="commit to compare with; checks the lines added since then",
    )
    args = parser.parse_args()
    try:
        skills_dir = find_skills_dir(args.skills)
        problems = check_skills(skills_dir, args.base)
    except subprocess.CalledProcessError as error:
        print(
            f"skill_check: git failed: {error.stderr.strip()}", file=sys.stderr
        )
        return 2
    except FileNotFoundError as error:
        print(f"skill_check: {error}", file=sys.stderr)
        return 2
    counts = []
    kinds = (("error", "errors"), ("repeat", "repeats"), ("grown", "grown"))
    for kind, plural in kinds:
        found = [problem for problem in problems if problem.kind == kind]
        for problem in found[:MAX_SHOWN]:
            path = os.path.relpath(problem.path)
            print(f"{path}:{problem.line}: {kind}: {problem.message}")
        if len(found) > MAX_SHOWN:
            print(f"{len(found) - MAX_SHOWN} more {plural}")
        counts.append(f"{len(found)} {plural}")
    print(", ".join(counts))
    return 1 if any(problem.kind == "error" for problem in problems) else 0


if __name__ == "__main__":
    sys.exit(main())
