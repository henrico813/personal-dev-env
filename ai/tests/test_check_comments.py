"""Behavior tests for the comment checker in the code-documentation skill."""

import json
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "ai/skills/code-documentation/scripts/check_comments.py"


def init_repo_and_commit_all(root: Path) -> None:
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


def write_settings_file(
    repo: Path,
    approved: Sequence[tuple[str, str, str]] = (),
    avoid: dict[str, str] | None = None,
) -> None:
    """Write the repository's ``.code-comments.json``.

    Args:
        approved: ``(file, name, text)`` entries the owner approved.
        avoid: Words not to use, each with what to say instead.
    """
    settings = {
        "approved": [
            {"file": f, "name": n, "text": t} for f, n, t in approved
        ],
        "avoid": avoid or {},
    }
    (repo / ".code-comments.json").write_text(json.dumps(settings))


def run_checker(
    repo: Path, *args: str, paths: list[str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Run the checker in ``repo``.

    Args:
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

    def test_method_comment_takes_the_method_name(
        self, tmp_path: Path
    ) -> None:
        """Guard filler detection on Go methods.

        A method line starts with its receiver, "func (c *Cart)", so a
        naive parse names the comment "func". Filler only counts in a
        sentence that starts with "This" or the code's name, so with the
        wrong name "Total facilitates ..." would pass unnoticed. The
        same name drives ``restated-name``, which is only ever reported,
        so no exit code would show it going quiet.
        """
        (tmp_path / "cart.go").write_text(
            "// Total facilitates the prices.\nfunc (c *Cart) Total() int {\n"
        )

        result = run_checker(tmp_path)

        assert 'cart.go:1 Cart.Total: filler: "facilitates"\n' in (
            result.stdout
        )
        assert (
            "cart.go:1 Cart.Total: restated-name: first sentence repeats "
            'the name "Total"\n'
        ) in result.stdout

    def test_block_comment_takes_first_entry_name(
        self, tmp_path: Path
    ) -> None:
        """Guard filler detection above const and var blocks.

        The first entry of a Go block often has a comment or a blank
        line above it, and an earlier version read that line as the
        entry, so the block's comment got the name "const " or none.
        Filler only counts in a sentence that starts with "This" or the
        code's name, so "Red facilitates ..." above the block passed
        unnoticed.
        """
        (tmp_path / "colors.go").write_text(
            "// Red facilitates the sale colors.\nconst (\n"
            "\t// Red marks a sale price.\n\n\tRed = 1\n)\n"
        )

        result = run_checker(tmp_path)

        assert 'colors.go:1 const Red: filler: "facilitates"\n' in (
            result.stdout
        )

    def test_go_string_text_is_never_named(self, tmp_path: Path) -> None:
        """Keep the owner's approvals on real code in Go tests.

        Go tests hold shell scripts and Go files in raw strings. The
        checker read a script's "esac" and a fixture's "func Total" as
        code, so the owner approved comments under names such as
        "esac.rec" and "Total" that no code has. ``quote`` checks that a
        backtick in an ordinary string starts no raw string.
        """
        (tmp_path / "cart_test.go").write_text(
            "package cart\n\n"
            "// A cart file for the tests.\n"
            "const cartGo = `\n"
            "// Sums the prices.\n"
            "func Total() int { return 0 }\n"
            "`\n\n"
            "func TestTotal(t *testing.T) {\n"
            '\tquote := "`"\n'
            '\tfake(t, `case "$1" in\n'
            "esac\n"
            "`)\n"
            "\t// The fake has run.\n"
            "\tcheck(t, quote)\n"
            "}\n\n"
            "// Returns the cart's total.\n"
            "func cartTotal() int { return 0 }\n"
        )

        result = run_checker(tmp_path, "--approve")

        assert result.returncode == 0
        assert read_settings_file(tmp_path) == {
            "approved": [
                {
                    "file": "cart_test.go",
                    "name": "cartGo",
                    "text": "// A cart file for the tests.\n",
                },
                {
                    "file": "cart_test.go",
                    "name": "cartTotal",
                    "text": "// Returns the cart's total.\n",
                },
            ],
            "avoid": {},
        }

    def test_python_comments_take_their_code_names(
        self, tmp_path: Path
    ) -> None:
        """Approve Python comments under the names of their code.

        Approvals are keyed by name, and the Go reader once named text in
        test strings as code, so the owner approved names no code has.
        The string, form feed and directive here check the same mistake
        in Python.
        """
        (tmp_path / "till.py").write_text(
            '"""Count the till in cents."""\n\n'
            "# Cents per coin.\n"
            'RATES = {"dime": 10}\n'
            "# Coins the drawer takes.\nclass Coins: dime = 10\n"
            "FAKE = '''\ndef fake():\n"
            '    """A function in a string."""\n'
            "'''\n\n\n"
            "class Till:\n"
            "    # Cents in the drawer.\n"
            "    total: int = 0\n\n"
            "    # Opens the drawer.\n    @property\n"
            "    def open(self):\n"
            "        '''Start the day at zero.'''\n"
            "        # Nothing is counted yet.\n"
            "        count = 0\n"
            "\f\n"
            "# type: ignore\n"
            "def report():\n"
            '    """Print the total.\n\n    Once a day.\n    """\n\n'
            "    def line():\n"
            '        """Return one line of the report."""\n\n\n'
            'def short(): """Say hi."""\n'
        )

        result = run_checker(
            tmp_path,
            "--approve",
            paths=[str(tmp_path / "till.py")],
        )

        assert result.stdout.startswith(
            "till.py:35 short: cannot be approved; its docstring shares a "
            "line with other code\n"
        )
        texts = {
            "<module>": '"""Count the till in cents."""\n',
            "# RATES": "# Cents per coin.\n",
            "# Coins": "# Coins the drawer takes.\n",
            "# Till.total": "    # Cents in the drawer.\n",
            "# Till.open": "    # Opens the drawer.\n",
            "Till.open": "        '''Start the day at zero.'''\n",
            "report": '    """Print the total.\n\n    Once a day.\n    """\n',
            "report.line": '        """Return one line of the report."""\n',
        }
        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "till.py", "name": name, "text": text}
                for name, text in texts.items()
            ],
            "avoid": {},
        }


