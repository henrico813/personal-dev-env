"""Behavior tests for the skill-development skill's check script."""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/skill-development/scripts/skill_check.py"
SKILL = """\
---
name: {name}
description: Plans garden beds. Use when planning a bed.
metadata:
  scope: personal
---

# Garden

{body}

## Remembering corrections

Follow every lesson in the Lessons section below. When the user
corrects you, save the correction with the skill name `{name}`.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
"""
GARDEN_BODY = """\
Water each bed early in the morning before the sun is high.
Read [the soil guide](references/soil.md) before you plant.
Pass leaves to the `compost` skill."""
COMPOST_BODY = "Turn the pile once a week so the middle stays warm."
GARDEN = "ai/skills/garden/SKILL.md"
COMPOST = "ai/skills/compost/SKILL.md"
SOIL = "ai/skills/garden/references/soil.md"
AGENTS = "ai/AGENTS.md"
ROUTE_GARDEN_BODY = """\
Read [the build notes](references/build.md) and
[the cases](references/evaluation.md) before you plant.

```text
python3 scripts/pde_route.py plan-review --start opencode-subagent
```"""
ROUTING = "ai/skills/model-routing/SKILL.md"
TABLE = "ai/skills/model-routing/routing.json"
BUILD = "ai/skills/garden/references/build.md"
EVALUATION = "ai/skills/garden/references/evaluation.md"


def make_env(tmp_path: Path) -> dict[str, str]:
    """Return an environment with a fake home and git isolated.

    Args:
        tmp_path: Test folder that holds the fake home.

    Returns:
        A copy of the process environment whose home is
        ``tmp_path / "home"`` and whose git ignores the developer's
        config.
    """
    return os.environ | {
        "HOME": str(tmp_path / "home"),
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CEILING_DIRECTORIES": str(tmp_path),
        "GIT_AUTHOR_NAME": "Test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "Test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }


def write_skill(repo: Path, name: str, body: str) -> Path:
    """Write a skill that passes every check.

    Args:
        repo: Checkout to write in.
        name: Skill and folder name.
        body: Markdown under the skill's title.

    Returns:
        The skill's SKILL.md.
    """
    path = repo / "ai/skills" / name / "SKILL.md"
    path.parent.mkdir(parents=True)
    path.write_text(SKILL.format(name=name, body=body))
    return path


def replace_text(path: Path, old: str, new: str) -> None:
    """Replace old with new in a file, failing when old is not there.

    Args:
        path: File to edit.
        old: Text that must be in the file.
        new: Text to put in its place.

    Raises:
        AssertionError: When old is not in the file.
    """
    text = path.read_text()
    assert old in text, (old, path)
    path.write_text(text.replace(old, new))


def commit_all(repo: Path) -> None:
    """Commit every change in the checkout, so it becomes the base.

    Args:
        repo: Checkout made by the ``repo`` fixture.
    """
    env = make_env(repo.parent)
    for command in (["add", "."], ["commit", "-qm", "change"]):
        subprocess.run(["git", *command], cwd=repo, env=env, check=True)


def run_check(repo: Path, *args: str) -> subprocess.CompletedProcess[str]:
    """Run the script from the checkout's top folder.

    Args:
        repo: Checkout made by the ``repo`` fixture.
        args: Options for the script.

    Returns:
        The finished process with captured output.
    """
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        cwd=repo,
        env=make_env(repo.parent),
        capture_output=True,
        text=True,
    )


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """A committed checkout with skills garden and compost, both valid.

    garden links a reference file and names compost, and
    ``ai/AGENTS.md`` routes to garden. A fake home has the installed
    skill ``hoe``.

    Args:
        tmp_path: Test folder for the checkout and the fake home.

    Returns:
        The checkout.
    """
    hoe = tmp_path / "home/.agents/skills/hoe/SKILL.md"
    hoe.parent.mkdir(parents=True)
    hoe.write_text("---\nname: hoe\n---\n")
    repo = tmp_path / "repo"
    garden = write_skill(repo, "garden", GARDEN_BODY)
    (garden.parent / "references").mkdir()
    (garden.parent / "references/soil.md").write_text("# Soil\n")
    write_skill(repo, "compost", COMPOST_BODY)
    (repo / AGENTS).write_text(
        "- When planning a bed, load and follow the `garden` skill.\n"
    )
    env = make_env(tmp_path)
    for command in (["init", "-q"], ["add", "."], ["commit", "-qm", "base"]):
        subprocess.run(["git", *command], cwd=repo, env=env, check=True)
    return repo


