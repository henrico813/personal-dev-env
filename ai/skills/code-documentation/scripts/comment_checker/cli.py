"""Report Go and Python comments that break a code-documentation rule.

Example:
    python3 check_comments.py --repo . internal/shop/*.go
    python3 check_comments.py --repo . --base main internal/shop/*.go
    python3 check_comments.py --repo . --base main --approve shop/*.go
    python3 check_comments.py --repo . --base main --approve \
        --rename Total=Sum --rename cart.go=basket.go shop/*.go
    python3 check_comments.py --repo . --approve-comment shop/cart.go Total
    python3 check_comments.py --repo . --merge-approvals ../wt 1a2b3c4
    python3 check_comments.py --repo . --list scripts/check_comments.py
    python3 check_comments.py list --interface 1 --repo . shop/cart.go

Prints one line per finding, then quotes each broken rule's sentence
from the skill. ``.code-comments.json`` in the ``--repo`` folder holds
the owner's approved text, ``""`` for a cut comment, and words to
avoid. ``--approve`` rewrites the approved text from the checked files,
after ``--rename OLD=NEW`` moves the entries of each renamed comment or
file, so a rename is not recorded as a cut::

    {"approved": [{"file": "store/cart.go", "name": "Cart.Total",
                   "text": "// Total is in cents.\n"}],
     "avoid": {"client": "say customer"}}

``--approve-comment FILE NAME`` records only that comment's text, or
``""`` when it is gone, after the renames, and leaves every other entry
as it was. A ``--rename`` pair of paths moves that file's
entries, and a name pair renames an entry only in FILE. FILE may be
deleted only when, after the renames, it has an entry for NAME; that
entry is then recorded as cut.

``--merge-approvals FOLDER COMMIT`` takes ``FOLDER``'s approvals, such
as a worktree's before removal, for the files ``COMMIT`` changed,
when their text is the comment now. If the two files approve a comment
differently and neither is the comment now, nothing is written.

In Python, every docstring is named, such as ``RenameRules.move_key``
or ``<module>`` for the module docstring, and so is a ``#`` comment
above a ``def``, a ``class`` or an assignment outside functions, such
as ``# RULES`` above ``RULES``.

``--list`` prints one line of JSON per Python file for
``pde-comment-review``, with each named comment's lines and the
warnings Python gives when it compiles the file. Line numbers start at
1, and ``end_line`` is the comment's last line. A ``#`` comment with a
tool directive line, such as ``# noqa``, is left out, since writing
over it could drop the directive.

The commands ``version`` and ``list`` answer ``pde-comment-review``
with JSON, as ``interface.py`` describes.

The rules are:

- ``approved``: approved text changed, a cut comment came back, or an
  approved comment or its file is gone. Without ``--approve``, every
  file the settings name is checked, even when it is not given. An
  approved file that Python cannot read fails too.
- ``filler``: a sentence about the code uses one of the skill's vague
  filler phrases, such as "facilitates".
- ``avoided-word``: a sentence uses a word from the avoid list.
- ``restated-name``: a comment's first sentence repeats the name of the
  code below it. This never fails the run.

Go files also get ``seven-words``, ``verb-first``, which never fails
the run, ``starts-with-name``, ``comment-length`` and ``comment-colon``,
the rules ``check_python_rules.py`` describes and checks in Python files.

With ``--base``, sentences present at that commit never fail the run,
and ``restated-name`` findings on them are left out. The Go rules skip
function names present at that commit and comments whose every sentence
is. Findings in approved text, or in a license header above
``package`` or a Python file's first statement, are only reported.
Generated files are skipped. A sentence is present only in a file of
the same language.

Exit codes:
    0: Nothing failed.
    1: An ``approved`` finding, or, with ``--base``, a finding other
        than ``restated-name`` or ``verb-first`` on a name or sentence
        not present at that commit, or
        ``--merge-approvals`` found two approvals of a comment, neither
        of them the comment now.
    2: A path is a folder or unreadable, no Go or Python file was given,
        ``--base`` is not a commit, the settings file is not JSON of
        the shape above, ``--approve-comment`` names a FILE that is
        not Go or Python, an empty NAME, a NAME that cannot be approved,
        or a deleted FILE with no entry for NAME, or
        ``--merge-approvals`` names a FOLDER without
        ``.code-comments.json``, or a COMMIT that ``--repo``'s HEAD
        lacks or whose changes Git cannot list. ``--approve-comment``
        and ``--merge-approvals`` also exit 2 on a file Python cannot
        read. ``--list`` also exits 2 on a file that is not Python, does
        not compile or has a carriage return, and then prints nothing.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from . import interface
from .approvals import (
    SETTINGS,
    apply_renames,
    approve_one_comment,
    build_approved_entries,
    list_function_names,
    load_settings,
    merge_approvals,
    parse_rename_pair,
    read_comments_in_files,
    read_files_at_commit,
    replace_settings_file,
    summarize_approval,
    unname_unapprovable_comments,
)
from .languages import LANGUAGES
from .languages.python import PYTHON_DIRECTIVE
from .records import Comment, Function, SourceError
from .rules import (
    RULES,
    find_name_violations,
    find_rule_violations,
    split_sentences,
)


def list_comments(repo: Path, paths: list[Path]) -> int:
    """Print each Python file's named comments as one line of JSON.

    The output lets ``pde-comment-review`` check, after writing the
    owner's answers, that only those comments changed and Python gives
    no new warning.

    Returns:
        The exit code described in the module docstring.
    """
    listings = []
    for path in paths:
        language = LANGUAGES.get(path.suffix)
        if language is None or language.list_warnings is None:
            print(f"{path}: --list takes only Python files", file=sys.stderr)
            return 2
        try:
            file = path.resolve().relative_to(repo).as_posix()
            # read_text would turn a carriage return into a line feed.
            source = path.read_bytes().decode("utf-8")
        except (OSError, ValueError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        try:
            if "\r" in source:
                raise SourceError("cannot be read (it has a carriage return)")
            found = language.read_comments(source, file)
            compile_warnings = language.list_warnings(source, file)
        except SourceError as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        listed = [
            {
                "name": c.name,
                "line": c.line,
                "end_line": c.line + c.text.rstrip("\n").count("\n"),
                "code_line": c.code_line,
                "only_body": c.only_body,
                "text": c.text,
            }
            for c in unname_unapprovable_comments(found)[0]
            if c.name
            # Writing over a # comment would drop its directive lines.
            and not (
                c.name.startswith("# ") and PYTHON_DIRECTIVE.search(c.text)
            )
        ]
        listings.append(
            {"file": file, "comments": listed, "warnings": compile_warnings}
        )
    # Print only once every file is listed, so no refusal follows output.
    for listing in listings:
        print(json.dumps(listing))
    return 0


def main(argv: list[str] | None = None) -> int:
    """Check the given Go and Python files and print one line per finding.

    Args:
        argv: Command-line arguments; ``sys.argv`` when ``None``.

    Returns:
        The exit code described in the module docstring.
    """
    argv = sys.argv[1:] if argv is None else argv
    if argv[:1] and argv[0] in interface.COMMANDS:
        return interface.main(argv)
    # "@renames.txt" reads one argument per line, such as
    # "--rename=const Red=const Crimson".
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0], fromfile_prefix_chars="@"
    )
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument(
        "--base", help="commit to compare with; its sentences never fail"
    )
    parser.add_argument(
        "--approve",
        action="store_true",
        help=f"rewrite {SETTINGS}; only the owner, or at their request",
    )
    parser.add_argument(
        "--rename",
        action="append",
        default=[],
        type=parse_rename_pair,
        metavar="OLD=NEW",
        help="a renamed comment or file; repeat for each",
    )
    parser.add_argument(
        "--approve-comment",
        nargs=2,
        metavar=("FILE", "NAME"),
        help="approve one comment; only the owner, or at their request",
    )
    parser.add_argument(
        "--merge-approvals",
        nargs=2,
        metavar=("FOLDER", "COMMIT"),
        help="take FOLDER's approvals for the files COMMIT changed; run "
        "as the cleanup-plan skill says",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="print each named comment of the given Python files as JSON",
    )
    parser.add_argument("paths", nargs="*", type=Path)
    args = parser.parse_args(argv)
    if args.rename and not (args.approve or args.approve_comment):
        parser.error("--rename needs --approve or --approve-comment")
    repo = args.repo.resolve()
    if args.list:
        if (
            args.approve
            or args.approve_comment
            or args.merge_approvals
            or args.base
        ):
            parser.error("--list takes only --repo and paths")
        if not args.paths:
            parser.error("give the files to list")
        return list_comments(repo, args.paths)
    if args.merge_approvals:
        if args.approve or args.approve_comment or args.base or args.paths:
            parser.error("--merge-approvals takes no other option or paths")
        folder, commit = args.merge_approvals
        return merge_approvals(repo, Path(folder).resolve(), commit)
    if args.approve_comment:
        if args.approve or args.base or args.paths:
            parser.error(
                "--approve-comment takes no --approve, --base or paths"
            )
        file, name = args.approve_comment
        return approve_one_comment(
            repo,
            file,
            name,
            dict(args.rename),
        )
    if not args.paths:
        parser.error("give the files to check")
    try:
        approved, avoid = load_settings(repo)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2
    paths = list(args.paths)
    if not args.approve:
        # An unlisted file keeps its approved text checked; a missing one
        # is reported as gone.
        approved_paths = sorted({repo / file for file, _ in approved})
        paths += [path for path in approved_paths if path.exists()]
    checked_files: set[str] = set()
    comments: list[Comment] = []
    functions: list[Function] = []
    unreadable: dict[str, str] = {}
    for path in paths:
        if path.is_dir():
            print(f"{path}: a folder; pass the files in it", file=sys.stderr)
            return 2
        language = LANGUAGES.get(path.suffix)
        if language is None:
            print(f"{path}: skipped; unknown language", file=sys.stderr)
            continue
        try:
            file = path.resolve().relative_to(repo).as_posix()
            # Skip a file given twice, such as "a.go" and "./a.go".
            if file in checked_files:
                continue
            source = path.read_text(encoding="utf-8")
        except (OSError, ValueError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            return 2
        if language.generated_marker.search(source):
            print(f"{path}: skipped; generated", file=sys.stderr)
            continue
        try:
            comments += language.read_comments(source, file)
        except SourceError as error:
            if path in args.paths:
                print(f"{path}: {error}", file=sys.stderr)
                return 2
            # Its approvals cannot be checked, but they are not gone.
            unreadable[file] = f"approved file {error}; fix the file"
            continue
        if language.list_functions:
            functions += language.list_functions(source, file)
        checked_files.add(file)
    if not checked_files:
        print("no file to check", file=sys.stderr)
        return 2
    comments, unapprovable = unname_unapprovable_comments(comments)
    base_comments: list[Comment] = []
    base_names = None
    if args.base:
        try:
            base_files = read_files_at_commit(
                repo,
                args.base,
                {Path(file).suffix for file in checked_files},
            )
        except ValueError as error:
            print(error, file=sys.stderr)
            return 2
        base_comments = read_comments_in_files(base_files)
        base_names = list_function_names(base_files)
    for (file, name), (reason, lines) in unapprovable.items():
        for line in lines:
            print(f"{file}:{line} {name}: cannot be approved; {reason}")
    if args.approve:
        # Rename first, so a renamed comment's entry moves instead of
        # being cut as gone or dropped with a deleted file.
        current_keys = {(c.file, c.name) for c in comments}
        approved, base_comments, unused = apply_renames(
            dict(args.rename),
            checked_files,
            current_keys,
            approved,
            base_comments,
        )
        for old in unused:
            print(
                f"--rename {old}: matches no entry or --base comment in "
                "the checked files",
                file=sys.stderr,
            )
        entries, removed = build_approved_entries(
            repo, checked_files, comments, base_comments, approved
        )
        replace_settings_file(repo, entries, avoid)
        print(summarize_approval(entries, approved, removed))
        return 0

    base_sentences = None
    if args.base:
        base_sentences = {
            (Path(comment.file).suffix, sentence)
            for comment in base_comments
            for sentence in split_sentences(comment.prose)
        }

    findings = find_rule_violations(
        checked_files,
        comments,
        {k: t for k, t in approved.items() if k[0] not in unreadable},
        avoid,
        base_sentences,
    )
    findings += find_name_violations(functions, base_names)
    broken_rules = ["approved"] if unreadable else []
    for file, message in unreadable.items():
        print(f"{file} -: approved: {message}")
    for finding in findings:
        comment = finding.comment
        # A gone comment has no line to point at.
        at_line = f":{comment.line}" if comment.line else ""
        place = f"{comment.file}{at_line} {comment.name or '-'}"
        note = f" ({finding.note})" if finding.note else ""
        print(f"{place}: {finding.rule}: {finding.message}{note}")
        if finding.rule not in broken_rules:
            broken_rules.append(finding.rule)
    for rule in broken_rules:
        print(f'{rule} rule: "{RULES[rule].sentence}"')
    failing_rules = {"approved"}
    if base_sentences is not None:
        failing_rules |= {
            "avoided-word",
            "comment-colon",
            "comment-length",
            "filler",
            "seven-words",
            "starts-with-name",
        }
    failed = bool(unreadable) or any(
        finding.rule in failing_rules and not finding.note
        for finding in findings
    )
    return 1 if failed else 0