class TestFiller:
    """Fail on the skill's filler examples in sentences not at ``--base``."""

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
    def test_filler_fails_when_code_is_subject(
        self, tmp_path: Path, comment: str, fails: bool
    ) -> None:
        """Fail on filler that describes the code, not the caller's duties.

        The skill lists "is responsible for" as filler, but also asks
        comments to say who must close or free things. If "The caller
        is responsible for closing it" failed, agents would delete a
        fact readers need.
        """
        init_repo_and_commit_all(tmp_path)
        source = "package shop\n\n" + comment + "func Post() {}\n"
        (tmp_path / "post.go").write_text(source)

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == (1 if fails else 0)
        assert ("filler:" in result.stdout) == fails


class TestExistingSentences:
    """Leave sentences present at ``--base`` to the people who wrote them."""

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
        """Never fail an agent for filler it did not write.

        A sentence already present at --base may be a person's words,
        and the agent must not rewrite them to pass. Each case is one
        way an old sentence can look new to a simpler check: a sentence
        added to the same comment, --repo below the Git root, and a
        moved file. ``restated-name`` findings on old sentences are dropped,
        so the agent is asked only about its own text.
        """
        repo = tmp_path / folder
        repo.mkdir(exist_ok=True)
        old = (
            "package shop\n\n// Send facilitates the queue. It keeps order.\n"
        )
        (repo / before).write_text(old + "func Send() {}\n")
        init_repo_and_commit_all(tmp_path)
        (repo / before).unlink()
        (repo / after).write_text(old + added + "func Send() {}\n")

        result = run_checker(repo, "--base", "HEAD")

        assert result.returncode == 0
        assert 'filler: "facilitates" (existing: report only)' in (
            result.stdout
        )
        assert "restated-name" not in result.stdout

    def test_sentences_count_only_in_their_language(
        self, tmp_path: Path
    ) -> None:
        """Excuse an old sentence only in its own language.

        Filler already in a Python docstring is a person's words, but
        the same words new in a Go comment are the agent's and must
        fail. A broken Python file at ``--base`` must not stop the run.
        """
        said = "This facilitates the queue."
        (tmp_path / "queue.py").write_text(f'"""{said}"""\n')
        (tmp_path / "broken.py").write_text("def broken(:\n")
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "post.go").write_text(f"// {said}\nfunc Post() {{}}\n")

        result = run_checker(
            tmp_path,
            "--base",
            "HEAD",
            paths=[str(tmp_path / "post.go"), str(tmp_path / "queue.py")],
        )

        assert result.returncode == 1
        assert 'post.go:1 Post: filler: "facilitates"\n' in result.stdout
        assert (
            'queue.py:1 <module>: filler: "facilitates" (existing: report '
            "only)\n"
        ) in result.stdout


class TestHeaders:
    """Keep license headers and generated files out of the agent's way."""

    def test_license_header_filler_is_report_only(
        self, tmp_path: Path
    ) -> None:
        """Only report filler in a license header, even in a new file.

        A license header above "package" is text no agent should
        rewrite, so failing on it would push agents to edit legal text.
        """
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "send.go").write_text(
            "// Copyright Shop. This facilitates nothing.\n\npackage shop\n"
        )

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == 0
        assert "(header: report only)" in result.stdout

    def test_package_doc_filler_fails(self, tmp_path: Path) -> None:
        """Fail on new filler in a package doc comment.

        A package doc comment sits above "package" like a license
        header, but an agent writes it, so new filler there must fail.
        An earlier version mistook it for a header and let it pass.
        """
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "doc.go").write_text(
            "// Package shop seamlessly sells things.\npackage shop\n"
        )

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == 1
        assert 'doc.go:1 shop: filler: "seamlessly"\n' in result.stdout

    def test_generated_files_are_skipped(self, tmp_path: Path) -> None:
        """Skip generated files, even new ones.

        A generator writes these files and rewrites them on its next
        run, so an agent cannot fix their comments.
        """
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "post.go").write_text("package shop\n")
        (tmp_path / "gen.go").write_text(
            "// Code generated by stringer. DO NOT EDIT.\n\n"
            "package shop\n\n// Red facilitates the palette.\nconst Red = 1\n"
        )

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == 0
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
    def test_uncheckable_path_exits_two(
        self, tmp_path: Path, path: str
    ) -> None:
        """Exit 2 when a given path cannot be checked as a Go file.

        An agent that passes a folder, or a file that is not there,
        would otherwise be skipped like a non-Go file and exit 0, and
        the agent would believe its comments passed.
        """
        init_repo_and_commit_all(tmp_path)
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


