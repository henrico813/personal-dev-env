"""Behavior tests for the comment checker in the code-documentation skill."""

import json
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "ai/skills/code-documentation/scripts/check_comments.py"
TOTAL = "// Total is in cents, so a half-cent discount rounds down.\n"


def write_repo(
    repo: Path,
    files: dict[str, str],
    approved: Sequence[tuple[str, str, str]] = (),
    avoid: dict[str, str] | None = None,
) -> Path:
    """Write source files and their ``.code-comments.json``.

    Args:
        repo: Empty folder to use as the repository.
        files: Source text by path inside the repository.
        approved: ``(file, name, text)`` entries the owner approved.
        avoid: Words not to use, each with what to say instead.

    Returns:
        The repository folder.
    """
    for name, text in files.items():
        (repo / name).write_text(text)
    settings = {
        "approved": [
            {"file": f, "name": n, "text": t} for f, n, t in approved
        ],
        "avoid": avoid or {},
    }
    (repo / ".code-comments.json").write_text(json.dumps(settings))
    return repo


def run_checker(
    repo: Path, *args: str, only: str = ""
) -> subprocess.CompletedProcess[str]:
    """Run the checker on every Go file in the repository.

    Args:
        repo: Repository written by ``write_repo``.
        *args: Extra checker options, such as ``--base``.
        only: Check just this file instead.

    Returns:
        The finished process, with output as text.
    """
    paths = sorted(str(p) for p in repo.iterdir() if p.suffix == ".go")
    paths = [str(repo / only)] if only else paths
    return subprocess.run(
        [sys.executable, str(CHECKER), "--repo", str(repo), *args, *paths],
        capture_output=True,
        text=True,
    )


@pytest.mark.parametrize(
    "source, approved, expected",
    [
        pytest.param(
            "// Total returns the cart total.\n"
            "func Total() int { return 0 }\n",
            [("cart.go", "Total", TOTAL)],
            "Total: approved: approved text changed",
            id="reworded",
        ),
        pytest.param(
            "// Empty reports whether the cart is empty.\n"
            "func Empty() bool { return false }\n",
            [("cart.go", "Empty", "")],
            "Empty: approved: the owner cut this comment",
            id="cut-comment-restored",
        ),
        pytest.param(
            "func Total() int { return 0 }\n",
            [("cart.go", "Total", TOTAL)],
            "Total: approved: approved comment is gone",
            id="deleted",
        ),
        pytest.param(
            "// A prints its name.\n"
            'func (a A) String() string { return "" }\n'
            "\n"
            "// B prints its name.\n"
            'func (b *B) String() string { return "" }\n',
            [
                ("cart.go", "A.String", "// A prints its name.\n"),
                ("cart.go", "B.String", "// B prints its id.\n"),
            ],
            "B.String: approved: approved text changed",
            id="same-name-methods",
        ),
        pytest.param(
            "// Load the default colors.\n"
            "func init() {}\n"
            "\n"
            "// Load the saved colors.\n"
            "func init() {}\n",
            [("cart.go", "init", "// Load the default colors.\n")],
            "init: approved: approved comment is gone",
            id="approved-name-shared",
        ),
        pytest.param(
            TOTAL + "func Total() int { return 0 }\n",
            [("old.go", "Total", TOTAL)],
            "Total: approved: approved file is gone",
            id="file-moved",
        ),
    ],
)
def test_changed_approved_text_fails(
    tmp_path: Path,
    source: str,
    approved: list[tuple[str, str, str]],
    expected: str,
) -> None:
    """Fail when an agent rewords, restores or deletes the owner's text.

    After the owner reviewed a Go package's comments one by one, later
    agent rounds reworded their text and put back comments they had cut.
    A failing exit is what keeps that work from passing as done. Two
    comments must never share approved text, or the agent is told to
    copy one comment over the other. Moving the file must not take the
    text out of the check.
    """
    repo = write_repo(tmp_path, {"cart.go": source}, approved)

    result = run_checker(repo)

    assert result.returncode == 1
    assert expected in result.stdout
    assert result.stdout.count(": approved: ") == 1


