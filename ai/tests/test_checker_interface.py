"""Behavior tests for the commands pde-comment-review runs on the checker."""

import io
import json
import re
import sys
from contextlib import redirect_stdout
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "ai/skills/code-documentation/scripts"))

from comment_checker import INTERFACE, VERSION  # noqa: E402
from comment_checker.cli import main  # noqa: E402
from comment_checker.languages import LANGUAGES, Language  # noqa: E402
from comment_checker.records import Comment  # noqa: E402

SAMPLES = Path(__file__).parent / "comment_samples"


def read_note_comments(source: str, file: str) -> list[Comment]:
    """Return each run of ``;`` lines in a made-up ``.note`` file.

    A run takes the word after ``item`` on an unindented line below it.
    """
    lines = source.splitlines(keepends=True)
    comments = []
    start = None
    for number, line in enumerate([*lines, ""], 1):
        if line.strip().startswith(";"):
            start = start or number
            continue
        if start:
            words = line.split()
            named = line[:1].isalpha() and words[:1] == ["item"]
            comments.append(
                Comment(
                    file=file,
                    line=start,
                    name=words[1] if named else "",
                    text="".join(lines[start - 1 : number - 1]),
                    prose="",
                    code_line=number,
                )
            )
            start = None
    return comments


# A made-up language, which proves a language needs only its module.
NOTE = Language(
    name="note",
    read_comments=read_note_comments,
    join_prose=str.strip,
    generated_marker=re.compile("(?!)"),
    is_writable=lambda comment: True,
    behaviors_not_applying={
        "inside_string": "notes have no strings",
        "unsafe_comment": "notes have no directives and end every line",
        "only_statement_docstring": "notes have no docstrings",
    },
)


def run_command(*args: str) -> tuple[int, dict]:
    """Run a checker command in this process.

    Returns:
        The exit code and the reply printed on stdout.
    """
    out = io.StringIO()
    with redirect_stdout(out):
        code = main(list(args))
    return code, json.loads(out.getvalue() or "{}")


def list_files(repo: Path, *paths: Path) -> tuple[int, dict]:
    """Run ``list`` on ``paths``, relative to ``repo``."""
    return run_command(
        "list", "--interface", "1", "--repo", str(repo), *map(str, paths)
    )


@pytest.fixture(params=[*LANGUAGES, ".note"])
def suffix(request, monkeypatch) -> str:
    """Return each table language, with the made-up one added for the test."""
    monkeypatch.setitem(LANGUAGES, ".note", NOTE)
    return request.param


def list_sample(suffix: str, behavior: str) -> tuple[list, list]:
    """Return the listing of a language's sample and the one expected.

    Skips a behavior the language's module says does not apply.
    """
    language = LANGUAGES[suffix]
    if behavior in language.behaviors_not_applying:
        pytest.skip(language.behaviors_not_applying[behavior])
    folder = SAMPLES / language.name
    sample = folder / f"{behavior}{suffix}"
    assert sample.exists(), (
        f"{language.name} needs {sample.name}, or must list {behavior} "
        "as not applying, with a reason"
    )
    code, reply = list_files(folder, sample)
    assert code == 0
    expected = json.loads((folder / "expected.json").read_text())
    return reply["files"][0]["comments"], expected[behavior]


class TestSharedBehaviors:
    """Each language lists its sample for a behavior as the file beside it.

    pde-comment-review shows the owner exactly what ``list`` says, so a
    language that names or bounds a comment differently would show the
    wrong text or write over the wrong lines.
    """

    def test_comment_takes_name_of_code_below(self, suffix: str) -> None:
        """Name each comment after the code just below it.

        Approvals are stored by name, so a wrong name loses the owner's
        approval or protects the wrong comment.
        """
        listed, expected = list_sample(suffix, "names_after_code")

        assert listed == expected

    def test_comment_in_body_is_listed_unnamed(self, suffix: str) -> None:
        """List a comment inside a function body with an empty name.

        An agent may still send an item for it, found by its text, so the
        listing must show it even though it cannot be approved.
        """
        listed, expected = list_sample(suffix, "unnamed_in_body")

        assert listed == expected

    def test_comment_lines_are_listed_whole(self, suffix: str) -> None:
        """List a comment of several lines as one comment.

        Writing over only some lines of a comment would leave half of the
        old text behind, which the review tool used to refuse itself.
        """
        listed, expected = list_sample(suffix, "whole_comment")

        assert listed == expected

    def test_text_inside_string_is_not_listed(self, suffix: str) -> None:
        """Leave out comment-like lines inside a string.

        Writing over such a line would change the program's data, such as
        a help text, instead of a comment.
        """
        listed, expected = list_sample(suffix, "inside_string")

        assert listed == expected

    def test_unsafe_comment_is_not_writable(self, suffix: str) -> None:
        """Mark a comment that a rewrite could break as not writable.

        Rewording a tool directive could drop it, which changes how the
        code builds or is checked, and a Go comment that ends the file
        with no line end is not whole lines the write can replace.
        """
        listed, expected = list_sample(suffix, "unsafe_comment")

        assert listed == expected

    def test_only_statement_docstring_cannot_be_cut(self, suffix: str) -> None:
        """Mark a docstring that is its body's only statement as not cuttable.

        Cutting it would leave an empty body, which does not parse.
        """
        listed, expected = list_sample(suffix, "only_statement_docstring")

        assert listed == expected


class TestCommands:
    """The tool checks the checker's version and interface first."""

    def test_version_reports_interface_and_languages(self) -> None:
        """Report the version, the interface number and the suffixes read.

        The tool checks this first, so a checker from another install is
        caught before any review is drafted.
        """
        code, reply = run_command("version")

        assert code == 0
        assert reply == {
            "interface": INTERFACE,
            "version": VERSION,
            "languages": [".go", ".py"],
        }

    def test_other_interface_number_is_refused(self, capsys) -> None:
        """Refuse a caller that speaks another interface number.

        A tool and checker from different installs would misread each
        other's JSON, so the message names both numbers and the fix.
        """
        code, reply = run_command("version", "--interface", "2")

        assert code == 2
        assert reply == {}
        error = capsys.readouterr().err
        assert "interface 1, not 2" in error
        assert "pde-installer" in error

    @pytest.mark.parametrize(
        ("exists", "expected_code", "expected_files"),
        [
            pytest.param(
                True,
                0,
                [{"file": "main.rs", "unsupported": True}],
                id="present",
            ),
            pytest.param(False, 2, None, id="missing"),
        ],
    )
    def test_only_existing_unsupported_file_is_named(
        self, tmp_path: Path, exists, expected_code, expected_files
    ) -> None:
        """Mark a code file the checker cannot read yet as unsupported.

        The agent then shows its comments to the owner another way. A
        mistyped path must fail instead, or those comments would be
        silently left out of the review.
        """
        if exists:
            (tmp_path / "main.rs").write_text("// Starts the program.\n")

        code, reply = list_files(tmp_path, tmp_path / "main.rs")

        assert code == expected_code
        assert reply.get("files") == expected_files