class TestSkillText:
    """Errors in the text of any skill, found without ``--base``."""

    def test_repository_skills_pass_every_check(self, tmp_path: Path) -> None:
        """The skills in this repository have no error.

        CI runs this test, so a change that breaks a link, a skill
        name, a reference, a frontmatter name, a Lessons section, a
        Remembering corrections line or a scope tag fails before it
        merges, whoever wrote it and whatever skill they loaded. A new
        skill without ``scope: personal`` fails here too.
        """
        result = subprocess.run(
            [sys.executable, str(SCRIPT)],
            cwd=ROOT,
            env=make_env(tmp_path),
            capture_output=True,
            text=True,
        )

        assert result.returncode == 0, result.stdout + result.stderr

    @pytest.mark.parametrize(
        "file,old,new,reason",
        [
            pytest.param(
                GARDEN,
                "references/soil.md)",
                "references/soils.md)",
                "links to missing references/soils.md",
                id="broken-link",
            ),
            pytest.param(
                GARDEN,
                "`compost` skill",
                "`mulch` skill",
                "no skill named mulch",
                id="unknown-skill",
            ),
            pytest.param(
                AGENTS,
                "`garden` skill",
                "`gardens` skill",
                "no skill named gardens",
                id="unknown-skill-in-routing-line",
            ),
            pytest.param(
                GARDEN,
                "[the soil guide](references/soil.md)",
                "the soil guide",
                "SKILL.md does not link this reference",
                id="unlinked-reference",
            ),
            pytest.param(
                GARDEN,
                "name: garden",
                "name: gardens",
                "frontmatter name must be garden",
                id="wrong-name",
            ),
            pytest.param(
                GARDEN,
                "skill name `garden`",
                "skill name `compost`",
                'must say "with the skill name `garden`"',
                id="lessons-for-other-skill",
            ),
            pytest.param(
                GARDEN,
                "do not edit -->\n",
                "do not edit -->\n- Water at dawn. [L1]\n",
                "Lessons must stay empty",
                id="lessons-written-by-hand",
            ),
            pytest.param(
                GARDEN,
                "<!-- skill-lessons: filled from the local store; "
                "do not edit -->\n",
                "- Water at dawn. [L1]\n",
                "the skill-lessons marker must follow",
                id="lessons-marker-gone",
            ),
            pytest.param(
                GARDEN,
                "## Lessons\n<!-- skill-lessons: filled from the local "
                "store; do not edit -->\n",
                "",
                "no ## Lessons section",
                id="lessons-section-gone",
            ),
            pytest.param(
                GARDEN,
                "with the skill name `garden`",
                "under `garden`",
                'must say "with the skill name `garden`"',
                id="lessons-without-skill-name",
            ),
            pytest.param(
                GARDEN,
                "scope: personal",
                "extra",
                "metadata.scope must be personal",
                id="work-scope-in-personal-repo",
            ),
            pytest.param(
                GARDEN,
                "metadata:\n  scope: personal\n",
                "",
                "metadata.scope must be personal",
                id="scope-missing",
            ),
        ],
    )
    def test_defect_fails_with_its_reason(
        self, repo: Path, file: str, old: str, new: str, reason: str
    ) -> None:
        """Each kind of defect fails the check and says what is wrong.

        Renaming, merging or copying skills is how these defects
        start. An agent that follows a link or skill name that leads
        nowhere misses the rule behind it. A Remembering corrections
        line copied from another skill saves the user's corrections to
        that skill, with no visible failure. A extra copied into
        this public repository would publish work text, and a skill
        with no scope tag cannot tell a fix which repository to go to.
        """
        replace_text(repo / file, old, new)

        result = run_check(repo)

        assert result.returncode == 1
        assert reason in result.stdout

    @pytest.mark.parametrize(
        "new",
        [
            pytest.param(
                "```text\nLoad the `mulch` skill; see [it](mulch.md).\n```",
                id="fenced-example",
            ),
            pytest.param(
                "    Load the `mulch` skill; see [it](mulch.md).",
                id="indented-example",
            ),
            pytest.param("Load the `hoe` skill.", id="installed-skill"),
        ],
    )
    def test_examples_and_installed_names_pass(
        self, repo: Path, new: str
    ) -> None:
        """Code examples and installed skill names are not errors.

        Skills show made-up skills and links in code examples. A
        skill here may also name a skill that another repository
        installs. Failing on those would push authors to delete good
        examples or skip the check.
        """
        replace_text(
            repo / GARDEN,
            "# Garden\n",
            f"# Garden\n\n{new}\n",
        )

        result = run_check(repo)

        assert result.returncode == 0, result.stdout

    def test_skill_fix_line_needs_no_lessons(self, repo: Path) -> None:
        """A skill that sends corrections to skill-fix needs no Lessons.

        skill-fix writes each correction into the skill's own text, so
        a new skill gets a Remembering corrections line that names
        skill-fix and no Lessons section. Requiring the section would
        make every new skill carry one that nothing fills.
        """
        garden = repo / GARDEN
        replace_text(garden, "save the correction", "follow `skill-fix`")
        replace_text(
            garden,
            "## Lessons\n<!-- skill-lessons: filled from the local "
            "store; do not edit -->\n",
            "",
        )

        result = run_check(repo)

        assert result.returncode == 0, result.stdout

    @pytest.mark.parametrize(
        "rule,code",
        [
            pytest.param("Garden", 0, id="heading"),
            pytest.param("Watering", 1, id="no-such-heading"),
        ],
    )
    def test_case_rule_must_be_skill_heading(
        self, repo: Path, rule: str, code: int
    ) -> None:
        """A case in ``evals/`` fails when its rule is not a heading.

        A case names the SKILL.md heading it tests. After that heading
        is renamed or removed, the case points at no rule, and an
        author deciding whether a rule is safe to drop by its cases
        finds none.
        """
        case = repo / "ai/skills/garden/evals/dawn.json"
        case.parent.mkdir()
        case.write_text(json.dumps({"id": "dawn", "rule": rule}))

        result = run_check(repo)

        assert result.returncode == code, result.stdout

    def test_git_ignored_file_is_not_checked(self, repo: Path) -> None:
        """A file Git ignores in a skill folder is not read.

        Installing an example's packages, as the typescript-development
        examples README says to, fills an ignored folder with other
        projects' Markdown. Errors there would block the author on
        their machine while CI, which never has those files, passes.
        """
        (repo / "ai/skills/garden/.gitignore").write_text("/vendor/\n")
        vendored = repo / "ai/skills/garden/vendor/README.md"
        vendored.parent.mkdir()
        vendored.write_text("See [usage](docs/usage.md).\n")

        result = run_check(repo)

        assert result.returncode == 0, result.stdout