TOTAL = "// Total is in cents, so a half-cent discount rounds down.\n"


class TestApprovedText:
    """Keep the owner's approved comments as the owner left them."""

    @pytest.mark.parametrize(
        "source, approved, listed, expected",
        [
            pytest.param(
                "// Total returns the cart total.\nfunc Total() {}\n",
                [("cart.go", "Total", TOTAL)],
                "cart.go",
                "cart.go:1 Total: approved: approved text changed",
                id="reworded",
            ),
            pytest.param(
                "// Empty reports whether the cart is empty.\n"
                "func Empty() {}\n",
                [("cart.go", "Empty", "")],
                "cart.go",
                "cart.go:1 Empty: approved: the owner cut this comment",
                id="cut-comment-restored",
            ),
            pytest.param(
                "func Total() {}\n",
                [("cart.go", "Total", TOTAL)],
                "cart.go",
                "cart.go Total: approved: approved comment is gone",
                id="deleted",
            ),
            pytest.param(
                "// Load the default colors.\nfunc init() {}\n\n"
                "// Load the saved colors.\nfunc init() {}\n",
                [("cart.go", "init", "// Load the default colors.\n")],
                "cart.go",
                "cart.go init: approved: approved comment is gone",
                id="name-shared",
            ),
            pytest.param(
                TOTAL + "func Total() {}\n",
                [("old.go", "Total", TOTAL)],
                "cart.go",
                "old.go Total: approved: approved file is gone",
                id="file-moved",
            ),
            pytest.param(
                "// Total returns the cart total.\nfunc Total() {}\n",
                [("cart.go", "Total", TOTAL)],
                "send.go",
                "cart.go:1 Total: approved: approved text changed",
                id="file-not-listed",
            ),
        ],
    )
    def test_changed_approved_text_fails(
        self,
        tmp_path: Path,
        source: str,
        approved: list[tuple[str, str, str]],
        listed: str,
        expected: str,
    ) -> None:
        """Fail when an agent rewords, restores or deletes the owner's text.

        After the owner reviewed a Go package's comments one by one,
        later agent rounds reworded them and put back comments the owner
        had cut. A failing exit keeps that work from passing as done.
        Two comments with one name must not share an entry, or the agent
        is told to copy one over the other. Moving the file, or leaving
        it off the command line as agents do with files they did not
        mean to change, must not take the text out of the check.
        """
        (tmp_path / "cart.go").write_text(source)
        (tmp_path / "send.go").write_text("package shop\n")
        write_settings_file(tmp_path, approved)

        result = run_checker(tmp_path, paths=[str(tmp_path / listed)])

        assert result.returncode == 1
        assert expected in result.stdout
        assert result.stdout.count(": approved: ") == 1

    @pytest.mark.parametrize(
        "source, expected",
        [
            pytest.param(
                'def total():\n    """Return the sum."""\n',
                "till.py:2 total: approved: approved text changed",
                id="reworded",
            ),
            pytest.param(
                "def total(:\n",
                "till.py -: approved: approved file cannot be read (Python "
                f"{sys.version_info.major}.{sys.version_info.minor}: "
                "SyntaxError at line 1); fix the file",
                id="cannot-parse",
            ),
        ],
    )
    def test_go_runs_check_approved_python_files(
        self, tmp_path: Path, source: str, expected: str
    ) -> None:
        """Fail a Go check when an approved Python file changes.

        Agents check the Go files they changed, and the checker adds
        every approved file, so a reworded docstring fails that run, as
        does a file Python cannot read, whose approvals are not gone.
        """
        (tmp_path / "post.go").write_text("package shop\n")
        (tmp_path / "till.py").write_text(source)
        doc = '    """Return the total in cents."""\n'
        write_settings_file(tmp_path, [("till.py", "total", doc)])

        result = run_checker(tmp_path)

        assert result.returncode == 1
        assert expected in result.stdout
        assert result.stdout.count(": approved: ") == 1

    def test_shared_names_are_reported_not_failed(
        self, tmp_path: Path
    ) -> None:
        """Keep ordinary Go with repeated names from blocking every run.

        Two ``var _`` interface checks in one file are ordinary Go and
        cannot be renamed, but approved text cannot tell their comments
        apart. Failing on them would block every run on such a file, so
        the checker only says they cannot be approved.
        """
        source = (
            "// Cart must be a Reader.\nvar _ io.Reader = (*Cart)(nil)\n\n"
            "// Till must be a Reader.\nvar _ io.Reader = (*Till)(nil)\n"
        )
        (tmp_path / "cart.go").write_text(source)

        result = run_checker(tmp_path)

        assert result.returncode == 0
        assert "cart.go:1 _: cannot be approved" in result.stdout
        assert "cart.go:4 _: cannot be approved" in result.stdout

    def test_approved_text_is_reported_not_failed(
        self, tmp_path: Path
    ) -> None:
        """Never make an agent rewrite text the owner approved.

        Only the owner changes their own words. The owner may approve a
        comment an agent wrote, so it is new since ``--base``; if the
        checker failed on its filler, the agent would have to rewrite
        the owner's approved text to pass.
        """
        init_repo_and_commit_all(tmp_path)
        comment = "// This is responsible for the cart total.\n"
        (tmp_path / "cart.go").write_text(comment + "func Total() {}\n")
        write_settings_file(tmp_path, [("cart.go", "Total", comment)])

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == 0
        assert (
            'Total: filler: "is responsible for" (approved: report only)'
            in result.stdout
        )


