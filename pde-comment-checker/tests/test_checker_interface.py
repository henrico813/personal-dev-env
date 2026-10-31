"""Behavior tests for the commands pde-comment-review runs on the checker."""

import ast
import io
import itertools
import json
import os
import re
import signal
import subprocess
import textwrap
from collections.abc import Iterator
from contextlib import redirect_stdout
from dataclasses import replace
from pathlib import Path
from unittest.mock import Mock

import pytest

from comment_checker import INTERFACE, VERSION, interface
from comment_checker.approvals import unname_unapprovable_comments
from comment_checker.cli import main
from comment_checker.languages import LANGUAGES, LanguageRules
from comment_checker.records import Comment

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
NOTE = LanguageRules(
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


def commit_all(repo: Path) -> str:
    """Commit everything in ``repo``, making it a Git repository first.

    Returns:
        The new commit.
    """
    git = ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t"]
    subprocess.run([*git, "init", "-q"], check=True)
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run([*git, "commit", "-qm", "c", "--allow-empty"], check=True)
    return subprocess.run(
        [*git, "rev-parse", "HEAD"], capture_output=True, text=True
    ).stdout.strip()


def list_changes(
    repo: Path, base: str, approved: dict[str, str] | None = None
) -> tuple[int, dict]:
    """Run ``changes`` on ``repo`` with ``approved`` as its approvals.

    Args:
        approved: Approved text by comment name, all in ``shop.go``.
    """
    entries = [
        {"file": "shop.go", "name": name, "text": text}
        for name, text in (approved or {}).items()
    ]
    settings = {"approved": entries, "avoid": {}}
    (repo / ".code-comments.json").write_text(json.dumps(settings))
    return run_command(
        "changes", "--interface", "1", "--repo", str(repo), "--base", base
    )


TOTAL = "// Returns the sum.\nfunc Total() int { return 0 }\n"


class TestChanges:
    """List what the owner must answer since the branch's base."""

    @pytest.mark.parametrize(
        ("after", "approved", "expected"),
        [
            pytest.param(
                TOTAL + "// Empties the cart.\nfunc Reset() {}\n",
                {},
                [("Reset", "new", "// Empties the cart.\n")],
                id="new",
            ),
            pytest.param(
                TOTAL.replace("sum", "total"),
                {},
                [("Total", "changed", "// Returns the total.\n")],
                id="changed",
            ),
            pytest.param(
                TOTAL.replace("sum", "total"),
                {"Total": "// Returns the sum.\n"},
                [("Total", "approved_edited", "// Returns the sum.\n")],
                id="approved-edited",
            ),
            pytest.param(
                TOTAL.replace("Total", "Sum"),
                {"Total": "// Returns the sum.\n"},
                [("Sum", "approved_renamed", "// Returns the sum.\n")],
                id="approved-renamed",
            ),
            pytest.param(
                "",
                {"Total": "// Returns the sum.\n"},
                [("Total", "code_removed", "// Returns the sum.\n")],
                id="code-removed",
            ),
            pytest.param(
                TOTAL + "func Reset() {}\n",
                {"Total": "// Returns the sum.\n"},
                [],
                id="approved-unchanged",
            ),
        ],
    )
    def test_lists_each_comment_owner_must_answer(
        self, tmp_path: Path, after, approved, expected
    ) -> None:
        """Give each comment the owner must answer, with its reason.

        The review shows the owner the approved text as ``old``, so a
        wrong reason or old text would ask them about the wrong words,
        and a missed comment would never reach them.
        """
        (tmp_path / "shop.go").write_text("package shop\n\n" + TOTAL)
        base = commit_all(tmp_path)
        (tmp_path / "shop.go").write_text("package shop\n\n" + after)

        code, reply = list_changes(tmp_path, base, approved)

        assert code == 0
        listed = [(i["name"], i["reason"], i["old"]) for i in reply["items"]]
        assert listed == expected

    def test_renamed_code_keeps_its_approval(self, tmp_path: Path) -> None:
        """Return the rename that moves an approval to the code's new name.

        Without it, the write would record the old name as cut and leave
        the renamed comment unprotected.
        """
        (tmp_path / "shop.go").write_text("package shop\n\n" + TOTAL)
        base = commit_all(tmp_path)
        subprocess.run(
            ["git", "-C", str(tmp_path), "mv", "shop.go", "store.go"],
            check=True,
        )
        store = tmp_path / "store.go"
        store.write_text(store.read_text().replace("Total", "Sum"))

        code, reply = list_changes(
            tmp_path, base, {"Total": "// Returns the sum.\n"}
        )

        assert code == 0
        assert reply["renames"] == ["Total=Sum", "shop.go=store.go"]

    def test_cut_comment_back_is_refused(self, tmp_path: Path) -> None:
        """Refuse when a comment the owner cut is back in the code.

        The owner already answered it, so the agent must delete it again
        before any review is drafted.
        """
        (tmp_path / "shop.go").write_text("package shop\n\n" + TOTAL)
        base = commit_all(tmp_path)

        code, reply = list_changes(tmp_path, base, {"Total": ""})

        assert code == 1
        assert reply["items"] == []
        assert reply["problems"] == [
            "shop.go Total: the owner cut this comment. Delete it from the "
            "code."
        ]

    @pytest.mark.parametrize(
        ("after", "approved", "problem"),
        [
            pytest.param(
                TOTAL + "// Sets up the shop.\nfunc init() {}\n" * 2,
                {"init": "// Sets up the shop.\n"},
                "shop.go init: the owner approved this comment, and now more "
                "than one comment in the file has its name, so none of them "
                "can be told apart. Keep a comment on only one of them.",
                id="shared-name",
            ),
            pytest.param(
                TOTAL.replace("Total", "Sum") + TOTAL.replace("Total", "Add"),
                {"Total": "// Returns the sum.\n"},
                "shop.go Total: the owner approved this comment, and it is "
                "gone. It may now be Sum or Add, but that is not certain, so "
                "none is guessed. Reword the first sentence of each comment "
                "that is not the renamed one.",
                id="uncertain-rename",
            ),
        ],
    )
    def test_unsure_gone_approval_is_refused(
        self, tmp_path: Path, after, approved, problem
    ) -> None:
        """Refuse when a gone approval has no sure new place.

        Its name is now shared, or it may be under one of several new
        names. Listing it as removed code instead would let the owner's
        text lose its protection without anyone noticing.
        """
        (tmp_path / "shop.go").write_text("package shop\n\n" + TOTAL)
        base = commit_all(tmp_path)
        (tmp_path / "shop.go").write_text("package shop\n\n" + after)

        code, reply = list_changes(tmp_path, base, approved)

        assert code == 1
        assert reply["items"] == []
        assert reply["problems"] == [problem]

    def test_directive_comment_is_left_out(self, tmp_path: Path) -> None:
        """Leave a new comment holding a tool directive out of the items.

        Rewording it could turn a line such as ``//go:generate`` into
        prose, which changes how the code builds.
        """
        (tmp_path / "shop.go").write_text("package shop\n\n" + TOTAL)
        base = commit_all(tmp_path)
        (tmp_path / "shop.go").write_text(
            "package shop\n\n"
            + TOTAL
            + "// Empties the cart.\nfunc Reset() {}\n"
            + "// Names a cart state.\n//go:generate stringer -type=State\n"
            + "type State int\n"
        )

        code, reply = list_changes(tmp_path, base)

        assert code == 0
        assert [i["name"] for i in reply["items"]] == ["Reset"]

    def test_new_python_docstring_is_listed(self, tmp_path: Path) -> None:
        """List a new Python docstring for the owner like a Go comment.

        A docstring left out of this list would stay in the code without
        the owner ever seeing it, and no later step would notice.

        Args:
            tmp_path: A folder the test turns into a Git repository.
        """
        base = commit_all(tmp_path)
        (tmp_path / "shop.py").write_text('"""Sells things."""\n')

        code, reply = list_changes(tmp_path, base)

        assert code == 0
        listed = [(i["name"], i["reason"]) for i in reply["items"]]
        assert listed == [("<module>", "new")]

    def test_unsupported_changed_file_is_named(self, tmp_path: Path) -> None:
        """Name a changed code file the checker cannot read yet.

        The agent then shows its comments to the owner another way, since
        no item can be made for them.
        """
        base = commit_all(tmp_path)
        (tmp_path / "main.rs").write_text("// Starts the program.\n")

        code, reply = list_changes(tmp_path, base)

        assert code == 0
        assert reply["unsupported"] == ["main.rs"]


def show_entries(repo: Path, *entries: dict[str, str]) -> tuple[int, dict]:
    """Run ``show`` in ``repo`` with ``entries`` on stdin."""
    with pytest.MonkeyPatch.context() as patch:
        patch.setattr("sys.stdin", io.StringIO(json.dumps(entries)))
        return run_command("show", "--interface", "1", "--repo", str(repo))


SHOP_GO = """package shop

type Cart struct {
\t// Items in the cart.
\tItems []string
}

//go:generate stringer -type=Color
// Color is a paint color.
type Color int
"""

SHOP_PY = """class Cart:
    # Items in the cart.
    items = []
"""

# Its first Go line, after a tab and "// ", ends exactly at column 79.
LONG_PLAIN = (
    "Returns the sum of prices, in cents, so a total of many items never "
    "gets rounded.\n\n\tTotal(prices)\n"
)


class TestShow:
    """Turn comments into what the editor and the screen show, and back.

    The review tool will draw these rows on the owner's phone and write
    this text into the code, so it must match what it does for Go today.
    """

    @pytest.mark.parametrize(
        ("file", "marker"),
        [
            pytest.param("shop.go", "//", id="go"),
            pytest.param("shop.py", "#", id="python"),
        ],
    )
    def test_comment_shows_as_plain_and_rows(
        self, tmp_path: Path, file: str, marker: str
    ) -> None:
        """Show sentences as one row each, and example lines as written.

        The owner edits the plain text, so an example line must keep its
        indent there, or writing it back would join it to the sentences.
        """
        text = (
            f"\t{marker} Sums the\n\t{marker} prices.\n\t{marker}\n"
            f"\t{marker}\tTotal(prices)  \n\t{marker}  return 0\n"
            f"\t{marker} In cents.\n"
        )

        code, reply = show_entries(tmp_path, {"file": file, "text": text})

        assert code == 0
        marks = {"first": f"{marker} ", "rest": f"{marker} "}
        assert reply["shown"] == [
            {
                "file": file,
                "name": "",
                "text": text,
                "plain": "Sums the prices.\n\n\tTotal(prices)\n  return 0\n\n"
                "In cents.\n",
                "screen": [
                    {**marks, "words": "Sums the prices."},
                    {"fixed": marker},
                    {"fixed": f"{marker}\tTotal(prices)"},
                    {"fixed": f"{marker}  return 0"},
                    {"fixed": marker},
                    {**marks, "words": "In cents."},
                ],
            }
        ]

    @pytest.mark.parametrize(
        ("comment", "expected"),
        [
            pytest.param(
                {"file": "shop.go", "name": "Cart.Items", "plain": LONG_PLAIN},
                "\t// Returns the sum of prices, in cents, so a total of many"
                " items never gets\n\t// rounded.\n\t//\n"
                "\t//\tTotal(prices)\n",
                id="go-by-name",
            ),
            pytest.param(
                {
                    "file": "shop.go",
                    "text": "\t// Old.\n",
                    "plain": "Returns the sum of prices, in cents, so a total "
                    "of many items never rounds.\n",
                },
                "\t// Returns the sum of prices, in cents, so a total of many"
                " items never\n\t// rounds.\n",
                id="go-by-text-tab",
            ),
            pytest.param(
                {
                    "file": "shop.py",
                    "name": "# Cart.items",
                    "plain": LONG_PLAIN,
                },
                "    # Returns the sum of prices, in cents, so a total of many"
                " items\n    # never gets rounded.\n    #\n"
                "    #\tTotal(prices)\n",
                id="python-by-name",
            ),
            pytest.param(
                {"file": "shop.go", "name": "Cart.Items", "plain": "\n"},
                "",
                id="no-words-cuts",
            ),
        ],
    )
    def test_plain_is_written_like_old_comment(
        self, tmp_path: Path, comment: dict[str, str], expected: str
    ) -> None:
        """Write new words at the language's width and the old indent.

        Go comments wrap at 79 columns and Python comments at 72. A
        wrong indent puts the comment out of line with its code. A wrong
        width leaves lines too long. Empty new words delete the comment
        instead of leaving an empty one.

        Args:
            tmp_path: An empty folder that gets the Go and Python files.
            comment: The comment to write, found by name or by its text,
                with its new words.
            expected: The comment the checker should write.
        """
        (tmp_path / "shop.go").write_text(SHOP_GO)
        (tmp_path / "shop.py").write_text(SHOP_PY)

        code, reply = show_entries(tmp_path, comment)

        assert code == 0
        assert reply["shown"][0]["text"] == expected

    @pytest.mark.parametrize(
        ("entry", "problem"),
        [
            pytest.param(
                {"file": "shop.go", "name": "Total", "plain": "New.\n"},
                "shop.go Total: no comment has this name in the file",
                id="missing-name",
            ),
            pytest.param(
                {"file": "shop.go", "name": "Color", "plain": "New.\n"},
                "shop.go Color: a rewrite could change how the code builds "
                "or is checked",
                id="directive",
            ),
            pytest.param(
                {"file": "shop.go", "text": "x := 1\n"},
                "shop.go: line 1 is not a // comment",
                id="text-not-a-comment",
            ),
            pytest.param(
                {"file": "shop.go", "text": "x := 1\n", "plain": "New.\n"},
                "shop.go: line 1 is not a // comment",
                id="layout-not-a-comment",
            ),
            pytest.param(
                {"file": "crlf.go", "name": "Cart.Items", "plain": "New.\n"},
                "crlf.go Cart.Items: cannot be read (it has a carriage "
                "return)",
                id="carriage-return",
            ),
            pytest.param(
                {
                    "file": "shop.py",
                    "text": "#!/usr/bin/env python3\n",
                    "plain": "New.\n",
                },
                "shop.py: a rewrite could change how the code builds or is "
                "checked",
                id="shebang",
            ),
            pytest.param(
                {"file": "shop.py", "text": "total = 0\n"},
                "shop.py: is neither # lines nor one whole docstring",
                id="python-not-a-comment",
            ),
            pytest.param(
                {"file": "shop.py", "text": '"a" "b"\n'},
                "shop.py: is neither # lines nor one whole docstring",
                id="joined-string-literals",
            ),
        ],
    )
    def test_unsafe_entry_is_refused(
        self, tmp_path: Path, entry: dict[str, str], problem: str
    ) -> None:
        """Refuse a comment the checker cannot show or safely lay out.

        Rewording a directive or shebang would change how the code builds
        or runs, a file with carriage returns would get mixed line ends,
        and drawing code as a comment would show the owner the wrong text.
        """
        (tmp_path / "shop.go").write_text(SHOP_GO)
        (tmp_path / "crlf.go").write_bytes(
            SHOP_GO.replace("\n", "\r\n").encode()
        )

        code, reply = show_entries(tmp_path, entry)

        assert code == 1
        assert reply["shown"] == []
        assert reply["problems"] == [problem]

    def test_one_bad_entry_refuses_whole_request(self, tmp_path: Path) -> None:
        """Refuse every entry when one fails, naming only the failing one.

        The tool shows or writes a whole review at once, so a reply that
        left out one comment would show the owner an incomplete review.
        """
        code, reply = show_entries(
            tmp_path,
            {"file": "shop.go", "text": "// Sums.\n"},
            {"file": "shop.go", "text": "x := 1\n"},
        )

        assert code == 1
        assert reply["shown"] == []
        assert reply["problems"] == ["shop.go: line 1 is not a // comment"]

    @pytest.mark.parametrize(
        ("request_body", "error"),
        [
            pytest.param(
                {"file": "shop.go", "text": "// Sums.\n"},
                "show reads a JSON list",
                id="not-a-list",
            ),
            pytest.param(
                [{"file": "main.rs", "text": "// Sums.\n"}],
                "main.rs: show does not take this language",
                id="unsupported-suffix",
            ),
            pytest.param(
                [{"file": "gone.go", "name": "Total", "plain": "Sums.\n"}],
                "gone.go: [Errno 2]",
                id="missing-file",
            ),
        ],
    )
    def test_bad_request_exits_two(
        self, tmp_path: Path, capsys, request_body, error: str
    ) -> None:
        """Exit 2 with the reason on stderr for a request show cannot take.

        The review tool tells a setup or usage error from a refusal by
        the exit code, so these must not look like a reply.
        """
        with pytest.MonkeyPatch.context() as patch:
            patch.setattr("sys.stdin", io.StringIO(json.dumps(request_body)))
            code, reply = run_command(
                "show", "--interface", "1", "--repo", str(tmp_path)
            )

        assert code == 2
        assert reply == {}
        assert error in capsys.readouterr().err


DOCSTRING = '''    r"""Sum the prices.

    Prices are in cents,
    so totals never round.

    Use total() \\
    for sums.

    For example::

        total([])

    Args:
        prices: The prices, which may
            be empty.
        cart: The cart.

    Example:
        total([1, 2]) \\
            + 3
    """
'''


class TestShowDocstrings:
    """Lay out Python docstrings for the editor, the screen and the file.

    Docstrings hold sections and examples that Go comments do not, so a
    wrong rule would join an example into a sentence or break the code.
    """

    def test_docstring_shows_sections_and_examples(
        self, tmp_path: Path
    ) -> None:
        """Show each paragraph and entry as one row, and examples as written.

        The owner reads these rows on the phone, and the quotes show where
        the docstring starts and ends.
        """
        code, reply = show_entries(
            tmp_path, {"file": "shop.py", "text": DOCSTRING}
        )

        assert code == 0
        shown = reply["shown"][0]
        assert shown["plain"] == (
            "Sum the prices.\n\nPrices are in cents, so totals never round.\n"
            "\nUse total() \\\nfor sums.\n\nFor example::\n\n    total([])\n"
            "\nArgs:\nprices: The prices, which may be empty.\n"
            "cart: The cart.\n\nExample:\n    total([1, 2]) \\\n        + 3\n"
        )
        paragraph = {"first": "", "rest": ""}
        entry = {"first": "", "rest": "  "}
        assert shown["screen"] == [
            {"first": 'r"""', "rest": "", "words": "Sum the prices."},
            {"fixed": ""},
            {
                **paragraph,
                "words": "Prices are in cents, so totals never round.",
            },
            {"fixed": ""},
            {"fixed": "Use total() \\"},
            {**paragraph, "words": "for sums."},
            {"fixed": ""},
            {**paragraph, "words": "For example::"},
            {"fixed": ""},
            {"fixed": "    total([])"},
            {"fixed": ""},
            {"fixed": "Args:"},
            {**entry, "words": "prices: The prices, which may be empty."},
            {**entry, "words": "cart: The cart."},
            {"fixed": ""},
            {"fixed": "Example:"},
            {"fixed": "    total([1, 2]) \\"},
            {"fixed": "        + 3"},
            {"fixed": '"""'},
        ]

    def test_one_line_docstring_draws_one_row(self, tmp_path: Path) -> None:
        """Draw a one-line docstring on one row, as in the code.

        Closing quotes on a row of their own made a one-line docstring
        look like two lines.

        Args:
            tmp_path: An empty folder the checker runs in.
        """
        text = '        """Sum the prices."""\n'

        code, reply = show_entries(tmp_path, {"file": "shop.py", "text": text})

        assert code == 0
        assert reply["shown"][0]["screen"] == [
            {"first": '"""', "rest": "", "words": 'Sum the prices."""'}
        ]

    @pytest.mark.parametrize(
        ("old", "plain", "expected"),
        [
            pytest.param(
                DOCSTRING,
                "Sum the prices.\n\nArgs:\nprices: The prices, in cents, of "
                "every item in the cart, which may be empty.\n\nIn cents.\n",
                '    r"""Sum the prices.\n\n    Args:\n        prices: The '
                "prices, in cents, of every item in the cart, which\n"
                '            may be empty.\n\n    In cents.\n    """\n',
                id="sections",
            ),
            pytest.param(
                "\t'Sum.'\n",
                "Sum the prices.\n",
                "\t'Sum the prices.'\n",
                id="one-line",
            ),
            pytest.param(
                "\t'Sum.'\n",
                "Sum the prices.\n\nIn cents.\n",
                "\t'''Sum the prices.\n\n\tIn cents.\n\t'''\n",
                id="single-quotes-grow",
            ),
            pytest.param(
                "\t'Sum.'\n",
                "Sum the cart's prices.\n",
                "\t'''Sum the cart's prices.'''\n",
                id="holds-its-quote",
            ),
            pytest.param(
                '"""Sum."""\n',
                'Sum the "prices"\n',
                '"""Sum the "prices"\n"""\n',
                id="ends-in-quote",
            ),
            pytest.param(
                '    """Sum at C:\\ prices."""\n',
                "Sum the prices.\n\nReads the price list from the shared "
                "folder on the north drive at C:\\ prices daily.\n",
                '    """Sum the prices.\n\n    Reads the price list from the '
                "shared folder on the north drive at\n"
                '    C:\\ prices daily.\n    """\n',
                id="backslash-keeps-next-word",
            ),
            pytest.param(
                '    """Sum at caf\\N{LATIN SMALL LETTER E WITH ACUTE}."""\n',
                "Sum the prices.\n\nPrices are listed in euros, and the name "
                "of the cafe is written caf\\N{LATIN SMALL LETTER E WITH "
                "ACUTE} here.\n",
                '    """Sum the prices.\n\n    Prices are listed in euros, '
                "and the name of the cafe is written\n"
                "    caf\\N{LATIN SMALL LETTER E WITH ACUTE} here.\n"
                '    """\n',
                id="named-escape-stays-whole",
            ),
            pytest.param(
                '"""Sum."""\n',
                "Match \\\\d.\n",
                '"""Match \\\\d."""\n',
                id="doubled-backslash",
            ),
            pytest.param(
                '    r"""Sum."""\n',
                "Match \\d.\n",
                '    r"""Match \\d."""\n',
                id="raw-keeps-backslash",
            ),
            pytest.param('"""Sum."""\n', "\n", "", id="no-words-cuts"),
        ],
    )
    def test_plain_is_written_as_docstring(
        self, tmp_path: Path, old: str, plain: str, expected: str
    ) -> None:
        """Write new words keeping the old indent, prefix and quotes.

        A lost ``r`` prefix changes what a backslash means, and a quote
        next to the closing quotes would end the string early. The
        checker refuses an escape the old docstring lacks, so two cases
        start from an old docstring that already holds theirs.

        Args:
            tmp_path: An empty folder the checker runs in.
            old: The docstring in the file now.
            plain: The new words, as the owner types them.
            expected: The docstring the checker should return.
        """
        code, reply = show_entries(
            tmp_path, {"file": "shop.py", "text": old, "plain": plain}
        )

        assert code == 0
        assert reply["shown"][0]["text"] == expected

    @pytest.mark.parametrize(
        ("plain", "problem"),
        [
            pytest.param(
                'A """ B\n',
                'shop.py: the text holds """, which ends the docstring',
                id="closing-quotes",
            ),
            pytest.param(
                "A\rB\n",
                "shop.py: the text holds a carriage return or a NUL",
                id="carriage-return",
            ),
            pytest.param(
                "A\0B\n",
                "shop.py: the text holds a carriage return or a NUL",
                id="nul",
            ),
            pytest.param(
                "Match \\d.\n",
                "shop.py: the text holds \\d, which Python warns about; "
                "double the backslash to keep it",
                id="unknown-escape",
            ),
            pytest.param(
                "Save to C:\\new.\n",
                "shop.py: the text holds \\n, which Python reads as another "
                "character; double the backslash to keep it",
                id="known-escape",
            ),
            pytest.param(
                "Sum the prices \\\nand the tax.\n",
                "shop.py: the text holds a backslash at a line end, which "
                "joins the next line; double the backslash to keep it",
                id="line-end-backslash",
            ),
        ],
    )
    def test_unwritable_plain_text_is_refused(
        self, tmp_path: Path, plain: str, problem: str
    ) -> None:
        """Refuse new wording that would break or change the docstring.

        The owner types on a phone and cannot see that ``C:\\new``
        becomes a line break. Refusing at edit time lets the owner fix
        the text before answering more comments.

        Args:
            tmp_path: An empty folder the checker runs in.
            plain: The new wording, as the owner types it.
            problem: The refusal the checker should give.
        """
        code, reply = show_entries(
            tmp_path,
            {"file": "shop.py", "text": '"""Sum."""\n', "plain": plain},
        )

        assert code == 1
        assert reply["problems"] == [problem]

    def test_repository_docstrings_keep_plain_text(self) -> None:
        """Write every docstring here from its plain text without losing any.

        Each rewrite must still be one string with the same plain text,
        so an owner's edit of any real docstring keeps its examples and
        sections and leaves the file parsing.
        """
        root = Path(__file__).resolve().parents[2]
        files = subprocess.run(
            ["git", "-C", str(root), "ls-files", "*.py"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.split()
        code, listed = list_files(root, *(root / file for file in files))
        docstrings = [
            {"file": entry["file"], "text": comment["text"]}
            for entry in listed["files"]
            for comment in entry["comments"]
            if comment["writable"] and not comment["text"].lstrip()[:1] == "#"
        ]

        shown = show_entries(root, *docstrings)[1]["shown"]
        rewritten = show_entries(
            root,
            *(
                {"file": s["file"], "text": s["text"], "plain": s["plain"]}
                for s in shown
            ),
        )[1]["shown"]

        assert code == 0
        assert len(docstrings) > 400
        for before, after in zip(shown, rewritten):
            assert after["plain"] == before["plain"], before["text"]
            module = ast.parse(textwrap.dedent(after["text"]))
            assert isinstance(module.body[0], ast.Expr), after["text"]
            assert len(module.body) == 1, after["text"]


CART_GO = """package cart

const (
\tMax = 50
\t// Prices are in cents, so sums never round.
\tCurrency = "USD"
)

// Sums the prices.
func Total(prices []int) int { return 0 }

// Reports whether a price is too high,
// over 1000 cents.
func TooHigh(price int) bool { return price > 1000 }

// Reports whether every price is above zero.
func Valid(prices []int) bool { return true }

// Fills the cart.
//
//go:noinline
func Fill() {
\t// Adds one item.
\t//nolint:ineffassign
\t_ = 1
}

// Deprecated: use Total.
func Sum() {}

// Deprecated: use Total.
func Add() {}

var help = `
// Sample help text.
`
"""

TOTAL_TEXT = "// Sums the prices.\n"
VALID_TEXT = "// Reports whether every price is above zero.\n"
ANSWER = "each answer needs an id, a plain relative file"
NEW_TEXT = "the new text could change how the code builds or is checked"
SETTINGS = ".code-comments.json"
# A removed-code answer, which has no current text; None drops a field.
REMOVED = dict(code_removed=True, cut=True, current=None, text=None, name="S")
APPROVED = {
    "approved": [
        {"file": "cart.go", "name": "Valid", "text": VALID_TEXT},
        {"file": "cart.go", "name": "Reset", "text": ""},
        {"file": "tax.go", "name": "Check", "text": "// Checks the tax.\n"},
    ],
    "avoid": {},
}


def make_answer(item: int, current: str, text: str | None = None, /, **fields):
    """Return an answer in ``cart.go`` keeping or rewriting ``current``.

    ``text=""`` cuts the comment, and ``fields`` add or replace fields,
    such as ``name``.
    """
    entry = {"id": item, "file": "cart.go", "old": current, "current": current}
    if text == "":
        entry["cut"] = True
    else:
        entry["text"] = current if text is None else text
    return {**entry, **fields}


def apply_answers(
    repo: Path,
    answers: list[dict],
    renames: tuple[str, ...] = (),
    flags: tuple[str, ...] = ("--check",),
) -> tuple[int, dict]:
    """Run ``apply`` on ``answers`` and ``renames``, by default to check."""
    request = {"interface": 1, "renames": list(renames), "answers": answers}
    with pytest.MonkeyPatch.context() as patch:
        patch.setattr("sys.stdin", io.StringIO(json.dumps(request)))
        return run_command(
            "apply", *flags, "--interface", "1", "--repo", str(repo)
        )


def write_cart(repo: Path, source: str = CART_GO) -> None:
    """Write ``cart.go`` and the owner's approvals into ``repo``."""
    (repo / "cart.go").write_text(source)
    (repo / ".code-comments.json").write_text(json.dumps(APPROVED))


class TestApplyCheck:
    """Refuse every answer the write could not make safely, before writing.

    The review tool will run this before the owner answers and again
    before the write, so a refusal missed here would write over code or
    approve text the owner never saw.
    """

    def test_safe_answers_pass(self, tmp_path: Path) -> None:
        """Accept a cut, a reword, keeps and an approved comment kept.

        These are the answers of an ordinary review, including a comment
        above a lint directive, which the write leaves in place. A false
        refusal here would block a review the write should accept.
        """
        write_cart(tmp_path)
        answers = [
            make_answer(
                1, "\t// Prices are in cents, so sums never round.\n", ""
            ),
            make_answer(2, TOTAL_TEXT, "// Returns the sum.\n", name="Total"),
            make_answer(4, VALID_TEXT),
            make_answer(5, "\t// Adds one item.\n"),
        ]

        code, reply = apply_answers(tmp_path, answers)

        assert (code, reply["problems"]) == (0, [])
        assert reply["changed"] == ["cart.go"]
        assert (tmp_path / "cart.go").read_text() == CART_GO

    @pytest.mark.parametrize(
        ("on_disk", "answers", "problem"),
        [
            pytest.param(
                CART_GO,
                [
                    make_answer(
                        3, "// Reports whether a price is over the limit.\n"
                    )
                ],
                "cart.go: item 3 changed in the file after it was added",
                id="kept-comment-edited",
            ),
            pytest.param(
                CART_GO,
                [make_answer(2, "// Sums all prices.\n", name="Total")],
                "cart.go: item 2 changed in the file after it was added",
                id="named-comment-edited",
            ),
            pytest.param(
                CART_GO,
                [
                    make_answer(1, TOTAL_TEXT, name="Total"),
                    make_answer(2, TOTAL_TEXT),
                ],
                "items 1 and 2 are the same comment in cart.go",
                id="same-comment-twice",
            ),
            pytest.param(
                CART_GO,
                [make_answer(1, "// Deprecated: use Total.\n", "")],
                "item 1: cart.go: comment found 2 times, at lines 28, 31",
                id="comment-repeated",
            ),
            pytest.param(
                CART_GO,
                [make_answer(1, "Sums the prices.\n", "Sums.\n")],
                "item 1: cart.go: comment does not start a line",
                id="mid-line",
            ),
            pytest.param(
                CART_GO,
                [make_answer(1, "// over 1000 cents.\n", "")],
                "item 1: cart.go: is only part of a comment",
                id="part-of-comment",
            ),
            pytest.param(
                CART_GO,
                [make_answer(1, "// Sample help text.\n", "")],
                "item 1: cart.go: is not a comment when the checker reads the "
                "file, such as text inside a string",
                id="inside-string",
            ),
            pytest.param(
                CART_GO,
                [
                    make_answer(
                        1,
                        "// Fills the cart.\n//\n//go:noinline\n",
                        name="Fill",
                    )
                ],
                "item 1: cart.go Fill: a rewrite could change how the code "
                "builds or is checked",
                id="directive",
            ),
            pytest.param(
                CART_GO,
                [make_answer(2, TOTAL_TEXT, "// Sums.\n//go:noinline\n")],
                "item 2: cart.go Total: the new text could change how the "
                "code builds or is checked",
                id="text-adds-directive",
            ),
            pytest.param(
                CART_GO.replace("Max = 50", "Max  =  50"),
                [make_answer(2, TOTAL_TEXT, "// Returns the sum.\n")],
                "gofmt would change cart.go, so the write would change more "
                "than comments",
                id="unformatted-file",
            ),
            pytest.param(
                CART_GO,
                [make_answer(2, TOTAL_TEXT, "// Sums.\nfunc Parse() {}\n")],
                "cart.go: the answers would not read back as the same "
                "comments",
                id="text-holds-code",
            ),
            pytest.param(
                CART_GO,
                [
                    make_answer(
                        1,
                        TOTAL_TEXT
                        + "func Total(prices []int) int { return 0 }\n",
                    )
                ],
                "item 1: cart.go: holds lines that are not part of the "
                "comment",
                id="trailing-code",
            ),
            pytest.param(
                CART_GO,
                [make_answer(1, "// Sums the prices.")],
                "item 1: cart.go: must be whole comment lines ending in a "
                "newline",
                id="missing-newline",
            ),
        ],
    )
    def test_unsafe_answer_is_refused(
        self, tmp_path: Path, on_disk: str, answers, problem: str
    ) -> None:
        """Refuse an answer that would write over code or the wrong comment.

        The tool shows these problems to the agent, so each must name its
        item and say what to fix. The review tool's messages are kept, as
        its tests and the agents' skills read them.
        """
        (tmp_path / "cart.go").write_text(on_disk)

        code, reply = apply_answers(tmp_path, answers)

        assert (code, reply["changed"]) == (1, [])
        assert len(reply["problems"]) == 1
        assert problem in reply["problems"][0]

    @pytest.mark.parametrize(
        ("on_disk", "answers", "renames", "problem"),
        [
            pytest.param(
                CART_GO.replace("above zero", "not negative"),
                [
                    make_answer(
                        4, VALID_TEXT.replace("above zero", "not negative")
                    )
                ],
                (),
                "item 4: cart.go Valid: differs from the text the owner "
                "approved",
                id="approved-comment-edited",
            ),
            pytest.param(
                CART_GO + "\n// Empties the cart.\nfunc Reset() {}\n",
                [make_answer(4, "// Empties the cart.\n")],
                (),
                "item 4: cart.go Reset: the owner cut this comment",
                id="cut-comment-back",
            ),
            pytest.param(
                CART_GO.replace(VALID_TEXT, ""),
                [make_answer(2, TOTAL_TEXT)],
                (),
                "cart.go Valid: the owner's approved comment is gone",
                id="approved-comment-gone",
            ),
            pytest.param(
                CART_GO,
                [make_answer(2, TOTAL_TEXT)],
                ("Check=Sum",),
                "rename Check would move no approved comment",
                id="rename-moves-nothing",
            ),
            pytest.param(
                CART_GO,
                [make_answer(4, TOTAL_TEXT, name="Total", old=VALID_TEXT)],
                (),
                "item 4: cart.go Total: no comment by this name has the "
                "item's old",
                id="named-after-another",
            ),
            pytest.param(
                CART_GO,
                [
                    {
                        "id": 4,
                        "file": "cart.go",
                        "name": "Valid",
                        "old": VALID_TEXT,
                        "code_removed": True,
                        "cut": True,
                    }
                ],
                (),
                "item 4: cart.go Valid: its removed code is back",
                id="removed-code-back",
            ),
        ],
    )
    def test_unseen_text_is_not_approved(
        self, tmp_path: Path, on_disk: str, answers, renames, problem: str
    ) -> None:
        """Refuse an answer whose approval would cover text never shown.

        Approving is the owner's lock on a comment, so an approval moved
        to the wrong name, or of an agent's edit, would protect text the
        owner never read. The review tool's messages are kept.
        """
        write_cart(tmp_path, on_disk)

        code, reply = apply_answers(tmp_path, answers, renames)

        assert (code, reply["changed"]) == (1, [])
        assert len(reply["problems"]) == 1
        assert problem in reply["problems"][0]

    @pytest.mark.parametrize(
        "flags",
        [pytest.param(("--check",), id="check"), pytest.param((), id="write")],
    )
    def test_reviews_off_language_is_refused(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        flags: tuple[str, ...],
    ) -> None:
        """Write no comment in a language whose reviews are off.

        A new language starts with reviews off, because the owner has
        not yet checked how the review screen shows its comments. The
        review tool has no list of languages, so only the checker keeps
        an agent's hand-written change out of a file in that language.

        Args:
            tmp_path: An empty folder the checker writes in.
            monkeypatch: Adds the made-up ``.note`` language for this
                test.
            flags: Extra arguments, so the run that only checks and the
                run that writes are both tested.
        """
        writable = replace(NOTE, finish_rewrite=lambda old, new, file: new)
        monkeypatch.setitem(LANGUAGES, ".note", writable)
        (tmp_path / "list.note").write_text("; Milk.\nitem milk\n")
        settings = '{"approved": [], "avoid": {}}\n'
        (tmp_path / ".code-comments.json").write_text(settings)
        answers = [
            make_answer(1, "; Milk.\n", "; Oat milk.\n", file="list.note")
        ]

        code, reply = apply_answers(tmp_path, answers, flags=flags)

        assert code == 1
        assert reply["problems"] == [
            "item 1: list.note: owner reviews are off for this language, so "
            "the checker writes no answer in it"
        ]
        assert (tmp_path / "list.note").read_text() == "; Milk.\nitem milk\n"
        assert (tmp_path / ".code-comments.json").read_text() == settings

    @pytest.mark.parametrize(
        ("edits", "problem"),
        [
            pytest.param(
                {"total": '    """Sum in cents."""\n'},
                "",
                id="code-warning-kept",
            ),
            pytest.param(
                {"total": '    """Sum \\d prices."""\n'},
                "shop.py: the write adds Python's warning \"invalid escape "
                "sequence '\\d'\"",
                id="invalid-escape",
            ),
            pytest.param(
                {"total": '    """Sum C:\\\\a or C:\\new."""\n'},
                "shop.py: the docstring of total would hold \\n",
                id="escape",
            ),
            pytest.param(
                {"total": '    """Sum \\N{EM DASH} in cents."""\n'},
                "the docstring of total would hold \\N{EM DASH}",
                id="named-escape",
            ),
            pytest.param(
                {"total": '    """Sum \\\n    prices."""\n'},
                "the docstring of total would hold a backslash at a line end",
                id="line-end-backslash",
            ),
            pytest.param(
                {
                    "fee": '    """Fee."""\n',
                    "total": '    """Sum at C:\\new."""\n',
                },
                "the docstring of total would hold \\n",
                id="swap-escape",
            ),
            pytest.param(
                {
                    "f@if": '        """F."""\n',
                    "f@else": '        """F at C:\\new."""\n',
                },
                "the docstring of f would hold \\n",
                id="swap-escape-same-name",
            ),
            pytest.param({"tax": ""}, "only statement", id="cut-whole-body"),
            pytest.param(
                {"f@else": ""}, "only statement", id="cut-shared-name-body"
            ),
            pytest.param(
                {"total": '    """Sum."""; return 1\n'},
                f"item 1: shop.py total: {NEW_TEXT}",
                id="code-after-docstring",
            ),
            pytest.param(
                {"<module>": '"""Shop."""; X = 2\n'},
                f"item 1: shop.py <module>: {NEW_TEXT}",
                id="code-after-module-docstring",
            ),
            pytest.param(
                {"total": '    """Sum """""\n'},
                f"item 1: shop.py total: {NEW_TEXT}",
                id="two-string-literals",
            ),
            pytest.param(
                {"#@top": "# nosec\n"},
                f"item 1: shop.py: {NEW_TEXT}",
                id="adds-directive",
            ),
            pytest.param(
                {"<module>": ""},
                "shop.py: the write changes the encoding Python reads it in",
                id="comment-becomes-coding-line",
            ),
            pytest.param(
                {"total": '"""Sum."""\n'},
                "shop.py: the new text would not compile",
                id="does-not-compile",
            ),
        ],
    )
    def test_python_write_keeps_code_meaning(
        self, tmp_path: Path, edits: dict, problem: str
    ) -> None:
        """Refuse a write that would change what the code does.

        The owner types plain words, so ``C:\\new`` would become a line
        break. The same escape moved from another docstring is refused
        too. The checker also refuses code after a docstring, a new tool
        comment such as ``# nosec``, a comment that becomes the encoding
        line and cutting a function's only statement. A doubled
        backslash, or a warning the code already had, passes.

        Args:
            tmp_path: An empty folder the checker writes in.
            edits: The new text for each docstring or comment. A key
                with ``@`` marks one found by its text, not its name.
            problem: Part of the refusal the checker should give, or an
                empty string when the change is allowed.
        """
        docstrings = {
            "<module>": '"""Shop."""\n',
            "#@top": "# Prices follow the decoding: latin-1 table.\n",
            "total": '    """Sum the prices."""\n',
            "fee": '    """Fee at C:\\new."""\n',
            "tax": '    """Return the tax."""\n',
            "f@if": '        """F at C:\\new."""\n',
            "f@else": '        """F."""\n',
        }
        (tmp_path / "shop.py").write_text(
            f"{docstrings['<module>']}\n{docstrings['#@top']}import re\n\n"
            'DIGITS = re.compile("\\d+")\n\n\n'
            f"def total():\n{docstrings['total']}    return 0\n\n\n"
            f"def fee():\n{docstrings['fee']}    return 0\n\n\n"
            f"def tax():\n{docstrings['tax']}\n\n"
            f"if DIGITS:\n\n    def f():\n{docstrings['f@if']}\n"
            f"else:\n\n    def f():\n{docstrings['f@else']}"
        )
        answers = [
            make_answer(
                item,
                docstrings[key],
                text,
                file="shop.py",
                name="" if "@" in key else key,
            )
            for item, (key, text) in enumerate(edits.items(), 1)
        ]

        code, reply = apply_answers(tmp_path, answers)

        assert code == (1 if problem else 0)
        assert len(reply["problems"]) == (1 if problem else 0)
        assert all(problem in p for p in reply["problems"])

    @pytest.mark.parametrize(
        ("interface", "fields", "error"),
        [
            pytest.param(2, {}, "apply reads", id="other-interface"),
            pytest.param(1, {"file": "./cart.go"}, ANSWER, id="dotted-path"),
            pytest.param(1, {"file": "../cart.go"}, ANSWER, id="outside-repo"),
            pytest.param(1, {"file": "/cart.go"}, ANSWER, id="absolute-path"),
            pytest.param(1, {"cut": True}, ANSWER, id="text-and-cut"),
            pytest.param(1, {"current": ""}, ANSWER, id="empty-current"),
            pytest.param(1, {**REMOVED, "name": None}, ANSWER, id="no-name"),
            pytest.param(1, {**REMOVED, "old": ""}, ANSWER, id="empty-old"),
            pytest.param(
                1, {**REMOVED, "cut": None, "text": "x"}, ANSWER, id="no-cut"
            ),
        ],
    )
    def test_bad_request_exits_two(
        self, tmp_path: Path, capsys, interface: int, fields: dict, error: str
    ) -> None:
        """Exit 2 with the reason on stderr for a request apply cannot take.

        ``./cart.go`` and ``cart.go`` would be written as two files, one
        losing the other's answers, a file outside the repository is not
        the review's to write, and an answer with both a text and a cut
        has no single meaning. Removed code needs a name, an old and a cut.
        """
        answer = make_answer(1, TOTAL_TEXT, **fields)
        answer = {key: v for key, v in answer.items() if v is not None}
        request = {"interface": interface, "answers": [answer]}
        with pytest.MonkeyPatch.context() as patch:
            patch.setattr("sys.stdin", io.StringIO(json.dumps(request)))
            code, reply = run_command(
                "apply", "--check", "--interface", "1", "--repo", str(tmp_path)
            )

        assert (code, reply) == (2, {})
        assert capsys.readouterr().err.startswith(error)


STOP_SIGNALS = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
SUMS_EDIT = make_answer(1, TOTAL_TEXT, "// Sums.\n", name="Total")
SUMS_GO = CART_GO.replace(TOTAL_TEXT, "// Sums.\n")
SUMS_APPROVED = {
    "approved": [
        {
            "file": "cart.go",
            "name": "Reset",
            "text": "",
        },
        {
            "file": "cart.go",
            "name": "Valid",
            "text": VALID_TEXT,
        },
        {
            "file": "cart.go",
            "name": "Total",
            "text": "// Sums.\n",
        },
        {
            "file": "tax.go",
            "name": "Check",
            "text": "// Checks the tax.\n",
        },
    ],
    "avoid": {},
}


def write_cart_and_tax(repo: Path) -> list[dict]:
    """Write two sample Go files and their approvals, and edit both.

    The files are cart.go and tax.go. The write saves them in order,
    then the approvals file, so a test can stop it at any save.

    Args:
        repo: The folder to write the files into.

    Returns:
        A new comment for each of the two files.
    """
    write_cart(repo)
    (repo / "tax.go").write_text(
        "package cart\n\n// Checks the tax.\nfunc Check() {}\n",
    )
    return [
        make_answer(1, TOTAL_TEXT, "// Sums.\n"),
        make_answer(
            2,
            "// Checks the tax.\n",
            "// Adds tax.\n",
            name="Check",
            file="tax.go",
        ),
    ]


def read_folder_as_bytes(repo: Path) -> dict[str, bytes]:
    """Return the bytes of every file in a folder, by file name.

    Comparing this before and after a write shows a changed file and a
    leftover file in one check.

    Args:
        repo: The folder to read.

    Returns:
        Each file name with the file's bytes.
    """
    return {path.name: path.read_bytes() for path in repo.iterdir()}


def read_stop_handlers() -> dict:
    """Return the handler that SIGINT, SIGTERM and SIGHUP each have now.

    The checker swaps these handlers while it writes, so tests compare
    them before and after.

    Returns:
        Each of the three signals with its handler.
    """
    return {sig: signal.getsignal(sig) for sig in STOP_SIGNALS}


def read_blocked_signals() -> set[signal.Signals]:
    """Return the signals this thread blocks now.

    The checker blocks stop signals while it puts handlers back, so
    tests check that it leaves this set as it found it. Blocking an
    empty set is how Python reads the set without changing it.

    Returns:
        The blocked signals.
    """
    return signal.pthread_sigmask(signal.SIG_BLOCK, set())


def fail_file_save(monkeypatch: pytest.MonkeyPatch, number: int) -> None:
    """Make one file save fail with a disk error, leaving the file.

    The count starts at 1 and also counts the saves that put old files
    back after a failure.

    Args:
        monkeypatch: Undoes the change after the test.
        number: Which save fails.
    """
    save_file, count = os.replace, itertools.count(1)

    def save_or_fail(source: str, target: Path) -> None:
        """Save the file, unless this is the save that has to fail.

        Args:
            source: The new copy of the file.
            target: The file the copy replaces.

        Raises:
            OSError: This is the save that has to fail.
        """
        if next(count) == number:
            raise OSError("No space left on device")
        save_file(source, target)

    monkeypatch.setattr(os, "replace", save_or_fail)


def interrupt_after_file_save(
    monkeypatch: pytest.MonkeyPatch, number: int
) -> None:
    """Make one file save raise KeyboardInterrupt after saving the file.

    The file has already changed at that point, so the checker has to
    put it back as well.

    Args:
        monkeypatch: Undoes the change after the test.
        number: The save that raises, starting at 1.
    """
    save_file, count = os.replace, itertools.count(1)

    def save_then_interrupt(source: str, target: Path) -> None:
        """Save the file, then stop if this is the chosen save.

        Args:
            source: The new copy of the file.
            target: The file the copy replaces.

        Raises:
            KeyboardInterrupt: This is the chosen save.
        """
        save_file(source, target)
        if next(count) == number:
            raise KeyboardInterrupt

    monkeypatch.setattr(os, "replace", save_then_interrupt)


def send_signal_before_save(
    monkeypatch: pytest.MonkeyPatch, number: int, sig: signal.Signals
) -> list[signal.Signals]:
    """Send a real signal to this process just before one file is saved.

    The count starts at 1 and also counts the saves that put old files
    back after a failure.

    Args:
        monkeypatch: Undoes the change after the test.
        number: The save that the signal comes before.
        sig: The signal to send.

    Returns:
        A list that gets the signal when it is sent.
    """
    save_file, count, sent = os.replace, itertools.count(1), []

    def send_then_save(source: str, target: Path) -> None:
        """Send the signal at the chosen save, then save the file.

        Args:
            source: The new copy of the file.
            target: The file the copy replaces.
        """
        if next(count) == number:
            sent.append(sig)
            os.kill(os.getpid(), sig)
        save_file(source, target)

    monkeypatch.setattr(os, "replace", send_then_save)
    return sent


def send_signal_on_first_file_delete(
    monkeypatch: pytest.MonkeyPatch, sig: signal.Signals
) -> list[signal.Signals]:
    """Send a real signal the first time the checker deletes a file.

    In a write with no errors, the checker deletes files only as it
    tidies up after the last save.

    Args:
        monkeypatch: Undoes the change after the test.
        sig: The signal to send.

    Returns:
        A list that gets the signal when it is sent.
    """
    delete_file, sent = Path.unlink, []

    def send_then_delete(path: Path, missing_ok: bool = False) -> None:
        """Send the signal the first time, then delete the file.

        Args:
            path: The file to delete.
            missing_ok: Whether a missing file is fine.
        """
        if not sent:
            sent.append(sig)
            os.kill(os.getpid(), sig)
        delete_file(path, missing_ok)

    monkeypatch.setattr(Path, "unlink", send_then_delete)
    return sent


def fail_file_delete(monkeypatch: pytest.MonkeyPatch) -> None:
    """Make every file delete fail with a disk error.

    In a write with no errors, the checker deletes files only as it
    tidies up after the last save. The new comments are in place by
    then.

    Args:
        monkeypatch: Undoes the change after the test.
    """
    monkeypatch.setattr(
        Path,
        "unlink",
        Mock(side_effect=OSError("cannot delete the file")),
    )


@pytest.fixture
def program_handler() -> Iterator[Mock]:
    """Set one stand-in handler on each stop signal, as a program would.

    It only notes each call, so a test can see a signal that reached the
    program too early. pytest's own handlers come back afterwards.

    Yields:
        Mock: The handler set on SIGINT, SIGTERM and SIGHUP.
    """
    # Keep the real signal.signal, since tests replace it.
    set_handler, handlers = signal.signal, read_stop_handlers()
    blocked = read_blocked_signals()
    handler = Mock()
    for sig in STOP_SIGNALS:
        set_handler(sig, handler)
    yield handler
    for sig, old in handlers.items():
        set_handler(sig, old)
    signal.pthread_sigmask(signal.SIG_SETMASK, blocked)


def fail_handler_setup(
    monkeypatch: pytest.MonkeyPatch, sig: signal.Signals
) -> None:
    """Make the checker's first change to one signal's handler fail.

    The checker sets its own handlers before it saves anything, so this
    stops it partway through that setup.

    Args:
        monkeypatch: Undoes the change after the test.
        sig: The signal whose handler cannot be set.
    """
    set_handler, failed = signal.signal, []

    def set_or_fail(signum: int, handler: object) -> object:
        """Set the handler, unless this is the first try for the signal.

        Args:
            signum: The signal's number.
            handler: The new handler.

        Returns:
            The handler the signal had before.

        Raises:
            OSError: This is the first try for the signal.
        """
        if signum == sig and not failed:
            failed.append(signum)
            raise OSError("cannot set the handler")
        return set_handler(signum, handler)

    monkeypatch.setattr(signal, "signal", set_or_fail)


def send_signal_when_first_handler_is_back(
    monkeypatch: pytest.MonkeyPatch, handler: Mock
) -> list[int]:
    """Send a signal just after the checker puts back the first handler.

    That signal goes to the program's handler again, while the other two
    still go to the checker's.

    Args:
        monkeypatch: Undoes the change after the test.
        handler: The program's handler, which the checker puts back.

    Returns:
        A list that gets the signal when it is sent.
    """
    set_handler, sent = signal.signal, []

    def set_then_signal(signum: int, new: object) -> object:
        """Set the handler, then send the signal if it is the program's.

        Args:
            signum: The signal's number.
            new: The new handler.

        Returns:
            The handler the signal had before.
        """
        old = set_handler(signum, new)
        if new is handler and not sent:
            sent.append(signum)
            os.kill(os.getpid(), signum)
        return old

    monkeypatch.setattr(signal, "signal", set_then_signal)
    return sent


class TestApply:
    """Write the owner's answers and approve them, all or nothing.

    The owner sees nothing between answering and the write, so a wrong
    byte or a missing approval would go unnoticed until a later review.
    """

    def test_answers_are_written_and_approved(self, tmp_path: Path) -> None:
        """Write each answer, let gofmt lay it out, and approve the result.

        An unanswered comment, such as Fill's, must stay unapproved, a
        rename must move its approval, and removed code is recorded cut.
        """
        # gofmt indents a doc comment's example line with a tab.
        sum_example = "// Sums.\n//\n//  Total(prices)\n"
        settings = {
            "approved": [
                {"file": "cart.go", "name": "Check", "text": VALID_TEXT},
                {"file": "cart.go", "name": "Clear", "text": "// Empties.\n"},
                {"file": "tax.go", "name": "Check", "text": "// Taxes.\n"},
            ],
            "avoid": {"client": "say customer"},
        }
        (tmp_path / "cart.go").write_text(CART_GO)
        (tmp_path / "cart.go").chmod(0o640)
        (tmp_path / ".code-comments.json").write_text(json.dumps(settings))
        answers = [
            make_answer(
                1, "\t// Prices are in cents, so sums never round.\n", ""
            ),
            make_answer(3, VALID_TEXT),
            {
                "id": 4,
                "file": "cart.go",
                "name": "Clear",
                "old": "// Empties.\n",
                "code_removed": True,
                "cut": True,
            },
            make_answer(2, TOTAL_TEXT, sum_example, name="Total"),
        ]

        code, reply = apply_answers(
            tmp_path, answers, ("Check=Valid",), flags=()
        )

        assert (code, reply["changed"]) == (0, ["cart.go"])
        assert (tmp_path / "cart.go").read_text() == CART_GO.replace(
            "\tMax = 50\n\t// Prices are in cents, so sums never round.\n",
            "\tMax      = 50\n",
        ).replace(TOTAL_TEXT, sum_example.replace("  ", "\t"))
        assert (tmp_path / "cart.go").stat().st_mode & 0o777 == 0o640
        written = json.loads((tmp_path / ".code-comments.json").read_text())
        assert [(e["name"], e["text"]) for e in written["approved"]] == [
            ("Clear", ""),
            ("Currency", ""),
            ("Valid", VALID_TEXT),
            ("Total", sum_example.replace("  ", "\t")),
            ("Check", "// Taxes.\n"),
        ]
        assert written["avoid"] == {"client": "say customer"}

    @pytest.mark.parametrize("save", [2, 3], ids=["tax", "approvals"])
    def test_disk_error_puts_every_file_back(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch, save: int
    ) -> None:
        """Put every file back when a disk error stops the write.

        A half-done write would leave new comments in the code that
        nobody approved.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Makes one file save fail.
            save: Which save fails. The write saves cart.go, then
                tax.go, then the approvals file.
        """
        edits = write_cart_and_tax(tmp_path)
        before = read_folder_as_bytes(tmp_path)
        handlers = read_stop_handlers()
        fail_file_save(monkeypatch, save)

        code, _ = apply_answers(tmp_path, edits, flags=())

        assert code == 2
        assert read_folder_as_bytes(tmp_path) == before
        assert read_stop_handlers() == handlers

    @pytest.mark.parametrize("save", [1, 3], ids=["cart", "approvals"])
    def test_file_saved_before_error_goes_back(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch, save: int
    ) -> None:
        """Put a file back when an error comes just after it is saved.

        The file has already changed then, so undoing only the files
        saved before it would leave it new. KeyboardInterrupt gets past
        code that catches only ordinary errors.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Raises the error after one file save.
            save: The save after which the error comes.
        """
        edits = write_cart_and_tax(tmp_path)
        before = read_folder_as_bytes(tmp_path)
        handlers = read_stop_handlers()
        interrupt_after_file_save(monkeypatch, save)

        with pytest.raises(KeyboardInterrupt):
            apply_answers(tmp_path, edits, flags=())

        assert read_folder_as_bytes(tmp_path) == before
        assert read_stop_handlers() == handlers

    @pytest.mark.parametrize("save", [2, 3], ids=["tax", "approvals"])
    @pytest.mark.parametrize("sig", STOP_SIGNALS, ids=lambda s: s.name)
    def test_stop_signal_puts_every_file_back(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        sig: signal.Signals,
        save: int,
    ) -> None:
        """Put every file back when a signal comes before the last save.

        Ctrl-C, a closed terminal or a request to stop would otherwise
        end the checker with some files new and some old. The checker
        still stops after putting the files back.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Sends the signal before one file save.
            sig: The signal to send.
            save: The save that the signal comes before.
        """
        edits = write_cart_and_tax(tmp_path)
        before = read_folder_as_bytes(tmp_path)
        handlers = read_stop_handlers()
        sent = send_signal_before_save(monkeypatch, save, sig)

        with pytest.raises(KeyboardInterrupt):
            apply_answers(tmp_path, edits, flags=())

        assert sent == [sig]
        assert read_folder_as_bytes(tmp_path) == before
        assert read_stop_handlers() == handlers

    def test_signal_cannot_stop_putting_files_back(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Finish putting files back even when a signal comes.

        Stopping there would leave some files new and some old after a
        write that already failed.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Fails a file save and sends the signal.
        """
        edits = write_cart_and_tax(tmp_path)
        before = read_folder_as_bytes(tmp_path)
        handlers = read_stop_handlers()
        fail_file_save(monkeypatch, 2)
        # Save 3 is the first one that puts an old file back.
        sent = send_signal_before_save(monkeypatch, 3, signal.SIGTERM)

        code, _ = apply_answers(tmp_path, edits, flags=())

        assert code == 2
        assert sent == [signal.SIGTERM]
        assert read_folder_as_bytes(tmp_path) == before
        assert read_stop_handlers() == handlers

    def test_signal_after_last_save_keeps_write(
        self, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Report success when a signal comes after the last save.

        The new comments and approvals are in place by then. A failure
        would say that a finished write did not happen.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Sends the signal after the last save.
        """
        write_cart(tmp_path)
        handlers = read_stop_handlers()
        sent = send_signal_on_first_file_delete(
            monkeypatch,
            signal.SIGTERM,
        )

        code, reply = apply_answers(tmp_path, [SUMS_EDIT], flags=())

        assert (code, reply["changed"]) == (0, ["cart.go"])
        assert sent == [signal.SIGTERM]
        assert sorted(os.listdir(tmp_path)) == [SETTINGS, "cart.go"]
        assert (tmp_path / "cart.go").read_text() == SUMS_GO
        assert json.loads((tmp_path / SETTINGS).read_text()) == SUMS_APPROVED
        assert read_stop_handlers() == handlers

    def test_signal_while_restoring_handlers_is_ignored(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        program_handler: Mock,
    ) -> None:
        """Ignore a signal while the checker puts handlers back.

        The checker puts back the program's handlers one at a time. A
        signal that reached the program in between could stop it with
        some of the checker's handlers still in place. The checker
        blocks the signal instead, and drops it once every handler is
        back.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Sends a signal once the first handler is back.
            program_handler: The program's handler on every stop signal.
        """
        write_cart(tmp_path)
        blocked = read_blocked_signals()
        sent = send_signal_when_first_handler_is_back(
            monkeypatch,
            program_handler,
        )

        code, _ = apply_answers(tmp_path, [SUMS_EDIT], flags=())

        assert code == 0
        assert len(sent) == 1
        program_handler.assert_not_called()
        assert read_stop_handlers() == dict.fromkeys(
            STOP_SIGNALS,
            program_handler,
        )
        assert read_blocked_signals() == blocked
        assert sorted(os.listdir(tmp_path)) == [SETTINGS, "cart.go"]
        assert (tmp_path / "cart.go").read_text() == SUMS_GO
        assert json.loads((tmp_path / SETTINGS).read_text()) == SUMS_APPROVED

    def test_signal_ignored_when_handler_setup_fails(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        program_handler: Mock,
    ) -> None:
        """Ignore a signal while the checker undoes a failed setup.

        The checker has already changed some handlers when it fails to
        set another. It puts them all back with the signal blocked, as
        it does after a write that works.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Fails the SIGHUP setup and sends the signal.
            program_handler: The program's handler on every stop signal.
        """
        write_cart(tmp_path)
        before = read_folder_as_bytes(tmp_path)
        blocked = read_blocked_signals()
        fail_handler_setup(monkeypatch, signal.SIGHUP)
        sent = send_signal_when_first_handler_is_back(
            monkeypatch,
            program_handler,
        )

        code, _ = apply_answers(tmp_path, [SUMS_EDIT], flags=())

        assert code == 2
        assert len(sent) == 1
        program_handler.assert_not_called()
        assert read_stop_handlers() == dict.fromkeys(
            STOP_SIGNALS,
            program_handler,
        )
        assert read_blocked_signals() == blocked
        assert read_folder_as_bytes(tmp_path) == before

    def test_signal_ignored_when_cleanup_fails(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        program_handler: Mock,
    ) -> None:
        """Ignore a signal that comes after the tidy-up fails.

        The files are already saved when the tidy-up fails. The checker
        still puts every handler back with the signal blocked.

        Args:
            tmp_path: The repository folder.
            monkeypatch: Fails the tidy-up and sends the signal.
            program_handler: The program's handler on every stop signal.
        """
        write_cart(tmp_path)
        blocked = read_blocked_signals()
        fail_file_delete(monkeypatch)
        sent = send_signal_when_first_handler_is_back(
            monkeypatch,
            program_handler,
        )

        code, _ = apply_answers(tmp_path, [SUMS_EDIT], flags=())

        assert code == 2
        assert len(sent) == 1
        program_handler.assert_not_called()
        assert read_stop_handlers() == dict.fromkeys(
            STOP_SIGNALS,
            program_handler,
        )
        assert read_blocked_signals() == blocked
        assert sorted(os.listdir(tmp_path)) == [SETTINGS, "cart.go"]
        assert (tmp_path / "cart.go").read_text() == SUMS_GO
        assert json.loads((tmp_path / SETTINGS).read_text()) == SUMS_APPROVED

    def test_unnamed_answers_leave_approvals_alone(
        self, tmp_path: Path
    ) -> None:
        """Leave ``.code-comments.json`` as it was when nothing is approved.

        The owner edits its avoid list by hand, so rewriting the file for
        an answer that approves nothing would reformat their edits.
        """
        settings = '{"approved": [], "avoid": {"client": "say customer"}}'
        (tmp_path / "cart.go").write_text(CART_GO)
        (tmp_path / ".code-comments.json").write_text(settings)
        answers = [make_answer(1, "\t// Adds one item.\n", "")]

        code, reply = apply_answers(tmp_path, answers, flags=())

        assert (code, reply["changed"]) == (0, ["cart.go"])
        assert (tmp_path / ".code-comments.json").read_text() == settings

    @pytest.mark.parametrize(
        ("edited", "problem"),
        [
            pytest.param("cart.go", "cart.go: item 1 changed", id="source"),
            pytest.param(SETTINGS, SETTINGS + " changed", id="approvals"),
        ],
    )
    def test_edit_after_check_is_kept(
        self, tmp_path: Path, monkeypatch, edited: str, problem: str
    ) -> None:
        """Refuse the write when a file changed after the answers were checked.

        An agent or another review may write a file in that moment, and
        writing the checked source over it would silently drop its edit.
        """
        write_cart(tmp_path)
        answers = [make_answer(1, TOTAL_TEXT, "// Sums.\n", name="Total")]
        check = interface.check_answers

        def check_then_edit(*args):
            """Check the answers, then edit ``edited`` as another process."""
            checked = check(*args)
            with (tmp_path / edited).open("a") as out:
                out.write("\n")
            return checked

        monkeypatch.setattr(interface, "check_answers", check_then_edit)
        edit = (tmp_path / edited).read_text() + "\n"

        code, reply = apply_answers(tmp_path, answers, flags=())

        assert code == 1
        assert reply["problems"][0].startswith(problem)
        assert TOTAL_TEXT in (tmp_path / "cart.go").read_text()
        assert (tmp_path / edited).read_text() == edit

    def test_repository_comments_survive_rewrite(self, tmp_path: Path) -> None:
        """Write back each comment, then reword every second one.

        In every Go and Python file here, writing a comment's own text
        must change no byte, and a reword must change only that comment.
        A small sample would miss a bug that changes a real file outside
        the comment.

        Args:
            tmp_path: An empty folder that gets a copy of each file.
        """
        root = Path(__file__).resolve().parents[2]
        listed = ["git", "-C", str(root), "ls-files", "*.go", "*.py"]
        files = subprocess.check_output(listed, text=True).split()
        reworded = 0
        for file in files:
            copy = tmp_path / file
            copy.parent.mkdir(parents=True, exist_ok=True)
            copy.write_bytes((root / file).read_bytes())
            language = LANGUAGES[copy.suffix]
            before = language.read_comments(copy.read_text(), file)
            named = unname_unapprovable_comments(before)[0]
            answers = [
                make_answer(index, c.text, file=file, name=c.name)
                for index, c in enumerate(named)
                if language.is_writable(c)
                and copy.read_text().count(c.text) == 1
            ]

            kept = apply_answers(tmp_path, answers, flags=())
            unchanged = copy.read_bytes() == (root / file).read_bytes()
            rewords = {
                answer["id"]: language.lay_out_wording(
                    language.prepare_for_screen(answer["text"])[0].replace(
                        "\n", " One more sentence.\n", 1
                    ),
                    answer["text"],
                )
                for answer in answers[::2]
            }
            changed = apply_answers(
                tmp_path,
                [{**a, "text": rewords[a["id"]]} for a in answers[::2]],
                flags=(),
            )

            assert (kept[0], unchanged) == (0, True), (file, kept[1])
            assert changed[0] == 0, (file, changed[1])
            after = language.read_comments(copy.read_text(), file)
            assert [c.name for c in after] == [c.name for c in before], file
            unanswered = [i for i in range(len(before)) if i not in rewords]
            assert [after[i].text for i in unanswered] == [
                before[i].text for i in unanswered
            ], file
            assert all("One more sentence." in after[i].prose for i in rewords)
            reworded += len(rewords)

        assert reworded > 800