class TestAddedLines:
    """``--base`` adds the checks of added lines, such as width."""

    def test_base_reports_only_added_repeats(self, repo: Path) -> None:
        """Repeats are reported for added lines and new files only.

        The skills on main share hundreds of sentences on purpose, so
        a report of every repeat would bury the one a change adds. A
        new skill is not committed when its author first runs the
        check, so its lines must count as added. Repeats never fail
        the check, since the author decides whether each one stays.
        """
        garden = repo / GARDEN
        replace_text(garden, "# Garden\n", f"# Garden\n\n{COMPOST_BODY}\n")
        write_skill(repo, "mulch", GARDEN_BODY.splitlines()[0])

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == 0, result.stdout
        assert result.stdout.splitlines()[-1].startswith(
            "0 errors, 2 repeats,"
        )
        assert "ai/skills/garden/SKILL.md:10: repeat: " in result.stdout
        assert "ai/skills/mulch/SKILL.md:10: repeat: " in result.stdout

    def test_base_reports_repeat_overlapping_another(self, repo: Path) -> None:
        """Overlapping repeats from two skills are both reported.

        An added paragraph can join sentences copied from two skills.
        The check once skipped the words after each repeat, so the
        second skill's copy went unreported.
        """
        replace_text(
            repo / GARDEN,
            "# Garden\n",
            "# Garden\n\nTurn the pile once a week so the middle stays warm"
            " and dry under straw.\n",
        )
        write_skill(
            repo, "mulch", "Rake so the middle stays warm and dry under straw."
        )

        result = run_check(repo, "--base", "HEAD")

        assert "is also in compost" in result.stdout
        assert "is also in mulch" in result.stdout

    @pytest.mark.parametrize(
        "line,code",
        [
            pytest.param(
                "Water the bed by hand when the soil is dry two fingers "
                "deep, and skip it after rain.",
                1,
                id="prose",
            ),
            pytest.param(
                "- [guide](https://example.com/garden/beds/planting/guides/"
                "for/every/season/of/the/year)",
                0,
                id="one-long-link",
            ),
            pytest.param(
                "| bed | plant | water | sun | soil | notes | sown | "
                "picked | yield | weeds | pests |",
                0,
                id="table-row",
            ),
        ],
    )
    def test_added_line_fails_only_when_wrappable(
        self, repo: Path, line: str, code: int
    ) -> None:
        """An added line over 72 columns fails only when it could wrap.

        The user reads skills on a phone, and lines over 72 columns
        break there. A table row or one long link cannot wrap, so
        failing on it would block a correct change.
        """
        replace_text(
            repo / GARDEN,
            "# Garden\n",
            f"# Garden\n\n{line}\n",
        )

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == code, result.stdout

    @pytest.mark.parametrize(
        "line,code",
        [
            pytest.param("Water as step 2 of Planting says.", 1, id="pointer"),
            pytest.param(
                "Water as steps 1\nto 2 of Planting say.", 1, id="wrapped"
            ),
            pytest.param(
                "Say you are on step 2 of 3 in each update.", 0, id="count"
            ),
        ],
    )
    def test_step_pointers_fail_but_counts_pass(
        self, repo: Path, line: str, code: int
    ) -> None:
        """An added pointer to a step by its number fails the check.

        Renumbering a skill's steps sends every such pointer to the
        wrong step, with no error. Pointers wrap at 72 columns like
        any text. Progress such as "step 2 of 3" counts steps and
        points at none, and evaluation files quote it.
        """
        replace_text(repo / GARDEN, "# Garden\n", f"# Garden\n\n{line}\n")

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == code, result.stdout


