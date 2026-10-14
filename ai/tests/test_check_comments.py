"""Behavior tests for the comment checker in the code-documentation skill."""

import runpy
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "ai/skills/code-documentation/scripts/check_comments.py"
go_comments = runpy.run_path(str(CHECKER))["go_comments"]


def commit_all(root: Path) -> None:
    """Make ``root`` a Git repository and commit everything in it.

    Args:
        root: Folder to commit; it may be empty.
    """
    git = ["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t"]
    subprocess.run([*git, "init", "-q"], check=True)
    subprocess.run([*git, "add", "."], check=True)
    subprocess.run(
        [*git, "commit", "-qm", "base", "--allow-empty"], check=True
    )


def run_checker(
    repo: Path, *args: str, paths: list[str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run the checker in ``repo``.

    Args:
        repo: The repository folder.
        *args: Extra checker options, such as ``--base``.
        paths: Paths to check; every Go file in ``repo`` when ``None``.

    Returns:
        The finished process, with output as text.
    """
    if paths is None:
        paths = sorted(str(p) for p in repo.iterdir() if p.suffix == ".go")
    return subprocess.run(
        [sys.executable, str(CHECKER), "--repo", str(repo), *args, *paths],
        capture_output=True,
        text=True,
    )


class TestCommentNames:
    """Name each comment after the code below it."""

    def test_method_comment_takes_the_method_name(self) -> None:
        """Name a method's comment ``Recv.Method``, not ``func``.

        A method declaration starts with its receiver, so the plain
        declaration pattern would name its comment ``func``. Filler
        counts only in a sentence about the code, such as one that
        starts with its name, so "Total facilitates ..." would pass.
        """
        source = "// Total adds up the prices.\nfunc (c *Cart) Total() int {\n"

        comments = go_comments(source, "cart.go")

        assert [c.name for c in comments] == ["Cart.Total"]


class TestFiller:
    """Fail on the skill's filler examples in sentences a task added."""

    @pytest.mark.parametrize(
        "comment, fails",
        [
            pytest.param("// Post facilitates the outbox.\n", True, id="code"),
            pytest.param(
                "// Post opens a queue. The caller is responsible for "
                "closing it.\n",
                False,
                id="caller",
            ),
        ],
    )
    def test_filler_fails_only_about_the_code(
        self, tmp_path: Path, comment: str, fails: bool
    ) -> None:
        """Fail on filler about the code, not on facts about the caller.

        The skill lists "is responsible for" as filler, and it also asks
        comments to say who owns what. Failing on "The caller is
        responsible for closing it" would push agents to cut that fact,
        and missing "Post facilitates" lets the filler through.
        """
        commit_all(tmp_path)
        source = "package shop\n\n" + comment + "func Post() {}\n"
        (tmp_path / "post.go").write_text(source)

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == (1 if fails else 0)
        assert ("filler:" in result.stdout) == fails


class TestExistingSentences:
    """Leave the sentences that were there before the task to people."""

    @pytest.mark.parametrize(
        "folder, before, after, added",
        [
            pytest.param(
                "",
                "send.go",
                "send.go",
                "// Send also logs the order.\n",
                id="sentence-added",
            ),
            pytest.param("service", "send.go", "send.go", "", id="subfolder"),
            pytest.param("", "send.go", "queue.go", "", id="file-moved"),
        ],
    )
    def test_older_sentences_stay_report_only(
        self, tmp_path: Path, folder: str, before: str, after: str, added: str
    ) -> None:
        """Only report filler in a sentence that was there at ``--base``.

        Such a sentence may be a person's words, and the agent must not
        rewrite them to pass. The checker compares sentences, not whole
        comments, reads ``--base`` from the folder given as ``--repo``,
        and reads every Go file there, so adding a sentence, a repository
        in a subfolder, or a moved file must not make the old sentence
        new. Questions about old sentences are left out too, so the
        agent is asked only about its own text.
        """
        repo = tmp_path / folder
        repo.mkdir(exist_ok=True)
        old = (
            "package shop\n\n// Send facilitates the queue. It keeps order.\n"
        )
        (repo / before).write_text(old + "func Send() {}\n")
        commit_all(tmp_path)
        (repo / before).unlink()
        (repo / after).write_text(old + added + "func Send() {}\n")

        result = run_checker(repo, "--base", "HEAD")

        assert result.returncode == 0
        assert 'filler: "facilitates" (existing: report only)' in (
            result.stdout
        )
        assert "restated-name" not in result.stdout


class TestHeaders:
    """Keep license headers and generated files out of the agent's way."""

    def test_only_license_headers_are_report_only(
        self, tmp_path: Path
    ) -> None:
        """Report license headers and skip generated files, but not docs.

        A license header above ``package`` and a generated file are text
        no agent should rewrite, even when they are new. A package doc
        comment also sits above ``package``, but an agent writes it, so
        new filler there must still fail; an earlier version let it pass
        as a header.
        """
        commit_all(tmp_path)
        (tmp_path / "send.go").write_text(
            "// Copyright Shop. This facilitates nothing.\n\npackage shop\n"
        )
        (tmp_path / "gen.go").write_text(
            "// Code generated by stringer. DO NOT EDIT.\n\n"
            "package shop\n\n// Red facilitates the palette.\nconst Red = 1\n"
        )
        (tmp_path / "doc.go").write_text(
            "// Package shop seamlessly sells things.\npackage shop\n"
        )

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == 1
        assert "(header: report only)" in result.stdout
        assert 'doc.go:1 shop: filler: "seamlessly"\n' in result.stdout
        assert "gen.go" not in result.stdout


class TestInput:
    """Refuse input the checker cannot check."""

    @pytest.mark.parametrize(
        "path",
        [
            pytest.param("gone.go", id="missing-file"),
            pytest.param("sub", id="directory"),
        ],
    )
    def test_unreadable_input_exits_two(
        self, tmp_path: Path, path: str
    ) -> None:
        """Exit 2 when a given path cannot be checked as a Go file.

        An agent that passes a folder, or a file that is not there,
        would otherwise see exit 0 and believe its comments passed,
        even when they hold new filler.
        """
        commit_all(tmp_path)
        (tmp_path / "post.go").write_text("package shop\n")
        (tmp_path / "sub").mkdir()
        (tmp_path / "sub" / "s.go").write_text(
            "package sub\n\n// S facilitates the queue.\nfunc S() {}\n"
        )

        result = run_checker(
            tmp_path,
            "--base",
            "HEAD",
            paths=[str(tmp_path / "post.go"), str(tmp_path / path)],
        )

        assert result.returncode == 2