def test_unlisted_approved_file_is_still_checked(tmp_path: Path) -> None:
    """Fail on changed approved text in a file the agent did not list.

    Agents pass the checker the files they think they changed. An edit
    to another file must not slip the owner's text past the check.
    """
    source = (
        "// Total returns the cart total.\nfunc Total() int { return 0 }\n"
    )
    files = {"cart.go": source, "send.go": "package shop\n"}
    repo = write_repo(tmp_path, files, [("cart.go", "Total", TOTAL)])

    result = run_checker(repo, only="send.go")

    assert result.returncode == 1
    assert "Total: approved: approved text changed" in result.stdout


def test_unnameable_comments_do_not_fail(tmp_path: Path) -> None:
    """Pass ordinary Go whose comments cannot be told apart by name.

    Two ``var _`` checks in one file are common and cannot be renamed,
    and comments inside a function have no name at all. Stopping on
    either would block every run on such a file. Shared names are
    reported as unable to be approved; comments in a function body are
    passed over without a message.
    """
    source = (
        "// Cart must be a Reader.\n"
        "var _ io.Reader = (*Cart)(nil)\n"
        "\n"
        "// Till must be a Reader.\n"
        "var _ io.Reader = (*Till)(nil)\n"
        "\n"
        "func Pay(\n"
        "\tc Cart,\n"
        ") {\n"
        "\t// if the cart is empty, there is nothing to pay.\n"
        "\tif c.Empty() {\n"
        "\t\treturn\n"
        "\t}\n"
        "\t// if the till is closed, wait for it.\n"
        "\tif closed {\n"
        "\t\twait()\n"
        "\t}\n"
        "}\n"
    )
    repo = write_repo(tmp_path, {"cart.go": source})

    result = run_checker(repo)

    assert result.returncode == 0
    assert result.stdout.count("cannot be approved") == 2
    assert "cart.go:1 _: cannot be approved" in result.stdout


def test_approved_text_is_reported_not_failed(tmp_path: Path) -> None:
    """Report problems in approved text without failing.

    Only the owner changes their own words. If the checker failed on
    them, an agent would have to rewrite the owner's text to pass.
    """
    comment = "// Total is responsible for the cart total.\n"
    source = comment + "func Total() int { return 0 }\n"
    repo = write_repo(
        tmp_path, {"cart.go": source}, [("cart.go", "Total", comment)]
    )

    result = run_checker(repo)

    assert result.returncode == 0
    assert (
        'Total: filler: "is responsible for" (approved: report only)'
        in result.stdout
    )


@pytest.mark.parametrize(
    "source, place, name",
    [
        pytest.param(
            "const (\n"
            "\t// Red: the color of a sale price.\n"
            '\tRed Color = "red"\n'
            ")\n",
            "cart.go:2 Red",
            "Red",
            id="constant-in-block",
        ),
        pytest.param(
            "// Total adds up the prices.\n"
            "func (c *Cart) Total() int { return 0 }\n",
            "cart.go:1 Cart.Total",
            "Total",
            id="method",
        ),
    ],
)
def test_flags_name_repeated_in_first_sentence(
    tmp_path: Path, source: str, place: str, name: str
) -> None:
    """Flag a first sentence that repeats the name it documents.

    Most of the comments the owner cut in their review began with the
    name below them. The flag depends on finding the right name, which
    is found differently for a constant in a block and a method. A
    flag is a question for the agent, so it does not fail.
    """
    repo = write_repo(tmp_path, {"cart.go": source})

    result = run_checker(repo)

    assert result.returncode == 0
    assert (
        f'{place}: restated-name: first sentence repeats the name "{name}"'
        in result.stdout
    )