def quote_soil_heading(repo: Path) -> None:
    """Commit a soil heading that the compost skill quotes.

    Args:
        repo: Checkout made by the ``repo`` fixture.
    """
    (repo / SOIL).write_text("# Soil\n\n## Testing the soil\n\nDig.\n")
    replace_text(
        repo / COMPOST,
        COMPOST_BODY,
        f'{COMPOST_BODY}\nRead "Testing\nthe soil" in the `garden` skill.',
    )
    commit_all(repo)


class TestChangedFiles:
    """``--base`` adds the checks of changed files, such as size."""

    @pytest.mark.parametrize(
        "changed,old,new,code",
        [
            pytest.param(
                COMPOST, "# Garden\n", "# Garden\n\nRake.\n", 1, id="grew"
            ),
            pytest.param(
                GARDEN,
                "# Garden\n",
                "# Garden\n\nRake.\n",
                0,
                id="other-skill",
            ),
            pytest.param(
                COMPOST, "Water bed 0 at dawn.\n", "", 0, id="shortened"
            ),
            pytest.param(
                COMPOST,
                "---\nname:",
                "---\nlicense: MIT\nname:",
                0,
                id="frontmatter-line",
            ),
        ],
    )
    def test_long_skill_fails_only_when_grown(
        self, repo: Path, changed: str, old: str, new: str, code: int
    ) -> None:
        """A SKILL.md body over 500 lines fails only a change that grows it.

        Every run loads the whole SKILL.md, so a long one costs every
        session. A long skill already on main must not fail changes
        to other skills, or a change that shortens it, or no skill
        change could merge until someone splits it. Only the body
        counts, so a new frontmatter line, such as a scope tag, passes.
        """
        long_body = "\n".join(f"Water bed {n} at dawn." for n in range(501))
        replace_text(repo / COMPOST, COMPOST_BODY, long_body)
        commit_all(repo)
        replace_text(repo / changed, old, new)

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == code, result.stdout

    @pytest.mark.parametrize(
        "head,code",
        [
            pytest.param("# Soil\n", 1, id="no-contents"),
            pytest.param("# Soil\n\n## Contents\n", 0, id="contents"),
        ],
    )
    def test_long_reference_needs_contents_list(
        self, repo: Path, head: str, code: int
    ) -> None:
        """A changed reference file over 100 lines needs a Contents list.

        An agent often reads only the start of a long file, so without
        a Contents list it misses what the rest holds.
        """
        rows = "\n".join(f"Loam bed {n} holds water." for n in range(100))
        (repo / SOIL).write_text(f"{head}\n{rows}\n")

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == code, result.stdout

    def test_renamed_heading_fails_while_still_quoted(
        self, repo: Path
    ) -> None:
        """Renaming a heading that another skill quotes fails the check.

        A quoted heading is how skills point at a section, so after a
        rename the pointer leads nowhere. The quote wraps across two
        lines, as quotes in 72-column text do.
        """
        quote_soil_heading(repo)
        replace_text(repo / SOIL, "## Testing the soil", "## Soil tests")

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == 1
        assert 'error: quotes "Testing the soil"' in result.stdout

    def test_heading_in_moved_file_passes(self, repo: Path) -> None:
        """Moving a file keeps its quoted headings valid.

        Renaming a skill's reference folder deletes each file at its
        old path, but every heading still exists at the new path. A
        check that compared one path's headings would block the move.
        """
        quote_soil_heading(repo)
        moved = repo / "ai/skills/garden/reference/soil.md"
        moved.parent.mkdir()
        (repo / SOIL).rename(moved)
        replace_text(repo / GARDEN, "references/soil", "reference/soil")

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == 0, result.stdout

    @pytest.mark.parametrize(
        "old,new,summary",
        [
            pytest.param(
                "# Garden\n",
                "# Garden\n\nRake.\n",
                "0 errors, 0 repeats, 1 grown",
                id="line-added",
            ),
            pytest.param(
                "Pass leaves",
                "Pass weeds",
                "0 errors, 0 repeats, 0 grown",
                id="line-changed",
            ),
        ],
    )
    def test_skill_reported_grown_only_when_longer(
        self, repo: Path, old: str, new: str, summary: str
    ) -> None:
        """A SKILL.md with more lines is reported, without failing.

        A skill should grow only for a reason the pull request gives,
        so the author must see each growth. An edit that keeps the
        length needs no reason, and a report for it would teach
        authors to ignore the line.
        """
        replace_text(repo / GARDEN, old, new)

        result = run_check(repo, "--base", "HEAD")

        assert result.returncode == 0, result.stdout
        assert result.stdout.splitlines()[-1] == summary


