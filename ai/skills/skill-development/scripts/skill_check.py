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
import json
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
ROUTING_TABLE = "model-routing/routing.json"
INSTALLED_TABLE = INSTALLED_SKILLS / ROUTING_TABLE
ROUTE_CALL = re.compile(r"pde_route\.py\s+([^\s`]+)")
ROUTE_COMMANDS = ("check", "audit")
ROLE_PHRASE = re.compile(
    r"\b(local|cheap|review|strongest) (model|role)s?\b", re.IGNORECASE
)
MODEL_NAME = re.compile(
    r"\b(claude-code|github-copilot|goog|openai|opencode-go)/\S+"
)
# These files quote models and wrong answers on purpose.
ROUTE_SKIPPED = (
    "references/evaluation.md",
    "references/example.md",
    "references/trials.md",
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


@dataclass(frozen=True)
class Routes:
    """The routing table and the skill lines that the route checks read.

    Attributes:
        table: The checkout's ``model-routing/routing.json``, else the
            installed copy, or None when neither exists.
        own: Whether the table is the checkout's own.
        kinds: The skills each kind lists, empty without a table.
        lines: Raw lines of each skill's Markdown files, except the
            files in ``ROUTE_SKIPPED`` and ``evals/`` folders.
        owners: The skill each file in lines belongs to.
        calls: Each ``pde_route.py <kind>`` call in lines, as file,
            line number and kind.
    """

    table: Path | None
    own: bool
    kinds: dict[str, list[str]]
    lines: dict[Path, list[str]]
    owners: dict[Path, str]
    calls: list[tuple[Path, int, str]]


def check_skills(skills_dir: Path, base: str | None) -> list[Problem]:
    """Run every check on the skills in skills_dir, in this order.

    1. A link to a file that does not exist is an error.
    2. A skill name with no skill folder is an error.
    3. A reference file that SKILL.md does not link is an error.
    4. A frontmatter name that is missing or differs from the folder
       is an error.
    5. A Lessons section that skill-lessons cannot fill, or that is
       missing from a skill whose Remembering corrections line names
       a skill but not skill-fix, is an error.
    6. A Remembering corrections line that names another skill, or
       that is missing from a skill with a Lessons section, is an
       error.
    7. A ``metadata.scope`` that is missing or names the other
       repository is an error.
    8. A ``pde_route.py <kind>`` call whose kind is not in the routing
       table is an error. Placeholders such as ``<kind>``, flags and
       the ``check`` and ``audit`` commands are skipped.
    9. A skill that a kind's ``skills`` list names but whose files
       hold no call for that kind is an error, and so is a call from
       a skill the list leaves out.
    10. An OpenCode model name, such as ``openai/gpt-6-luna``, outside
        model-routing is an error.
    11. A case in a skill's ``evals/``, a test prompt that trials.py
        runs, whose ``rule`` is not a heading of that skill's SKILL.md
        is an error, since no one can then tell which rule it tests.

    With base, more checks report only on files and lines changed
    since base, so a problem already on main fails no unrelated
    change.

    12. An added line over 72 columns that could wrap is an error.
    13. An added line that points at a step number, such as "step 6
        of", is an error.
    14. An added role phrase outside model-routing, such as "the
        review model", is an error.
    15. A SKILL.md body that the change made longer than 500 lines is
        an error.
    16. A changed reference file over 100 lines with no Contents
        heading is an error.
    17. A quote of a heading that the change removed or renamed is an
        error, unless some file still has that heading.
    18. Eight words in a row that another skill or ``AGENTS.md`` also
        has are a repeat, for the author to judge.
    19. A SKILL.md whose body has more lines than in base is reported
        as grown, for the pull request to give a reason.

    Checks 1, 2, 12, 13, 17 and 18 also read the ``AGENTS.md`` next to
    skills_dir.

    The route checks, 8, 9, 10 and 14, read the raw lines of skill
    Markdown files, code included. They skip
    ``references/evaluation.md``, ``references/example.md``,
    ``references/trials.md`` and ``evals/`` folders. Check 8 reads the
    table from skills_dir, else from ``~/.agents/skills``. With neither,
    it warns "could not check kinds" on stderr when a skill calls
    ``pde_route.py``, and does not fail. Checks 9, 10 and 14 run only
    when skills_dir holds the table, so an extra checkout gets check 8
    alone.

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
        ValueError: When the routing table is not valid JSON or a kind
            has no ``skills`` list.
    """
    skills = read_skills(skills_dir)
    routes = read_routes(skills, skills_dir)
    problems = (
        check_links(skills)
        + check_skill_names(skills)
        + check_references_linked(skills)
        + check_frontmatter_names(skills)
        + check_lessons_sections(skills)
        + check_corrections_names(skills)
        + check_scopes(skills)
        + check_route_kinds(routes)
        + check_route_skills(routes)
        + check_model_names(routes)
        + check_case_rules(skills)
    )
    if base is None:
        return problems
    added = find_added_lines(skills_dir, base, list(skills.prose))
    before = read_changed_base_texts(skills_dir, base)
    return (
        problems
        + check_line_widths(skills, added)
        + check_step_pointers(skills, added)
        + check_role_phrases(routes, added)
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
    for line in list_frontmatter_lines(text):
        if line.startswith("name:"):
            return line.removeprefix("name:").strip()
    return None


def check_scopes(skills: Skills) -> list[Problem]:
    """Report each skill whose scope tag is missing or wrong.

    The tag in ``metadata.scope`` names the repository that holds the
    skill's source. Skills in an ``ai/skills`` folder, as in this
    public repository, must say ``personal``. Skills in a top-level
    ``skills`` folder, as in the private extra skills repository, must say
    ``extra``. An extra skill copied into the public repository would
    publish private text.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per skill whose tag is missing or does not match its
        folder.
    """
    problems = []
    for skill in skills.folders:
        path = skill / "SKILL.md"
        scope = read_frontmatter_scope(path.read_text(encoding="utf-8"))
        in_ai_folder = skill.parent.parent.name == "ai"
        expected = "personal" if in_ai_folder else "extra"
        if scope != expected:
            message = f"metadata.scope must be {expected}"
            problems.append(Problem(path, 1, "error", message))
    return problems


def read_frontmatter_scope(text: str) -> str | None:
    """Return the ``scope`` value under ``metadata:`` in a SKILL.md.

    Quotes around the value are dropped, so ``scope: "extra"`` gives
    ``extra``.

    Args:
        text: Contents of a SKILL.md.

    Returns:
        The scope, or None when the frontmatter has no scope under
        ``metadata:``.
    """
    in_metadata = False
    for line in list_frontmatter_lines(text):
        if not line.startswith((" ", "\t")):
            in_metadata = line.rstrip() == "metadata:"
        elif in_metadata and line.strip().startswith("scope:"):
            value = line.strip().removeprefix("scope:").strip()
            return value.strip("\"'")
    return None


def list_frontmatter_lines(text: str) -> list[str]:
    """Return the lines between a SKILL.md's two ``---`` lines.

    Args:
        text: Contents of a SKILL.md.

    Returns:
        The frontmatter lines, or an empty list when the frontmatter is
        missing or not closed.
    """
    lines = text.splitlines()
    if lines[:1] != ["---"] or "---" not in lines[1:]:
        return []
    return lines[1 : lines.index("---", 1)]


def check_lessons_sections(skills: Skills) -> list[Problem]:
    """Report each Lessons section that skill-lessons cannot fill.

    skill-lessons refuses a skill without the marker line under its
    Lessons heading, and the installer would overwrite text written
    under it. A missing section is reported only for a skill whose
    Remembering corrections line names a skill but not skill-fix.

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
    Lessons section, unless the line sends corrections to skill-fix,
    which writes them into the skill's text. A skill with neither is
    fine.

    Args:
        path: The SKILL.md.
        text: Its contents.

    Returns:
        One error per problem found.
    """
    lines = text.splitlines()
    if "## Lessons" not in lines:
        if LESSONS_NAME.search(text) and not names_skill_fix(text):
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


def names_skill_fix(text: str) -> bool:
    """Return whether the Remembering corrections section names skill-fix.

    Args:
        text: Contents of a SKILL.md.

    Returns:
        True when that section mentions ``skill-fix``, such as in a
        link to ``../skill-fix/SKILL.md``.
    """
    _, found, rest = text.partition("\n## Remembering corrections\n")
    section = rest.split("\n## ", 1)[0]
    return bool(found) and "skill-fix" in section


def check_corrections_names(skills: Skills) -> list[Problem]:
    """Report each Remembering corrections line that is wrong or missing.

    skill-fix commits the user's corrections to the skill this line
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


def read_routes(skills: Skills, skills_dir: Path) -> Routes:
    """Read the routing table and the skill lines the route checks need.

    Args:
        skills: The skills from ``read_skills``.
        skills_dir: Folder with one subfolder per skill.

    Returns:
        The table, if found, and each checked file's raw lines and
        calls.

    Raises:
        ValueError: When the table is not valid JSON or a kind has no
            ``skills`` list.
    """
    own_table = skills_dir / ROUTING_TABLE
    table: Path | None = None
    for candidate in (own_table, INSTALLED_TABLE):
        if candidate.is_file():
            table = candidate
            break
    kinds = read_routing_table(table) if table else {}
    lines = {}
    owners = {}
    for path, owner in skills.owners.items():
        if owner == "AGENTS.md":
            continue
        relative = path.relative_to(skills_dir / owner)
        if relative.as_posix() in ROUTE_SKIPPED or "evals" in relative.parts:
            continue
        lines[path] = path.read_text(encoding="utf-8").splitlines()
        owners[path] = owner
    calls = find_route_calls(lines)
    return Routes(table, table == own_table, kinds, lines, owners, calls)


def read_routing_table(path: Path) -> dict[str, list[str]]:
    """Return the skills each kind in a routing table lists.

    Only the ``skills`` field is read. ``pde_route.py`` checks the
    other fields when it loads the table.

    Args:
        path: A ``routing.json`` file.

    Returns:
        Each kind with its ``skills`` list.

    Raises:
        ValueError: When the file is not valid JSON, is not an object
            of kinds, or a kind has no list of skill names.
    """
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"{path} is not valid JSON: {error}") from error
    if not isinstance(data, dict):
        raise ValueError(f"{path} must hold an object of kinds")
    kinds = {}
    for kind, entry in data.items():
        names = entry.get("skills") if isinstance(entry, dict) else None
        if not isinstance(names, list) or not all(
            isinstance(name, str) for name in names
        ):
            raise ValueError(f"{path}: kind {kind} needs a skills list")
        kinds[kind] = names
    return kinds


def find_route_calls(
    lines: dict[Path, list[str]],
) -> list[tuple[Path, int, str]]:
    """List each ``pde_route.py <kind>`` call with its kind.

    The kind is the word after ``pde_route.py``. A placeholder such as
    ``<kind>``, a flag and the ``check`` and ``audit`` commands name no
    kind, so they are skipped.

    Args:
        lines: Raw lines of each checked file.

    Returns:
        Each call's file, line number and kind.
    """
    calls = []
    for path, path_lines in lines.items():
        for number, line in enumerate(path_lines, 1):
            for match in ROUTE_CALL.finditer(line):
                kind = match.group(1)
                skipped = kind.startswith(("<", "-"))
                if not skipped and kind not in ROUTE_COMMANDS:
                    calls.append((path, number, kind))
    return calls


def check_route_kinds(routes: Routes) -> list[Problem]:
    """Report each ``pde_route.py`` call whose kind the table lacks.

    ``pde_route.py`` exits 2 on an unknown kind, so the agent starts
    with no model. With no table to read, this prints a warning to
    stderr when a skill calls ``pde_route.py``, and reports nothing.

    Args:
        routes: The table and calls from ``read_routes``.

    Returns:
        One error per call with an unknown kind.
    """
    if routes.table is None:
        if routes.calls:
            print(
                "skill_check: could not check kinds; no routing.json in the "
                "checkout or ~/.agents/skills",
                file=sys.stderr,
            )
        return []
    return [
        Problem(path, line, "error", f"no route kind named {kind}")
        for path, line, kind in routes.calls
        if kind not in routes.kinds
    ]


def check_route_skills(routes: Routes) -> list[Problem]:
    """Report each mismatch between a kind's ``skills`` list and the calls.

    The list tells a reader of the table which skills a model change
    affects. A listed skill whose call was deleted is reported at the
    kind's line in the table.

    Args:
        routes: The table and calls from ``read_routes``.

    Returns:
        One error per listed skill with no call for the kind, and one
        per call from a skill the kind does not list. None when the
        checkout does not hold the table.
    """
    table = routes.table
    if not routes.own or table is None:
        return []
    table_lines = table.read_text(encoding="utf-8").splitlines()
    called = {(routes.owners[path], kind) for path, _, kind in routes.calls}
    problems = []
    for kind, names in routes.kinds.items():
        for name in names:
            if (name, kind) not in called:
                line = find_kind_line(table_lines, kind)
                message = f"kind {kind} lists {name}, which has no call for it"
                problems.append(Problem(table, line, "error", message))
    for path, line, kind in routes.calls:
        owner = routes.owners[path]
        if kind in routes.kinds and owner not in routes.kinds[kind]:
            message = f"kind {kind} in routing.json does not list {owner}"
            problems.append(Problem(path, line, "error", message))
    return problems


def find_kind_line(table_lines: list[str], kind: str) -> int:
    """Return the line number of a kind's key in the routing table.

    Args:
        table_lines: Lines of ``routing.json``.
        kind: A kind in the table.

    Returns:
        The number of the first line that starts with the quoted kind,
        or 1 when no line does.
    """
    for number, line in enumerate(table_lines, 1):
        if line.lstrip().startswith(f'"{kind}"'):
            return number
    return 1


def check_model_names(routes: Routes) -> list[Problem]:
    """Report each OpenCode model name outside model-routing.

    A skill that names a model keeps it after the table moves on.

    Args:
        routes: The lines from ``read_routes``.

    Returns:
        One error per name, or none when the checkout does not hold
        the table.
    """
    if not routes.own:
        return []
    problems = []
    for path, lines in routes.lines.items():
        if routes.owners[path] == "model-routing":
            continue
        for number, line in enumerate(lines, 1):
            for match in MODEL_NAME.finditer(line):
                message = f"{match.group()} names a model; call pde_route.py"
                problems.append(Problem(path, number, "error", message))
    return problems


def check_case_rules(skills: Skills) -> list[Problem]:
    """Report each eval case whose ``rule`` is not a heading of its skill.

    A case in ``evals/`` names the SKILL.md heading it tests. Once that
    heading is renamed or removed, no one can tell which rule the case
    covers, or whether the rule is still there.

    Args:
        skills: The skills from ``read_skills``.

    Returns:
        One error per case file whose ``rule`` names no heading.
    """
    problems = []
    for path in skills.files:
        skill = path.parent.parent
        if path.suffix != ".json" or path.parent.name != "evals":
            continue
        if skill not in skills.folders:
            continue
        try:
            case = json.loads(path.read_text(encoding="utf-8"))
        except ValueError:
            case = None
        rule = case.get("rule") if isinstance(case, dict) else None
        titles = list_heading_titles(skills.prose[skill / "SKILL.md"])
        if not isinstance(rule, str) or rule not in titles:
            message = f"rule {rule!r} is not a heading of {skill.name}"
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


def check_role_phrases(
    routes: Routes, added: dict[Path, set[int]]
) -> list[Problem]:
    """Report each added role phrase, such as "the cheap model".

    Kinds in the routing table replace roles, so a role phrase outside
    model-routing names a model that no table picks.

    Args:
        routes: The lines from ``read_routes``.
        added: Line numbers added since the base commit, by file.

    Returns:
        One error per phrase on an added line, or none when the
        checkout does not hold the table.
    """
    if not routes.own:
        return []
    problems = []
    for path, lines in routes.lines.items():
        if routes.owners[path] == "model-routing":
            continue
        for number in sorted(added.get(path, set())):
            for match in ROLE_PHRASE.finditer(lines[number - 1]):
                message = f'"{match.group()}" names a role; call pde_route.py'
                problems.append(Problem(path, number, "error", message))
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
    except (FileNotFoundError, ValueError) as error:
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