def test_flags_fact_repeated_in_another_comment(tmp_path: Path) -> None:
    """Flag a fact that two comments state, and say where the other is.

    In the owner's review, six of the cuts removed a fact that another
    comment already stated. The copies sat on different declarations,
    so the agents that wrote them did not notice.
    """
    source = (
        "const (\n"
        "\t// Draft: only the editor creates it; the store never saves it.\n"
        '\tDraft = "draft"\n'
        ")\n"
        "\n"
        "// The store never saves a cart the editor creates as a draft.\n"
        "func Save(c Cart) error { return nil }\n"
    )
    repo = write_repo(tmp_path, {"cart.go": source})

    result = run_checker(repo)

    assert result.returncode == 0
    assert (
        "cart.go:2 Draft: repeated-fact: repeats cart.go:6:" in result.stdout
    )


def commit_all(root: Path) -> list[str]:
    """Make ``root`` a Git repository and commit everything in it.

    Args:
        root: Folder to commit.

    Returns:
        The ``git`` command prefix for further commands in ``root``.
    """
    git = ["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t"]
    subprocess.run([*git, "init", "-q"], check=True)
    subprocess.run([*git, "add", "."], check=True)
    subprocess.run(
        [*git, "commit", "-qm", "base", "--allow-empty"], check=True
    )
    return git


@pytest.mark.parametrize(
    "old, new, avoid, rule, message",
    [
        pytest.param(
            "// Send passes the order to the client.\n",
            "// Post hands the bill to the client.\n",
            {"client": "say customer"},
            "avoided-word",
            '"client": say customer',
            id="avoided-word",
        ),
        pytest.param(
            "// Send facilitates the queue.\n",
            "// Post facilitates the outbox.\n",
            {},
            "filler",
            '"facilitates"',
            id="filler",
        ),
    ],
)
def test_style_problems_fail_only_new_sentences(
    tmp_path: Path,
    old: str,
    new: str,
    avoid: dict[str, str],
    rule: str,
    message: str,
) -> None:
    """Fail on an avoided word or filler only in the task's sentences.

    The checker decides these alone, so an agent must fix them in what
    it wrote. A sentence that was there before the task may be a
    person's words that nobody has approved yet. Adding a sentence to
    that comment must not make the person's sentence the agent's to
    rewrite, so it is only reported.
    """
    package = "package shop\n\n"
    repo = write_repo(
        tmp_path, {"old.go": package + old + "func Send() {}\n"}, avoid=avoid
    )
    commit_all(repo)
    added = "// Send also logs the order.\n"
    (repo / "old.go").write_text(package + old + added + "func Send() {}\n")
    (repo / "new.go").write_text(package + new + "func Post() {}\n")

    result = run_checker(repo, "--base", "HEAD")

    assert result.returncode == 1
    assert f"new.go:3 Post: {rule}: {message}\n" in result.stdout
    assert (
        f"old.go:3 Send: {rule}: {message} (existing: report only)\n"
        in result.stdout
    )


@pytest.mark.parametrize(
    "sentence",
    [
        pytest.param(
            "The caller is responsible for closing it.", id="ownership"
        ),
        pytest.param(
            "Send has no other mechanism for limiting work.", id="limit"
        ),
    ],
)
def test_ownership_sentences_are_not_filler(
    tmp_path: Path, sentence: str
) -> None:
    """Pass a new sentence about who owns what, despite filler words.

    The skill asks for ownership and limits, and these sentences state
    them with words from its filler examples. Failing on them would
    push agents to cut the facts the skill wants.
    """
    repo = write_repo(tmp_path, {})
    commit_all(repo)
    source = f"package shop\n\n// Send opens a queue. {sentence}\n"
    (repo / "send.go").write_text(source + "func Send() {}\n")

    result = run_checker(repo, "--base", "HEAD")

    assert result.returncode == 0
    assert "filler" not in result.stdout