class TestSettingsFile:
    """Read the words to avoid and the owner's hand edits safely."""

    def test_avoided_word_fails_in_new_sentence(self, tmp_path: Path) -> None:
        """Make agents replace words the owner found confusing.

        The owner lists words that confused readers, such as one thing
        having two names, with the word to use instead. The checker
        decides these alone, so the agent must fix them in the sentences
        it wrote, and the message must say what to write.
        """
        init_repo_and_commit_all(tmp_path)
        source = "// Post hands the bill to the client.\nfunc Post() {}\n"
        (tmp_path / "post.go").write_text(source)
        write_settings_file(tmp_path, avoid={"client": "say customer"})

        result = run_checker(tmp_path, "--base", "HEAD")

        assert result.returncode == 1
        assert 'post.go:1 Post: avoided-word: "client": say customer\n' in (
            result.stdout
        )

    @pytest.mark.parametrize(
        "text",
        [
            pytest.param('{"avoid": ["client"]}', id="avoid-not-object"),
            pytest.param(
                '{"avoid": {"client": "say customer",}}', id="not-json"
            ),
        ],
    )
    def test_wrong_settings_shape_exits_two(
        self, tmp_path: Path, text: str
    ) -> None:
        """Exit 2 with a message when a hand edit breaks the file.

        The owner edits the avoid list by hand, since nothing writes it.
        Without the check, either edit crashes the checker with exit 1,
        which agents read as approved text to restore.
        """
        (tmp_path / "cart.go").write_text("package shop\n")
        (tmp_path / ".code-comments.json").write_text(text)

        result = run_checker(tmp_path)

        assert result.returncode == 2
        assert "Traceback" not in result.stderr


def read_settings_file(repo: Path) -> object:
    """Return the parsed ``.code-comments.json`` the checker wrote."""
    return json.loads((repo / ".code-comments.json").read_text())


class TestApprove:
    """Record the owner's approval exactly, without losing any."""

    @pytest.mark.parametrize(
        "approved, args",
        [
            pytest.param([], ["--base", "HEAD"], id="gone-since-base"),
            pytest.param([("cart.go", "Total", TOTAL)], [], id="gone-entry"),
        ],
    )
    def test_approve_writes_entries_the_check_accepts(
        self,
        tmp_path: Path,
        approved: list[tuple[str, str, str]],
        args: list[str],
    ) -> None:
        """Let the owner approve comments without the check then failing.

        Hand-typed entries lost tabs and final newlines, so the checker
        called the owner's own comment changed. A block constant is keyed
        by its own name, and a comment inside a function body is not
        keyed, since it moves with every edit there. A gone comment is
        recorded as cut, or an agent could add it back unnoticed.
        ``--approve`` writes the file itself because saving printed
        output with a shell redirect emptied it.
        """
        blue = '\t// Blue: "blue" is shown when none is set.\n'
        colors = f'package shop\n\nconst (\n{blue}\tBlue = "blue"\n)\n\n'
        pay = (
            "func Pay(\n\tc Cart,\n) {\n"
            "\t// There is nothing to pay for an empty cart.\n"
            "\tif c.Empty() {\n\t\treturn\n\t}\n}\n"
        )
        avoid = {"client": "say customer"}
        (tmp_path / "cart.go").write_text(colors + TOTAL + "func Total()\n")
        write_settings_file(tmp_path, approved, avoid)
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "cart.go").write_text(colors + pay)

        printed = run_checker(tmp_path, *args, "--approve")
        result = run_checker(tmp_path)

        assert printed.stdout == (
            ".code-comments.json: 1 added or changed, 0 kept, 1 newly cut\n"
        )
        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "cart.go", "name": "Blue", "text": blue},
                {"file": "cart.go", "name": "Total", "text": ""},
            ],
            "avoid": avoid,
        }
        assert result.returncode == 0

    def test_approve_keeps_other_files_approvals(self, tmp_path: Path) -> None:
        """Never lose approvals when the owner approves one package.

        The owner approves one package at a time, and each run rewrites
        the settings file. Dropping another file's entries, or a cut
        entry, would let agents rewrite that text or add the comment
        back. Approving only the files given matters as much: here
        ``b.go`` changes after its approval, and approving ``a.go`` must
        not approve that change too, nor, with ``--base``, record
        ``b.go``'s comments as cut. An old cut entry is kept, not newly
        cut, or the summary tells the owner something was cut in ``a.go``.
        """
        adds, sends = "// A adds the prices.\n", "// B sends the bill.\n"
        (tmp_path / "a.go").write_text(adds + "func A() {}\n")
        (tmp_path / "b.go").write_text(sends + "func B() {}\n")
        avoid = {"client": "say customer"}
        write_settings_file(tmp_path, [("b.go", "Old", "")], avoid)

        for file in ("a.go", "b.go"):
            run_checker(tmp_path, "--approve", paths=[str(tmp_path / file)])
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "b.go").write_text("// B mails it.\nfunc B() {}\n")
        printed = run_checker(
            tmp_path,
            "--base",
            "HEAD",
            "--approve",
            paths=[str(tmp_path / "a.go")],
        )

        assert printed.stdout == (
            ".code-comments.json: 0 added or changed, 3 kept, 0 newly cut\n"
        )
        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "a.go", "name": "A", "text": adds},
                {"file": "b.go", "name": "B", "text": sends},
                {"file": "b.go", "name": "Old", "text": ""},
            ],
            "avoid": avoid,
        }

    def test_approve_clears_deleted_files_approvals(
        self, tmp_path: Path
    ) -> None:
        """Let work finish once the owner approves a deleted file.

        The check fails while a file with approved text is gone, and the
        owner running ``--approve`` approves the deletion. Keeping the
        entries would leave every later task failing with no way to
        finish.
        """
        adds = "// A adds the prices.\n"
        (tmp_path / "a.go").write_text(adds + "func A() {}\n")
        gone = [("gone.go", "Total", TOTAL), ("gone.go", "Empty", "")]
        write_settings_file(tmp_path, gone)

        printed = run_checker(tmp_path, "--approve")
        result = run_checker(tmp_path)

        assert printed.stdout == (
            ".code-comments.json: 1 added or changed, 0 kept, 0 newly cut, "
            "2 removed with deleted files\n"
        )
        assert read_settings_file(tmp_path) == {
            "approved": [{"file": "a.go", "name": "A", "text": adds}],
            "avoid": {},
        }
        assert result.returncode == 0


