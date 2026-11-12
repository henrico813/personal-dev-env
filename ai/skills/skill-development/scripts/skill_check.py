"""Check a checkout's skills for mistakes that break them.

Prints one line per problem and exits 1 when any problem is an error.
Errors are a link to a file that does not exist, a skill name with no
skill folder, a reference file SKILL.md does not link, a frontmatter
name that differs from the folder, and a Lessons section skill-lessons
cannot fill. With ``--base``, the lines added since that commit are
checked too. An added line over 72 columns that could wrap is an
error, and eight words in a row that another skill also has are
reported for the author to judge. Files Git ignores are skipped. Read
``check_skills`` first.

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
MAX_SHOWN = 10
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


@dataclass(frozen=True)
class Problem:
    """One finding, printed as ``path:line: kind: message``.

    Its kind is "error", which fails the check, or "repeat", which the
    author judges.
    """

    path: Path
    line: int
    kind: str
    message: str


def check_skills(skills_dir: Path, base: str | None) -> list[Problem]:
    """Run every check on the skills in skills_dir.

    Only files from ``list_skill_files`` are read. The ``AGENTS.md``
    next to skills_dir, when present, is checked for skill names, and
    its added lines for width and repeats.

    Args:
        skills_dir: Folder with one subfolder per skill.
        base: Commit to compare with, or None to skip the checks of
            added lines.

    Returns:
        Errors first, then repeats.

    Raises:
        subprocess.CalledProcessError: When git fails, such as outside
            a checkout or for a base that does not exist.
    """
    files = list_skill_files(skills_dir)
    skills = sorted(
        path.parent
        for path in files
        if path.name == "SKILL.md" and path.parent.parent == skills_dir
    )
    names = {skill.name for skill in skills}
    names |= {p.parent.name for p in INSTALLED_SKILLS.glob("*/SKILL.md")}
    owners = {}
    for path in files:
        folder = skills_dir / path.relative_to(skills_dir).parts[0]
        if path.suffix == ".md" and folder in skills:
            owners[path] = folder.name
    agents = skills_dir.parent / "AGENTS.md"
    if agents.is_file():
        owners[agents] = "AGENTS.md"
    prose = {
        path: blank_code_lines(path.read_text(encoding="utf-8"))
        for path in owners
    }
    errors = []
    for path, lines in prose.items():
        errors += check_links(path, lines)
        errors += check_skill_names(path, lines, names)
    for skill in skills:
        text = (skill / "SKILL.md").read_text(encoding="utf-8")
        errors += check_frontmatter_name(skill, text)
        errors += check_references(skill, prose[skill / "SKILL.md"], files)
        errors += check_lessons(skill, text)
    if base is None:
        return errors
    added = find_added_lines(skills_dir, base, list(owners))
    for path, lines in prose.items():
        errors += find_wide_lines(path, lines, added.get(path, set()))
    return errors + find_repeats(prose, owners, added)


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


def check_links(path: Path, lines: list[str]) -> list[Problem]:
    """Report each relative link whose file does not exist.

    Args:
        path: Markdown file the lines come from.
        lines: The file's prose lines from ``blank_code_lines``.

    Returns:
        One error per broken link.
    """
    problems = []
    for target, line in find_link_targets(lines):
        if not (path.parent / target).exists():
            problems.append(
                Problem(path, line, "error", f"links to missing {target}")
            )
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
    text = INLINE_CODE.sub(
        lambda match: " " * len(match.group()),
        "\n".join(lines),
    )
    targets = []
    for match in LINK.finditer(text):
        target = match.group(1).split("#")[0]
        if target and not re.match(r"[a-z]+:", target):
            line = text.count("\n", 0, match.start()) + 1
            targets.append((target, line))
    return targets


def check_skill_names(
    path: Path, lines: list[str], names: set[str]
) -> list[Problem]:
    """Report each skill the prose names that has no skill folder.

    Only names written as skill names count, such as "the
    `code-documentation` skill" or a link to ``../<name>/SKILL.md``.

    Args:
        path: Markdown file the lines come from.
        lines: The file's prose lines from ``blank_code_lines``.
        names: Skills in the checkout or installed.

    Returns:
        One error per unknown name.
    """
    problems = []
    for number, line in enumerate(lines, 1):
        for match in SKILL_NAME.finditer(line):
            name = next(group for group in match.groups() if group)
            if name not in names:
                problems.append(
                    Problem(path, number, "error", f"no skill named {name}")
                )
    return problems


def check_frontmatter_name(skill: Path, text: str) -> list[Problem]:
    """Report a frontmatter name that differs from the skill's folder.

    Agents load a skill by its frontmatter name, and the installer and
    skill-lessons find it by its folder.

    Args:
        skill: The skill's folder.
        text: Contents of its SKILL.md.

    Returns:
        One error when the name differs or is missing, else nothing.
    """
    lines = text.splitlines()
    closed = lines[:1] == ["---"] and "---" in lines[1:]
    end = lines.index("---", 1) if closed else 0
    for line in lines[1:end]:
        if line.startswith("name:"):
            name = line.removeprefix("name:").strip()
            if name == skill.name:
                return []
            break
    message = f"frontmatter name must be {skill.name}"
    return [Problem(skill / "SKILL.md", 1, "error", message)]


def check_references(
    skill: Path, lines: list[str], files: list[Path]
) -> list[Problem]:
    """Report each reference file that SKILL.md does not link.

    An agent reads only what SKILL.md points to, so an unlinked
    reference is never read.

    Args:
        skill: The skill's folder.
        lines: Prose lines of its SKILL.md from ``blank_code_lines``.
        files: Files from ``list_skill_files``.

    Returns:
        One error per unlinked file in ``references/`` or
        ``reference/``.
    """
    linked = {
        (skill / target).resolve() for target, _ in find_link_targets(lines)
    }
    folders = (skill / "references", skill / "reference")
    problems = []
    for path in files:
        in_folder = any(path.is_relative_to(folder) for folder in folders)
        if in_folder and path.resolve() not in linked:
            message = "SKILL.md does not link this reference"
            problems.append(Problem(path, 1, "error", message))
    return problems


def check_lessons(skill: Path, text: str) -> list[Problem]:
    """Report a Lessons section that skill-lessons cannot fill.

    skill-lessons refuses a skill without the marker line under its
    Lessons heading, and the installer would overwrite text written
    under it. A Remembering corrections line that names another skill
    sends the user's corrections to that skill.

    Args:
        skill: The skill's folder.
        text: Contents of its SKILL.md.

    Returns:
        One error per problem, or nothing for a skill without lessons.
    """
    lines = text.splitlines()
    named = set(LESSONS_NAME.findall(text))
    if "## Lessons" not in lines and not named:
        return []
    path = skill / "SKILL.md"
    problems = []
    if "## Lessons" not in lines:
        problems.append(Problem(path, 1, "error", "no ## Lessons section"))
    else:
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
    if named != {skill.name}:
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


def find_wide_lines(
    path: Path, lines: list[str], added: set[int]
) -> list[Problem]:
    """Report each added prose line over 72 columns that could wrap.

    Tables and headings are skipped, and so is a line whose only space
    before column 72 is in its indent or list marker, such as one long
    link.

    Args:
        path: Markdown file the lines come from.
        lines: The file's prose lines from ``blank_code_lines``.
        added: Line numbers added since the base commit.

    Returns:
        One error per line to wrap.
    """
    problems = []
    for number in sorted(added):
        line = lines[number - 1]
        if len(line) <= MAX_WIDTH or line.lstrip().startswith(("|", "#")):
            continue
        head = LIST_MARKER.sub("", line[: MAX_WIDTH + 1], count=1)
        if " " in head:
            message = f"{len(line)} columns; wrap at {MAX_WIDTH}"
            problems.append(Problem(path, number, "error", message))
    return problems


def find_repeats(
    prose: dict[Path, list[str]],
    owners: dict[Path, str],
    added: dict[Path, set[int]],
) -> list[Problem]:
    """Report each added run of words another skill also has.

    A run is eight or more words in a row that another skill or
    ``AGENTS.md`` also has, compared in lowercase. A rule stated twice
    drifts apart when one copy changes.

    Args:
        prose: Prose lines of each file from ``blank_code_lines``.
        owners: The skill, or ``AGENTS.md``, that owns each file.
        added: Line numbers added since the base commit, by file.

    Returns:
        One repeat per run that touches an added line.
    """
    words = {path: read_words(lines) for path, lines in prose.items()}
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
            # Overlapping shared runs make one repeat, reported once.
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
            start = end + REPEAT_WORDS - 1
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
    errors = [problem for problem in problems if problem.kind == "error"]
    repeats = [problem for problem in problems if problem.kind == "repeat"]
    for kind, found in (("error", errors), ("repeat", repeats)):
        for problem in found[:MAX_SHOWN]:
            path = os.path.relpath(problem.path)
            print(f"{path}:{problem.line}: {kind}: {problem.message}")
        if len(found) > MAX_SHOWN:
            print(f"{len(found) - MAX_SHOWN} more {kind}s")
    print(f"{len(errors)} errors, {len(repeats)} repeats")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