def commit_new_repo(repo: Path) -> None:
    """Make a Git checkout of the files in repo and commit them.

    Args:
        repo: Folder that holds the sample files.
    """
    env = make_env(repo.parent)
    for command in (["init", "-q"], ["add", "."], ["commit", "-qm", "base"]):
        subprocess.run(["git", *command], cwd=repo, env=env, check=True)


@pytest.fixture
def route_repo(tmp_path: Path) -> Path:
    """A committed checkout with a model-routing skill and its table.

    The table lists garden for kind plan-review, and garden calls
    ``pde_route.py plan-review``. garden links the reference files
    ``build.md`` and ``evaluation.md``. compost calls no kind. The fake
    home has no installed table.

    Args:
        tmp_path: Test folder for the checkout and the fake home.

    Returns:
        The checkout.
    """
    repo = tmp_path / "repo"
    write_skill(repo, "model-routing", "Pick with `pde_route.py <kind>`.")
    table = {
        "plan-review": {
            "effort": "high",
            "skills": ["garden"],
            "opencode": [["claude-code/claude-opus-5-5[1m]"]],
        },
        "web-lookup": {
            "effort": "none",
            "skills": [],
            "opencode": [["openai/gpt-6-luna"]],
        },
    }
    (repo / TABLE).write_text(json.dumps(table, indent=2) + "\n")
    garden = write_skill(repo, "garden", ROUTE_GARDEN_BODY)
    (garden.parent / "references").mkdir()
    (repo / BUILD).write_text("# Build\n")
    (repo / EVALUATION).write_text("# Evaluation\n")
    write_skill(repo, "compost", COMPOST_BODY)
    commit_new_repo(repo)
    return repo