class TestApproveComment:
    """Approve the one comment the owner answered, and nothing else."""

    @pytest.mark.parametrize(
        "path, total, earlier",
        [
            pytest.param(
                "cart.go",
                "// Total sums the prices.\n",
                [("cart.go", "Total", TOTAL)],
                id="reworded",
            ),
            pytest.param(
                "cart.go", "", [("cart.go", "Total", TOTAL)], id="cut"
            ),
            pytest.param("cart.go", TOTAL, [], id="first-approval"),
            pytest.param(
                "./cart.go",
                "// Total sums the prices.\n",
                [("cart.go", "Total", TOTAL)],
                id="dot-slash-path",
            ),
        ],
    )
    def test_approves_only_the_named_comment(
        self,
        tmp_path: Path,
        path: str,
        total: str,
        earlier: list[tuple[str, str, str]],
    ) -> None:
        """Never approve a comment the owner has not answered yet.

        On the first real comment review the owner answered 22 items.
        Approving each answer by whole file would approve the file's
        later items before the owner reached them. A code review found
        that ``./cart.go`` wrote an entry separate from ``cart.go``'s,
        which broke every later check.
        """
        clear = "// Clear empties the cart.\n"
        (tmp_path / "cart.go").write_text(
            total + "func Total() {}\n\n" + clear + "func Clear() {}\n"
        )
        avoid = {"client": "say customer"}
        write_settings_file(
            tmp_path, [("till.go", "Open", ""), *earlier], avoid
        )

        result = run_checker(
            tmp_path, "--approve-comment", path, "Total", paths=[]
        )

        assert result.returncode == 0
        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "cart.go", "name": "Total", "text": total},
                {"file": "till.go", "name": "Open", "text": ""},
            ],
            "avoid": avoid,
        }

    @pytest.mark.parametrize(
        "file, source, name, message",
        [
            pytest.param(
                "cart.go",
                "// Loads the prices.\nfunc init() {}\n\n"
                "// Loads the taxes.\nfunc init() {}\n",
                "init",
                "cart.go init: cannot be approved",
                id="shared-name",
            ),
            pytest.param(
                "notes.txt",
                "// Total sums the prices.\n",
                "Total",
                "needs a .go or .py file and a comment name",
                id="not-go",
            ),
            pytest.param(
                "cart.go",
                "func Pay() {\n\t// Nothing to pay.\n\treturn\n}\n",
                "",
                "needs a .go or .py file and a comment name",
                id="empty-name",
            ),
            pytest.param(
                "crt.go",
                None,
                "Total",
                "crt.go: no such file",
                id="missing-file",
            ),
        ],
    )
    def test_refuses_comment_it_cannot_approve(
        self,
        tmp_path: Path,
        file: str,
        source: str | None,
        name: str,
        message: str,
    ) -> None:
        """Refuse a comment the checker cannot pin down, and change nothing.

        A non-Go file, an empty name, a name two comments share and a
        missing file each approved or cut the wrong entry, or would
        have.
        """
        if source is not None:
            (tmp_path / file).write_text(source)
        write_settings_file(tmp_path)
        before = (tmp_path / ".code-comments.json").read_text()

        result = run_checker(
            tmp_path, "--approve-comment", file, name, paths=[]
        )

        assert result.returncode == 2
        assert message in result.stderr
        assert (tmp_path / ".code-comments.json").read_text() == before

    def test_answers_after_renames_keep_checks_passing(
        self, tmp_path: Path
    ) -> None:
        """Carry approvals through renames and moves when approving one answer.

        Answers are approved one at a time, so without this a renamed
        function or moved file keeps its approval under the old name and
        every later check fails.
        """
        (tmp_path / "new.go").write_text(
            TOTAL + "func Sum() {}\n\n" + READS + "func Load() {}\n"
        )
        write_settings_file(
            tmp_path,
            [
                ("old.go", "Total", TOTAL),
                ("old.go", "Load", READS),
            ],
        )

        run_checker(
            tmp_path,
            "--approve-comment",
            "new.go",
            "Sum",
            "--rename=old.go=new.go",
            "--rename=Total=Sum",
            paths=[],
        )
        result = run_checker(tmp_path)

        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "new.go", "name": "Load", "text": READS},
                {"file": "new.go", "name": "Sum", "text": TOTAL},
            ],
            "avoid": {},
        }
        assert result.returncode == 0

    def test_cuts_comment_of_deleted_file(self, tmp_path: Path) -> None:
        """Cut a deleted file's approved comment when its answer is approved.

        When code is removed, the owner can only drop its comment, so
        approving that answer must record a cut, or every later check
        fails on the gone comment.
        """
        (tmp_path / "till.go").write_text("package till\n")
        write_settings_file(
            tmp_path,
            [("gone.go", "Open", "// Open opens the till.\n")],
        )

        run_checker(
            tmp_path,
            "--approve-comment",
            "gone.go",
            "Open",
            paths=[],
        )
        result = run_checker(tmp_path)

        assert read_settings_file(tmp_path) == {
            "approved": [{"file": "gone.go", "name": "Open", "text": ""}],
            "avoid": {},
        }
        assert result.returncode == 0