@pytest.mark.parametrize("layout", ["repo-in-subfolder", "file-moved"])
def test_older_sentences_stay_report_only(tmp_path: Path, layout: str) -> None:
    """Keep a person's sentence report-only wherever the file now lives.

    Git reads a path in ``rev:path`` from the top of the repository,
    and a moved file has a new path. Either way, an older comment
    could look new, and the agent would be told to rewrite a person's
    text. Questions about unchanged sentences are left out: on one
    unchanged Go package they made 664 lines of noise.
    """
    repo = tmp_path / "service"
    repo.mkdir()
    source = "package shop\n\n// Send facilitates the queue.\nfunc Send() {}\n"
    write_repo(repo, {"send.go": source})
    git = commit_all(tmp_path)
    if layout == "file-moved":
        subprocess.run(
            [*git, "mv", "service/send.go", "service/queue.go"], check=True
        )

    result = run_checker(repo, "--base", "HEAD")

    assert result.returncode == 0
    assert 'filler: "facilitates" (existing: report only)' in result.stdout
    assert "restated-name" not in result.stdout


def test_only_license_headers_are_report_only(tmp_path: Path) -> None:
    """Report license headers and skip generated files, but not doc text.

    A license header above ``package`` and a generated file are text
    no agent should rewrite, even when they are new. A package doc
    comment also sits above ``package``, but an agent writes it, so
    new filler there must still fail.
    """
    repo = write_repo(tmp_path, {})
    commit_all(repo)
    (repo / "send.go").write_text(
        "// Copyright Shop. This facilitates nothing.\n\npackage shop\n"
    )
    (repo / "gen.go").write_text(
        "// Code generated by stringer. DO NOT EDIT.\n\n"
        "package shop\n\n// Red facilitates the palette.\nconst Red = 1\n"
    )
    (repo / "doc.go").write_text(
        "// Package shop seamlessly sells things.\npackage shop\n"
    )

    result = run_checker(repo, "--base", "HEAD")

    assert result.returncode == 1
    assert "(header: report only)" in result.stdout
    assert 'doc.go:1 shop: filler: "seamlessly"\n' in result.stdout
    assert "gen.go" not in result.stdout


def settings_of(repo: Path) -> object:
    """Read the settings file the checker wrote.

    Args:
        repo: Repository written by ``write_repo``.

    Returns:
        The parsed ``.code-comments.json``.
    """
    return json.loads((repo / ".code-comments.json").read_text())


def test_approve_round_trips_exact_text(tmp_path: Path) -> None:
    """Write a settings file that the checker then passes unchanged.

    A hand-typed entry that drops a tab or the final newline makes the
    checker report the owner's own comment as changed, and the agent
    restores the wrong text. Cut comments must be listed too, or an
    agent can put them back. The owner first saved printed output with
    a shell redirect, which emptied the file before the checker read
    it, so ``--approve`` writes the file and prints only a summary.
    """
    blue = '\t// Blue: "blue" is shown when none is set.\n'
    source = f'package shop\n\nconst (\n{blue}\tBlue = "blue"\n)\n'
    avoid = {"client": "say customer"}
    files = {"cart.go": source + TOTAL + "func Total()\n"}
    repo = write_repo(tmp_path, files, avoid=avoid)
    commit_all(repo)
    (repo / "cart.go").write_text(source + "func Total()\n")

    printed = run_checker(repo, "--base", "HEAD", "--approve")
    result = run_checker(repo)

    assert printed.stdout == (
        ".code-comments.json: 1 added or changed, 0 kept, 1 cut\n"
    )
    assert settings_of(repo) == {
        "approved": [
            {"file": "cart.go", "name": "Blue", "text": blue},
            {"file": "cart.go", "name": "Total", "text": ""},
        ],
        "avoid": avoid,
    }
    assert result.returncode == 0


