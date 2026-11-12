"""Behavior tests for the skill-development skill's check script."""

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
AGENTS = "ai/AGENTS.md"


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
        name, a reference, a frontmatter name or a Lessons section
        fails before it merges, whoever wrote it and whatever skill
        they loaded.
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
                "",
                "the skill-lessons marker must follow",
                id="lessons-marker-gone",
            ),
        ],
    )
    def test_defect_fails_with_its_reason(
        self, repo: Path, file: str, old: str, new: str, reason: str
    ) -> None:
        """Each kind of defect fails the check and says what is wrong.

        Renaming, merging or copying skills is how these defects
        start. An agent that follows a dead pointer misses the rule
        behind it, and a copied Lessons section saves the user's
        corrections to the wrong skill, with no visible failure.
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

        Skills show made-up skills and links in code examples, and a
        skill in another repository names skills this one installs.
        Failing on those would push authors to delete good examples
        or skip the check.
        """
        replace_text(
            repo / GARDEN,
            "# Garden\n",
            f"# Garden\n\n{new}\n",
        )

        result = run_check(repo)

        assert result.returncode == 0, result.stdout

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
    """Checks that ``--base`` limits to the lines a change adds."""

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
        assert result.stdout.splitlines()[-1] == "0 errors, 2 repeats"
        assert "ai/skills/garden/SKILL.md:8: repeat: " in result.stdout
        assert "ai/skills/mulch/SKILL.md:8: repeat: " in result.stdout

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