READS = "// Load reads the saved cart.\n"


class TestRenames:
    """Keep the owner's approvals with code that a task renamed."""

    def test_rename_moves_entries_to_new_names(self, tmp_path: Path) -> None:
        """Keep the owner's review when a task renames code.

        Entries are keyed by file and name, so before --rename, --approve
        recorded renamed comments as cut, letting cut comments return
        under the new name.
        """
        (tmp_path / "cart.go").write_text(
            TOTAL + "func Total() {}\n\n" + READS + "func Load() {}\n"
        )
        (tmp_path / "till.go").write_text(TOTAL + "func Total() {}\n")
        (tmp_path / "other.go").write_text(TOTAL + "func Total() {}\n")
        write_settings_file(
            tmp_path,
            [
                ("cart.go", "Total", TOTAL),
                ("cart.go", "Empty", ""),
                ("till.go", "Total", TOTAL),
                ("other.go", "Total", TOTAL),
            ],
        )
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "cart.go").write_text(
            TOTAL + "func Sum() {}\n\n" + READS + "func Read() {}\n"
        )

        # till.go still has Total, and other.go is not given, so both keep
        # their Total entries. "Totl" is a typo that renames nothing.
        printed = run_checker(
            tmp_path,
            "--base",
            "HEAD",
            "--approve",
            "--rename=Total=Sum",
            "--rename=Load=Read",
            "--rename=Empty=IsEmpty",
            "--rename=Totl=Add",
            paths=[str(tmp_path / "cart.go"), str(tmp_path / "till.go")],
        )
        result = run_checker(tmp_path)

        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "cart.go", "name": "Sum", "text": TOTAL},
                {"file": "cart.go", "name": "Read", "text": READS},
                {"file": "cart.go", "name": "IsEmpty", "text": ""},
                {"file": "other.go", "name": "Total", "text": TOTAL},
                {"file": "till.go", "name": "Total", "text": TOTAL},
            ],
            "avoid": {},
        }
        assert printed.stderr == (
            "--rename Totl: matches no entry or --base comment in "
            "the checked files\n"
        )
        assert result.returncode == 0

    def test_file_move_keeps_its_entries(self, tmp_path: Path) -> None:
        """Keep a moved file's approvals, cut entries included.

        Entries are keyed by file and name, so before --rename, --approve
        dropped a moved file's entries as if the file were deleted, and
        the owner's cuts no longer held in the new file.
        """
        (tmp_path / "old.go").write_text(TOTAL + "func Total() {}\n")
        write_settings_file(
            tmp_path, [("old.go", "Total", TOTAL), ("old.go", "Empty", "")]
        )
        init_repo_and_commit_all(tmp_path)
        (tmp_path / "old.go").rename(tmp_path / "new.go")

        printed = run_checker(
            tmp_path,
            "--base",
            "HEAD",
            "--approve",
            "--rename=old.go=new.go",
            paths=[str(tmp_path / "new.go")],
        )
        result = run_checker(tmp_path)

        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "new.go", "name": "Total", "text": TOTAL},
                {"file": "new.go", "name": "Empty", "text": ""},
            ],
            "avoid": {},
        }
        assert printed.stderr == ""
        assert result.returncode == 0

    @pytest.mark.parametrize(
        "moved_file",
        [
            pytest.param("basket.go", id="sorts-before-new"),
            pytest.param("old.go", id="sorts-after-new"),
        ],
    )
    def test_existing_entry_wins_over_moved_one(
        self, tmp_path: Path, moved_file: str
    ) -> None:
        """Keep the approval of code that was already under the new name.

        A file can be moved onto a name that already has approved
        entries. The owner approved the comments that are there now, so
        moved text must not replace them, whichever file name sorts
        first.
        """
        (tmp_path / "new.go").write_text(TOTAL + "func Total() {}\n")
        (tmp_path / "a.go").write_text("// A adds the prices.\nfunc A() {}\n")
        moved = "// Total is in dollars.\n"
        write_settings_file(
            tmp_path,
            [(moved_file, "Total", moved), ("new.go", "Total", TOTAL)],
        )

        run_checker(
            tmp_path,
            "--approve",
            f"--rename={moved_file}=new.go",
            paths=[str(tmp_path / "a.go")],
        )
        result = run_checker(tmp_path)

        assert read_settings_file(tmp_path) == {
            "approved": [
                {
                    "file": "a.go",
                    "name": "A",
                    "text": "// A adds the prices.\n",
                },
                {"file": "new.go", "name": "Total", "text": TOTAL},
            ],
            "avoid": {},
        }
        assert result.returncode == 0

    def test_gone_comment_names_its_likely_rename(
        self, tmp_path: Path
    ) -> None:
        """Tell the agent where the owner's text went after a rename.

        A Go doc comment starts with its name, so after a rename the
        checker could only say the approved comment was gone. A wrong
        hint is worse than none: the agent would move the owner's text
        onto other code, or never record a real deletion.
        """
        total = "// total is in cents, so ParseCart rounds half-cents down.\n"
        till = "// till is in cents, so ParseTill rounds half-cents down.\n"
        source = (
            "// sum is in cents, so ParseOrder rounds half-cents down.\n"
            "func sum() {}\n\n"
            "// count is in cents, so len(items) rounds half-cents down.\n"
            "func count() {}\n\n"
            "// part is in cents, so ParsePart rounds\n"
            "func part() {}\n\n" + till + "func till() {}\n"
        )
        (tmp_path / "cart.go").write_text(source)
        approved = [("cart.go", "total", total), ("cart.go", "till", till)]
        write_settings_file(tmp_path, approved)

        result = run_checker(tmp_path)

        # Not hints: count says something else, part has only the start
        # of the sentence, and till is the owner's other approved comment.
        assert result.returncode == 1
        assert (
            "cart.go total: approved: approved comment is gone; possibly now "
            "sum at 1 (--approve --rename total=sum)" in result.stdout
        )
        assert "possibly now count" not in result.stdout
        assert "possibly now part" not in result.stdout
        assert "possibly now till" not in result.stdout

    def test_renamed_python_function_gets_hint(self, tmp_path: Path) -> None:
        """Hint the new name when a Python function is renamed.

        Without the hint, a rename looks like a cut comment and a new
        one. A docstring's quotes must not count as words, or the match
        fails.
        """
        doc = '    """Return the total in cents, rounded down."""\n'
        (tmp_path / "till.py").write_text("def sum():\n" + doc)
        write_settings_file(tmp_path, [("till.py", "total", doc)])

        result = run_checker(tmp_path, paths=[str(tmp_path / "till.py")])

        assert (
            "till.py total: approved: approved comment is gone; possibly now "
            "sum at 2 (--approve --rename total=sum)\n"
        ) in result.stdout

    def test_renames_move_python_entries(self, tmp_path: Path) -> None:
        """Move approvals with a moved Python file and a method named go.

        The review tool approves one answer at a time, so
        ``--approve-comment`` must read Python, and a moved file's
        approvals must not be left under the old path.
        """
        doc = '        """Start the job."""\n'
        (tmp_path / "new.py").write_text(
            "class Runner:\n    def run(self):\n" + doc
        )
        write_settings_file(
            tmp_path,
            [("old.py", "Runner.go", doc), ("old.py", "Runner.stop", "")],
        )

        run_checker(
            tmp_path,
            "--approve-comment",
            "new.py",
            "Runner.run",
            "--rename=old.py=new.py",
            "--rename=Runner.go=Runner.run",
            paths=[],
        )

        assert read_settings_file(tmp_path) == {
            "approved": [
                {"file": "new.py", "name": "Runner.run", "text": doc},
                {"file": "new.py", "name": "Runner.stop", "text": ""},
            ],
            "avoid": {},
        }