def test_approve_keeps_other_files_approvals(tmp_path: Path) -> None:
    """Keep every earlier approval when the owner approves another file.

    The owner approves one package at a time, and each run rewrites
    the settings file. Dropping the other package's entries would let
    agents rewrite that text, and dropping a cut entry would let them
    put the comment back.
    """
    adds, sends = "// A adds the prices.\n", "// B sends the bill.\n"
    files = {"a.go": adds + "func A() {}\n", "b.go": sends + "func B() {}\n"}
    avoid = {"client": "say customer"}
    repo = write_repo(tmp_path, files, [("b.go", "Old", "")], avoid)

    for file in ("a.go", "b.go"):
        run_checker(repo, "--approve", only=file)
    result = run_checker(repo)

    assert settings_of(repo) == {
        "approved": [
            {"file": "b.go", "name": "B", "text": sends},
            {"file": "a.go", "name": "A", "text": adds},
            {"file": "b.go", "name": "Old", "text": ""},
        ],
        "avoid": avoid,
    }
    assert result.returncode == 0


def test_rename_carries_approvals_to_new_names(tmp_path: Path) -> None:
    """Record approved and cut comments under the names after a rename.

    Approved entries are keyed by name. When a task renamed the code,
    ``--approve`` recorded the owner's reviewed comments as cut under
    the old names, and left earlier cuts under names nothing has, so
    an agent could add those comments back unnoticed.
    """
    reads = "// Load reads the saved cart.\n"
    source = "package shop\n\n" + TOTAL + "func Total() {}\n\n"
    write_repo(
        tmp_path,
        {"cart.go": source + reads + "func Load() {}\n"},
        [("cart.go", "Total", TOTAL), ("cart.go", "Empty", "")],
    )
    commit_all(tmp_path)
    renamed = "package shop\n\n" + TOTAL + "func Sum() {}\n\n"
    (tmp_path / "cart.go").write_text(renamed + reads + "func Read() {}\n")

    run_checker(
        tmp_path,
        "--base",
        "HEAD",
        "--approve",
        "--rename",
        "Total=Sum",
        "--rename",
        "Load=Read",
        "--rename",
        "Empty=IsEmpty",
    )

    assert settings_of(tmp_path) == {
        "approved": [
            {"file": "cart.go", "name": "Sum", "text": TOTAL},
            {"file": "cart.go", "name": "Read", "text": reads},
            {"file": "cart.go", "name": "IsEmpty", "text": ""},
        ],
        "avoid": {},
    }


def test_moved_approved_text_is_reported_not_failed(tmp_path: Path) -> None:
    """Keep approved text protected under its new name until recorded.

    After a rename, the owner's unchanged text sits under a name the
    settings file does not know. Failing would push the agent to undo
    the rename; treating the text as new would let it be rewritten.
    Without ``--base`` the checker cannot tell a new name from an old
    one, so the comment still counts as gone.
    """
    total = "// This is responsible for the cart total.\n"
    repo = write_repo(tmp_path, {}, [("cart.go", "Total", total)])
    commit_all(repo)
    source = "package shop\n\n" + total + "func Sum() {}\n"
    (repo / "cart.go").write_text(source)

    result = run_checker(repo, "--base", "HEAD")
    without_base = run_checker(repo)

    assert result.returncode == 0
    assert (
        "cart.go Total: approved: approved text moved to cart.go:3 Sum; "
        "record it with --approve --rename (renamed: report only)\n"
        in result.stdout
    )
    assert "(approved: report only)" in result.stdout
    assert without_base.returncode == 1


DEPRECATED = "// Deprecated: do not use.\n"