@pytest.fixture
def extra(tmp_path: Path) -> Path:
    """A committed extra with a top-level skills folder and no table.

    Its one skill, garden, has ``extra``. The fake home has no
    installed table either.

    Args:
        tmp_path: Test folder for the checkout and the fake home.

    Returns:
        The checkout.
    """
    repo = tmp_path / "work"
    garden = repo / "skills/garden/SKILL.md"
    garden.parent.mkdir(parents=True)
    text = SKILL.format(name="garden", body=COMPOST_BODY)
    garden.write_text(text.replace("scope: personal", "extra"))
    commit_new_repo(repo)
    return repo


class TestRouteChecks:
    """Checks of ``pde_route.py`` calls, role phrases and model names."""

    @pytest.mark.parametrize(
        "file,old,new,reason",
        [
            pytest.param(
                GARDEN,
                "pde_route.py plan-review",
                "pde_route.py no-such-kind",
                "ai/skills/garden/SKILL.md:14: error: "
                "no route kind named no-such-kind",
                id="unknown-route-kind",
            ),
            pytest.param(
                GARDEN,
                "python3 scripts/pde_route.py plan-review --start "
                "opencode-subagent\n",
                "",
                f"{TABLE}:2: error: kind plan-review lists garden, "
                "which has no call for it",
                id="listed-skill-lacks-call",
            ),
            pytest.param(
                COMPOST,
                COMPOST_BODY,
                f"{COMPOST_BODY}\n\n```text\n"
                "python3 scripts/pde_route.py plan-review\n```",
                "ai/skills/compost/SKILL.md:13: error: "
                "kind plan-review in routing.json does not list compost",
                id="unlisted-skill-calls-kind",
            ),
            pytest.param(
                COMPOST,
                COMPOST_BODY,
                f"{COMPOST_BODY}\nAsk the local model to turn it.",
                'ai/skills/compost/SKILL.md:11: error: "local model" names',
                id="role-phrase-local",
            ),
            pytest.param(
                COMPOST,
                COMPOST_BODY,
                f"{COMPOST_BODY}\nAsk the cheap roles to turn it.",
                'ai/skills/compost/SKILL.md:11: error: "cheap roles" names',
                id="role-phrase-plural-role",
            ),
            pytest.param(
                COMPOST,
                COMPOST_BODY,
                f"{COMPOST_BODY}\nAsk the review model to turn it.",
                'ai/skills/compost/SKILL.md:11: error: "review model" names',
                id="role-phrase-review",
            ),
            pytest.param(
                BUILD,
                "# Build\n",
                "# Build\n\nAsk the local model to build it.\n",
                f'{BUILD}:3: error: "local model" names',
                id="role-phrase-in-reference",
            ),
            pytest.param(
                COMPOST,
                COMPOST_BODY,
                f"{COMPOST_BODY}\nTurn it with claude-code/claude-sonnet-5.",
                "ai/skills/compost/SKILL.md:11: error: "
                "claude-code/claude-sonnet-5. names a model",
                id="model-name-outside-routing",
            ),
        ],
    )
    def test_route_defect_fails_with_its_reason(
        self, route_repo: Path, file: str, old: str, new: str, reason: str
    ) -> None:
        """Each route defect fails the check, naming its file and line.

        A skill that calls an unknown kind starts its agent with no
        model, and a table whose ``skills`` lists drift from the calls
        misleads whoever changes a kind's models. A role phrase or a
        model name in a skill picks a model that the table does not
        control. Reference files that are not evaluation files hold
        real starts, so a skip list that hid them would hide these.
        """
        replace_text(route_repo / file, old, new)

        result = run_check(route_repo, "--base", "HEAD")

        assert result.returncode == 1, result.stdout
        assert reason in result.stdout

    @pytest.mark.parametrize(
        "file,old,new,base",
        [
            pytest.param(
                ROUTING,
                "# Garden\n",
                "# Garden\n\nAsk the review model to plan.\n",
                "HEAD~1",
                id="inside-model-routing",
            ),
            pytest.param(
                GARDEN,
                "# Garden\n",
                "# Garden\n\nAsk the review model to plan.\n",
                "HEAD",
                id="unchanged-line",
            ),
            pytest.param(
                EVALUATION,
                "# Evaluation\n",
                "# Evaluation\n\nBad answer: ask the review model.\n",
                "HEAD~1",
                id="evaluation-file",
            ),
        ],
    )
    def test_role_phrase_passes_where_allowed(
        self, route_repo: Path, file: str, old: str, new: str, base: str
    ) -> None:
        """A role phrase passes in model-routing, old lines and eval files.

        model-routing maps roles to kinds, and evaluation files quote
        wrong answers on purpose. Lines already on main hold role
        phrases until a later change converts them, so failing them
        now would block every skill change. The change is committed,
        so base ``HEAD~1`` makes its line added and ``HEAD`` does not.
        """
        replace_text(route_repo / file, old, new)
        commit_all(route_repo)

        result = run_check(route_repo, "--base", base)

        assert result.returncode == 0, result.stdout

    @pytest.mark.parametrize(
        "call",
        [
            pytest.param("pde_route.py <kind>", id="placeholder-kind"),
            pytest.param("pde_route.py check", id="check-command"),
            pytest.param("pde_route.py audit --since 1d", id="audit-command"),
        ],
    )
    def test_kind_check_skips_placeholders_and_commands(
        self, route_repo: Path, call: str
    ) -> None:
        """A placeholder kind or a command is not read as a kind.

        model-routing's own examples write ``<kind>`` and run the
        ``check`` and ``audit`` commands. Reading those as kinds would
        fail CI on every pull request.
        """
        replace_text(route_repo / GARDEN, "```text\n", f"```text\n{call}\n")

        result = run_check(route_repo, "--base", "HEAD")

        assert result.returncode == 0, result.stdout

    @pytest.mark.parametrize(
        "line,warns",
        [
            pytest.param(
                "Ask the review model, or claude-code/claude-sonnet-5.",
                False,
                id="role-and-model-name",
            ),
            pytest.param(
                "```text\npython3 scripts/pde_route.py plan-review\n```",
                True,
                id="call-warns",
            ),
        ],
    )
    def test_checkout_without_table_skips_route_checks(
        self, extra: Path, line: str, warns: bool
    ) -> None:
        """A extra with no routing table passes the route checks.

        extra sit in a top-level ``skills`` folder with no
        model-routing skill, and CI has no installed table. Failing
        there would block every pull request of a extra. A
        call whose kind cannot be checked is still named on stderr.
        """
        garden = extra / "skills/garden/SKILL.md"
        replace_text(garden, "# Garden\n", f"# Garden\n\n{line}\n")

        result = run_check(extra, "--base", "HEAD")

        assert result.returncode == 0, result.stdout + result.stderr
        assert ("could not check kinds" in result.stderr) == warns

    @pytest.mark.parametrize(
        "kind,reason",
        [
            pytest.param("plan-review", None, id="known-kind"),
            pytest.param(
                "no-such-kind",
                "skills/garden/SKILL.md:13: error: "
                "no route kind named no-such-kind",
                id="unknown-kind",
            ),
        ],
    )
    def test_installed_table_checks_work_kinds_only(
        self, extra: Path, kind: str, reason: str | None
    ) -> None:
        """A extra gets only the kind check from the installed table.

        Every personal machine has the installed table, and its
        ``skills`` lists name personal skills that a extra
        lacks. Running the other route checks there would fail every
        work pull request; dropping the table would let a mistyped
        kind through.
        """
        table = extra.parent / "home/.agents/skills/model-routing"
        table.mkdir(parents=True)
        (table / "routing.json").write_text(
            json.dumps(
                {
                    "plan-review": {
                        "effort": "high",
                        "skills": ["review-plan"],
                        "opencode": [["claude-code/claude-opus-5-5[1m]"]],
                    }
                }
            )
        )
        garden = extra / "skills/garden/SKILL.md"
        line = (
            "Ask the review model, or claude-code/claude-sonnet-5.\n\n"
            f"```text\npython3 scripts/pde_route.py {kind}\n```"
        )
        replace_text(garden, "# Garden\n", f"# Garden\n\n{line}\n")

        result = run_check(extra, "--base", "HEAD")

        output = result.stdout + result.stderr
        if reason is None:
            assert result.returncode == 0, output
            assert "could not check kinds" not in result.stderr
        else:
            assert result.returncode == 1, output
            assert reason in result.stdout