def commit_all(root: Path) -> None:
    """Commit every change in ``root``, as merging a pull request does."""
    git = ["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t"]
    subprocess.run([*git, "add", "-A"], check=True)
    subprocess.run([*git, "commit", "-qm", "merged"], check=True)


SUMS = "// Total sums the prices.\n"


class TestMergeApprovals:
    """Bring a removed worktree's approvals to the main checkout."""

    def test_merge_keeps_only_approvals_matching_code(
        self, tmp_path: Path
    ) -> None:
        """Keep only approvals that match the merged code.

        Older worktrees held older text for newer approvals, so copying a
        whole file back undid them. A review found a renamed comment's old
        name left in the main copy, failing the check in every new
        worktree.
        """
        main, worktree = tmp_path / "main", tmp_path / "worktree"
        main.mkdir()
        worktree.mkdir()
        opens = "// Open starts the day's sales.\n"
        old_name = ("cart.go", "Add", "// Add puts an item in the cart.\n")
        new_name = ("cart.go", "Put", "// Put puts an item in the cart.\n")
        not_in_code = ("cart.go", "Sum", "// Sum adds the prices.\n")
        (main / "cart.go").write_text(
            TOTAL + "func Total() {}\n\n" + old_name[2] + "func Add() {}\n"
        )
        (main / "till.go").write_text(opens + "func Open() {}\n")
        (main / "old.go").write_text(READS + "func Load() {}\n")
        init_repo_and_commit_all(main)
        (main / "cart.go").write_text(
            SUMS + "func Total() {}\n\n" + new_name[2] + "func Put() {}\n"
        )
        (main / "old.go").unlink()
        commit_all(main)
        write_settings_file(
            main,
            [
                old_name,
                ("cart.go", "Gone", ""),
                ("cart.go", "Total", TOTAL),
                ("old.go", "Load", READS),
                ("till.go", "Open", opens),
            ],
        )
        write_settings_file(
            worktree,
            [
                ("cart.go", "Clear", ""),
                new_name,
                not_in_code,
                ("cart.go", "Total", SUMS),
                ("till.go", "Open", "// Open opens the till.\n"),
            ],
        )

        merged = run_checker(
            main, "--merge-approvals", str(worktree), "HEAD", paths=[]
        )
        result = run_checker(main)

        assert merged.returncode == 0
        assert merged.stdout.endswith(
            "3 taken from " + str(worktree) + ", 1 skipped, 1 dropped, "
            "1 removed with deleted files\n"
        )
        assert read_settings_file(main) == {
            "approved": [
                {"file": "cart.go", "name": "Gone", "text": ""},
                {"file": "cart.go", "name": "Total", "text": SUMS},
                {"file": "cart.go", "name": "Clear", "text": ""},
                {"file": "cart.go", "name": "Put", "text": new_name[2]},
                {"file": "till.go", "name": "Open", "text": opens},
            ],
            "avoid": {},
        }
        assert result.returncode == 0

    @pytest.mark.parametrize(
        "folder, exit_code, message",
        [
            pytest.param(
                "worktree",
                1,
                "cart.go Total: approved differently",
                id="copies-disagree",
            ),
            pytest.param("missing", 2, "no such file", id="no-settings-file"),
        ],
    )
    def test_writes_nothing_when_merge_is_refused(
        self, tmp_path: Path, folder: str, exit_code: int, message: str
    ) -> None:
        """Leave the choice to the owner when no copy is the code.

        Taking either text could drop the owner's real decision, so the
        comment is named for the owner to approve. A review found that a
        wrong folder dropped entries and rewrote the main copy, so it is
        refused too.
        """
        main, worktree = tmp_path / "main", tmp_path / "worktree"
        main.mkdir()
        worktree.mkdir()
        (main / "cart.go").write_text(TOTAL + "func Total() {}\n")
        init_repo_and_commit_all(main)
        (main / "cart.go").write_text(SUMS + "func Total() {}\n")
        commit_all(main)
        adds = [("cart.go", "Total", "// Total adds the prices.\n")]
        write_settings_file(main, adds)
        write_settings_file(worktree, [("cart.go", "Total", TOTAL)])
        before = (main / ".code-comments.json").read_text()

        result = run_checker(
            main, "--merge-approvals", str(tmp_path / folder), "HEAD", paths=[]
        )

        assert result.returncode == exit_code
        assert message in result.stderr
        assert (main / ".code-comments.json").read_text() == before

    def test_merge_takes_python_approvals(self, tmp_path: Path) -> None:
        """Bring a worktree's Python approvals back at cleanup.

        Only Go files' approvals were taken, so removing a worktree
        would drop every Python docstring the owner approved in it. A
        changed file that does not parse, with no approvals, must not
        stop it.
        """
        worktree = tmp_path / "worktree"
        worktree.mkdir()
        init_repo_and_commit_all(tmp_path)
        doc = '"""Count the till."""\n'
        (tmp_path / "till.py").write_text(doc)
        (tmp_path / "tool.py").write_text("print 'Python 2'\n")
        commit_all(tmp_path)
        write_settings_file(worktree, [("till.py", "<module>", doc)])

        run_checker(
            tmp_path,
            "--merge-approvals",
            str(worktree),
            "HEAD",
            paths=[],
        )

        assert read_settings_file(tmp_path) == {
            "approved": [{"file": "till.py", "name": "<module>", "text": doc}],
            "avoid": {},
        }