@pytest.mark.parametrize(
    "before, after, approved, name",
    [
        pytest.param(
            DEPRECATED + "func A() {}\n",
            "func A() {}\n\n" + DEPRECATED + "func B() {}\n",
            [("cart.go", "A", DEPRECATED), ("cart.go", "B", DEPRECATED)],
            "A",
            id="copy-has-own-entry",
        ),
        pytest.param(
            TOTAL + "func Total() {}\n\n// Pay charges the card.\n"
            "func Pay() {}\n",
            TOTAL + "func Pay() {}\n",
            [("cart.go", "Total", TOTAL)],
            "Total",
            id="copy-over-older-comment",
        ),
        pytest.param(
            DEPRECATED + "func A() {}\n\n" + DEPRECATED + "func B() {}\n",
            DEPRECATED + "func C() {}\n",
            [("cart.go", "A", DEPRECATED), ("cart.go", "B", DEPRECATED)],
            "B",
            id="one-copy-for-two",
        ),
    ],
)
def test_copies_cannot_hide_a_deleted_comment(
    tmp_path: Path,
    before: str,
    after: str,
    approved: list[tuple[str, str, str]],
    name: str,
) -> None:
    """Fail on a deleted approved comment even when its text remains.

    Short approved text such as a ``Deprecated:`` line often sits on
    several declarations. Counting another copy as the deleted comment
    under a new name would let an agent delete the owner's text and
    pass. Only a name with no comment at ``--base`` and no entry can
    receive moved text, and each copy can stand for one comment.
    """
    package = "package shop\n\n"
    repo = write_repo(tmp_path, {"cart.go": package + before}, approved)
    commit_all(repo)
    (repo / "cart.go").write_text(package + after)

    result = run_checker(repo, "--base", "HEAD")

    assert result.returncode == 1
    assert f"cart.go {name}: approved: approved comment is gone" in (
        result.stdout
    )


def test_unknown_rename_name_is_warned(tmp_path: Path) -> None:
    """Warn when ``--rename`` names nothing in the checked files.

    A mistyped old name changes nothing: the comment is still recorded
    as cut under its old name, and the owner believes the rename was
    recorded. The run itself still succeeds.
    """
    source = "package shop\n\n" + TOTAL + "func Total() {}\n"
    repo = write_repo(tmp_path, {"cart.go": source})
    commit_all(repo)
    (repo / "cart.go").write_text(
        "package shop\n\n" + TOTAL + "func Add() {}\n"
    )

    result = run_checker(
        repo, "--base", "HEAD", "--approve", "--rename", "Totl=Add"
    )

    assert result.returncode == 0
    assert (
        "--rename Totl=Add: no entry or --base comment is named 'Totl' "
        "in the checked files\n" in result.stderr
    )


def test_gone_approved_comment_names_likely_rename(tmp_path: Path) -> None:
    """Point at the comment a gone approved comment probably became.

    A Go doc comment starts with its name, so a rename changes the
    approved text and the checker can only say it is gone. Naming the
    comment that differs only in identifiers tells the agent where the
    owner's text went. A comment that says something else is not named.
    """
    total = "// Total is in cents, so ParseCart rounds half-cents down.\n"
    source = (
        "package shop\n\n"
        "// Sum is in cents, so ParseOrder rounds half-cents down.\n"
        "func Sum() {}\n\n"
        "// Count is in items, so ParseOrder rounds half-items down.\n"
        "func Count() {}\n"
    )
    repo = write_repo(
        tmp_path, {"cart.go": source}, [("cart.go", "Total", total)]
    )

    result = run_checker(repo)

    assert result.returncode == 1
    assert (
        "cart.go Total: approved: approved comment is gone; "
        "possibly now Sum at 3\n" in result.stdout
    )


@pytest.mark.parametrize(
    "settings",
    [
        pytest.param([], id="bare-list"),
        pytest.param({"avoid": ["client"]}, id="avoid-not-object"),
        pytest.param({"approved": [{"file": "cart.go"}]}, id="missing-text"),
    ],
)
def test_wrong_settings_shape_exits_two(
    tmp_path: Path, settings: object
) -> None:
    """Exit 2 with a message when the settings file has the wrong shape.

    An earlier ``--approve`` printed a bare list, and saving it made
    the checker crash with exit 1, which agents read as changed
    approved text to restore.
    """
    repo = write_repo(tmp_path, {"cart.go": "package shop\n"})
    (repo / ".code-comments.json").write_text(json.dumps(settings))

    result = run_checker(repo)

    assert result.returncode == 2
    assert "expected" in result.stderr
    assert "Traceback" not in result.stderr
