"""Behavior tests for the commands pde-comment-review runs on the checker."""

import ast
import io
import json
import re
import subprocess
import textwrap
from contextlib import redirect_stdout
from pathlib import Path

import pytest

from comment_checker import INTERFACE, VERSION
from comment_checker.cli import main
from comment_checker.languages import LANGUAGES, Language
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

    def test_python_comments_wait_for_reviews(self, tmp_path: Path) -> None:
        """List no Python comment while Python's owner reviews are off.

        The review tool cannot write Python answers yet, so a Python item
        would make the whole review fail when the owner sends it.
        """
        base = commit_all(tmp_path)
        (tmp_path / "shop.py").write_text('"""Sells things."""\n')

        code, reply = list_changes(tmp_path, base)

        assert code == 0
        assert reply["items"] == []

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
        ("entry", "expected"),
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
                " items never gets\n    # rounded.\n    #\n"
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
        self, tmp_path: Path, entry: dict[str, str], expected: str
    ) -> None:
        """Write plain sentences with the old comment's indent, at 79 columns.

        A wrong indent would put the comment out of line with its code,
        a wrong width would leave lines past 79 columns, and plain text
        with no words must cut the comment rather than leave an empty one.
        """
        (tmp_path / "shop.go").write_text(SHOP_GO)
        (tmp_path / "shop.py").write_text(SHOP_PY)

        code, reply = show_entries(tmp_path, entry)

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

    @pytest.mark.parametrize(
        ("old", "plain", "expected"),
        [
            pytest.param(
                DOCSTRING,
                "Sum the prices.\n\nArgs:\nprices: The prices, in cents, of "
                "every item in the cart, which may be empty.\n\nIn cents.\n",
                '    r"""Sum the prices.\n\n    Args:\n        prices: The '
                "prices, in cents, of every item in the cart, which may be\n"
                '            empty.\n\n    In cents.\n    """\n',
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
                '    """Sum."""\n',
                "Sum the prices.\n\nReads the price list from the shared "
                "folder on the north drive at C:\\ prices daily.\n",
                '    """Sum the prices.\n\n    Reads the price list from the '
                "shared folder on the north drive at\n"
                '    C:\\ prices daily.\n    """\n',
                id="backslash-keeps-next-word",
            ),
            pytest.param(
                '    """Sum."""\n',
                "Sum the prices.\n\nPrices are listed in euros, and the name "
                "of the cafe is written caf\\N{LATIN SMALL LETTER E WITH "
                "ACUTE} here.\n",
                '    """Sum the prices.\n\n    Prices are listed in euros, '
                "and the name of the cafe is written\n"
                "    caf\\N{LATIN SMALL LETTER E WITH ACUTE} here.\n"
                '    """\n',
                id="named-escape-stays-whole",
            ),
            pytest.param('"""Sum."""\n', "\n", "", id="no-words-cuts"),
        ],
    )
    def test_plain_is_written_as_docstring(
        self, tmp_path: Path, old: str, plain: str, expected: str
    ) -> None:
        """Write plain text with the old docstring's indent, prefix and quotes.

        A lost ``r`` prefix changes what a backslash means, and a quote
        next to the closing quotes would end the string early.
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
        ],
    )
    def test_unwritable_plain_text_is_refused(
        self, tmp_path: Path, plain: str, problem: str
    ) -> None:
        """Refuse plain text that would break the file it is written into.

        Closing quotes would turn the rest of the text into code, Python
        reads a carriage return as a line end, which the checker refuses
        in a file, and a file holding a NUL does not compile.
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
