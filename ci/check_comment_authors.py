"""Fail when a branch adds comment lines the worktree's ledger lacks, changes a file whose comment style it does not know, or leaves AGENTS.md edits uncommitted. Example finding: ci/x.py:12"""

import argparse
import ast
import io
import os
import pathlib
import re
import subprocess
import sys
import tokenize

LEDGER_NAME = "comment-ledger.txt"
MAX_SHOWN = 5
SECTION_START = "<!-- check_comment_authors: local rule, never commit -->"
SECTION_END = "<!-- check_comment_authors: end of local rule -->"
RULE_FILE = "~/.config/pde/comment-author-rule.md"
SECTION = (
    "\n"
    + SECTION_START
    + """
## Who writes code comments in this worktree

Every new code comment, docstring and reworded line in this worktree must come from the model named below, for the reason below.

Model: {model}
Reason: {reason}

- Leave each comment as a placeholder for that model, such as `# the loop stops when stdin closes`.
- Pipe each line that model writes to `python3 ci/check_comment_authors.py record`.
- Before a pull request, run
  `python3 ci/check_comment_authors.py check <base>`.
  Fails on each added or changed comment line missing from the ledger at `{ledger}`, on each changed file whose comment style it does not know, and when AGENTS.md has edits that are not committed.
- This section is local: AGENTS.md is marked skip-worktree, so Git
  ignores it and every other AGENTS.md edit made here.
  To change AGENTS.md, or when a merge, rebase or checkout stops on it, run `python3 ci/check_comment_authors.py teardown`, then edit and commit AGENTS.md, then run setup again.
"""
    + SECTION_END
    + "\n"
)
SUFFIXES = {
    ".py": "python",
    ".lua": "dash",
    **dict.fromkeys(".sh .bash .zsh .toml .yml .yaml .conf .cfg".split(), "hash"),
    **dict.fromkeys(".go .rs .ts .tsx .mts .cts .js .jsx .mjs .cjs".split(), "slash"),
}
SKIPPED_SUFFIXES = set(
    ".md .json .jsonl .lock .sum .png .jpg .jpeg .gif .webp .ico .pdf".split()
)
SHEBANGS = {
    "python": "python",
    "node": "slash",
    "sh": "hash",
    "bash": "hash",
    "zsh": "hash",
}
MARKERS = {"hash": "#", "dash": "--"}
DIFF_OPTIONS = "--no-color --no-ext-diff --no-textconv --no-relative --text -M".split()
HUNK = re.compile(r"^@@ -\S+ \+(\d+)(?:,(\d+))? @@")
LEADING_MARKER = re.compile(
    r"""^\s*(?:[rRuUbB]*(?:\"\"\"|''')|\{\{-?\s*/\*+|\*+/\s*-?\}\}|/\*+!?|\*+/?|//[/!]?|#+|--+(?:\[\[)?)\s*"""
)
TRAILING_MARKER = re.compile(r"""\s*(?:\"\"\"|'''|\*+/\s*-?\}\}|\*+/|\]\])\s*$""")
TEMPLATE_COMMENT = re.compile(r"\{\{-?\s*/\*")


class SetupError(Exception):
    """A usage or setup failure that main reports and turns into exit code 2."""


def main(argv=None):
    """Run one subcommand and return 0 on a pass, 1 for unrecorded comment lines, unchecked files or uncommitted AGENTS.md edits, and 2 for any error."""
    parser = argparse.ArgumentParser(
        description="Fail when a branch adds comment lines not in the ledger."
    )
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("setup", help="add the local rule and the ledger")
    commands.add_parser("teardown", help="remove the local rule")
    commands.add_parser("record", help="add stdin lines to the ledger")
    check = commands.add_parser("check", help="check BASE...HEAD")
    check.add_argument("base")
    check.add_argument("head", nargs="?", default="HEAD")
    args = parser.parse_args(argv)
    try:
        # git reads pathspecs from the current folder, so every git call runs from the worktree top level and check gives the same result from a subfolder.
        os.chdir(run_git("rev-parse", "--show-toplevel").strip())
        if args.command == "setup":
            return set_up_worktree()
        if args.command == "teardown":
            return tear_down_worktree()
        if args.command == "record":
            return record_lines(sys.stdin)
        return check_branch(args.base, args.head)
    except Exception as error:
        message = " ".join(str(error).split()) or type(error).__name__
        print(f"check_comment_authors: {message}", file=sys.stderr)
        return 2


def set_up_worktree():
    """Append the rule to the tracked AGENTS.md once, hide it with skip-worktree, create the ledger, and refuse an AGENTS.md with uncommitted edits."""
    model, reason = read_comment_rule()
    root, agents = find_agents_file()
    status = run_git("-C", str(root), "status", "--porcelain", "--", "AGENTS.md")
    if status.strip():
        raise SetupError(f"{agents} has uncommitted changes; commit them first")
    ledger = find_ledger()
    text = agents.read_bytes().decode("utf-8")
    # set skip-worktree before the write, so a failed write leaves no dirty worktree for build scripts to refuse
    run_git("-C", str(root), "update-index", "--skip-worktree", "AGENTS.md")
    if SECTION_START not in text:
        section = SECTION.format(ledger=ledger, model=model, reason=reason)
        agents.write_bytes((text + section).encode("utf-8"))
    ledger.touch()
    print(f"ledger: {ledger}")
    return 0


def read_comment_rule():
    """Return the model name and the reason from the local rule file, and raises SetupError naming that file when it is missing or lacks either line."""
    path = pathlib.Path(os.path.expanduser(RULE_FILE))
    text = path.read_text(encoding="utf-8") if path.is_file() else ""
    fields = {}
    for line in text.splitlines():
        key, _, value = line.partition(":")
        fields.setdefault(key.strip(), value.strip())
    model, reason = fields.get("Model"), fields.get("Reason")
    if not model or not reason:
        raise SetupError(f"{path} must exist and hold a Model: line and a Reason: line")
    return model, reason


def tear_down_worktree():
    """Remove exactly the section setup added, keep every other AGENTS.md edit, and clear skip-worktree so those edits show in git status."""
    root, agents = find_agents_file()
    text = agents.read_bytes().decode("utf-8")
    rest = remove_local_rule(text)
    if rest == text:
        raise SetupError(f"{agents} has no check_comment_authors section")
    agents.write_bytes(rest.encode("utf-8"))
    run_git("-C", str(root), "update-index", "--no-skip-worktree", "AGENTS.md")
    print(f"removed the local rule from {agents}")
    return 0


def remove_local_rule(text):
    """Return the AGENTS.md text without the section setup added, or the text unchanged when it holds no whole section."""
    start = text.find("\n" + SECTION_START)
    end = text.find(SECTION_END + "\n", max(start, 0))
    if start < 0 or end < 0:
        return text
    return text[:start] + text[end + len(SECTION_END) + 1 :]


def record_lines(lines):
    """Add each non-blank stdin line, without comment markers, to the ledger."""
    ledger = find_existing_ledger()
    entries = [normalize(line) for line in lines]
    entries = [entry for entry in entries if entry]
    with ledger.open("a", encoding="utf-8") as file:
        file.writelines(entry + "\n" for entry in entries)
    print(f"recorded {len(entries)} lines in {ledger}")
    return 0


def check_branch(base, head):
    """Print a count and the first few file:line hits for comment lines head adds since its merge base with base that the ledger lacks, the files it cannot check, and AGENTS.md when it has uncommitted edits."""
    ledger = find_existing_ledger()
    recorded = set(ledger.read_text(encoding="utf-8").split("\n"))
    start = run_git("merge-base", base, head).strip()
    hits = []
    unchecked = []
    for path, numbers in list_added_lines(start, head).items():
        text = run_git("cat-file", "blob", f"{head}:{path}")
        style = pick_comment_style(path, text)
        if style is None:
            unchecked.append(path)
            continue
        comments = find_comments(style, text, template=path.endswith(".tmpl"))
        for number, comment in comments.items():
            if number in numbers and comment not in recorded:
                hits.append(f"{path}:{number}")
    _, agents = find_agents_file()
    agents_text = agents.read_bytes().decode("utf-8")
    edited = remove_local_rule(agents_text) != run_git("show", "HEAD:AGENTS.md")
    print(f"{len(hits)} added comment lines are not in {ledger}")
    for hit in hits[:MAX_SHOWN]:
        print(hit)
    if unchecked:
        print(f"{len(unchecked)} changed files have no known comment style")
        for path in unchecked[:MAX_SHOWN]:
            print(path)
    if edited:
        print(
            f"{agents}: "
            "has edits that are not committed; run teardown, then edit and commit AGENTS.md, then run setup again"
        )
    return 1 if hits or unchecked or edited else 0


def run_git(*args):
    """Return git's stdout as text with its line endings kept, and raise SetupError with git's message when git fails."""
    result = subprocess.run(["git", *args], capture_output=True)
    stdout = result.stdout.decode("utf-8", errors="replace")
    stderr = result.stderr.decode("utf-8", errors="replace")
    if result.returncode != 0:
        raise SetupError(stderr.strip() or f"git {args[0]} failed")
    return stdout


def find_agents_file():
    """Return the worktree root and its AGENTS.md, refuse an untracked one since skip-worktree needs a tracked file, and refuse a symlink since setup would write into its target."""
    root = pathlib.Path(run_git("rev-parse", "--show-toplevel").strip())
    agents = root / "AGENTS.md"
    if not run_git("-C", str(root), "ls-files", "AGENTS.md").strip():
        raise SetupError(f"{agents} is not tracked by git")
    if agents.is_symlink():
        raise SetupError(f"{agents} is a symlink")
    return root, agents


def find_ledger():
    """The ledger lives in this worktree's own git folder, so git status never shows it and git add never commits it."""
    path = run_git("rev-parse", "--path-format=absolute", "--git-path", LEDGER_NAME)
    return pathlib.Path(path.strip())


def find_existing_ledger():
    """Return the ledger path, raising SetupError when setup has not made it yet."""
    ledger = find_ledger()
    if not ledger.exists():
        raise SetupError(f"no ledger at {ledger}; run setup first")
    return ledger


def list_added_lines(start, head):
    """Map each file changed in start..head to the line numbers its new version adds or changes, and keep unusual file names intact with the NUL-separated list."""
    status = run_git("diff", "-z", "--name-status", *DIFF_OPTIONS, start, head)
    fields = status.split("\0")
    added = {}
    index = 0
    while index < len(fields) - 1:
        kind = fields[index]
        width = 2 if kind[0] in "RC" else 1
        paths = fields[index + 1 : index + 1 + width]
        index += 1 + width
        if kind != "D":
            numbers = list_new_line_numbers(start, head, paths)
            if numbers:
                added[paths[-1]] = numbers
    return added


def list_new_line_numbers(start, head, paths):
    """Read only the hunk headers of one file's diff; every content line starts with +, - or a space and cannot look like a header."""
    args = ("diff", "--unified=0", *DIFF_OPTIONS, start, head, "--", *paths)
    diff = run_git("--literal-pathspecs", *args)
    numbers = set()
    for line in diff.split("\n"):
        if match := HUNK.match(line):
            first = int(match[1])
            count = int(match[2] or "1")
            numbers.update(range(first, first + count))
    return numbers


def pick_comment_style(path, text):
    """Pick the comment style from the file name, a .tmpl by the name inside it and an extensionless file by its shebang, return skip for SKIPPED_SUFFIXES and binary files, and None when it cannot classify."""
    name = pathlib.PurePosixPath(path).name.removesuffix(".tmpl")
    suffix = pathlib.PurePosixPath(name).suffix
    if suffix in SKIPPED_SUFFIXES:
        return "skip"
    if suffix in SUFFIXES:
        return SUFFIXES[suffix]
    if name.startswith("Dockerfile"):
        return "hash"
    if not suffix and name.startswith((".", "dot_")):
        return "hash"
    if not suffix and text.startswith("#!"):
        return read_shebang(text.split("\n", 1)[0])
    if "\0" in text:
        return "skip"
    return None


def read_shebang(first_line):
    """Map a shebang such as #!/usr/bin/env python3 to its comment style, or None for another program."""
    names = [pathlib.PurePosixPath(word).name for word in first_line[2:].split()]
    if names and names[0] == "env":
        names = [name for name in names[1:] if not name.startswith("-")]
    program = re.sub(r"[\d.]+$", "", names[0]) if names else ""
    return SHEBANGS.get(program)


def find_comments(style, text, template=False):
    """Map each comment line number to its text without markers, drop lines that are empty once markers are stripped, and split on newline only, since git counts lines by newline. With template=True it also maps each line of a Go template comment."""
    lines = text.split("\n")
    if style == "python":
        found = find_python_comments(text)
    elif style == "slash":
        found = find_slash_comments(lines)
    elif style in MARKERS:
        found = find_marker_comments(lines, MARKERS[style])
    else:
        found = {}
    if template:
        found.update(find_template_comments(lines))
    keys = {number: normalize(comment) for number, comment in found.items()}
    return {number: key for number, key in keys.items() if key}


def find_python_comments(text):
    """Find real # comments and docstring lines, so a # inside a string is not flagged, and fall back to the line-based # scan, which finds no docstrings, when code does not parse."""
    try:
        tree = ast.parse(text)
        tokens = list(tokenize.generate_tokens(io.StringIO(text).readline))
    except (SyntaxError, ValueError, tokenize.TokenError):
        return find_marker_comments(text.split("\n"), "#")
    found = {
        token.start[0]: token.string
        for token in tokens
        if token.type == tokenize.COMMENT
        and not (token.start == (1, 0) and token.string.startswith("#!"))
    }
    lines = text.split("\n")
    documented = (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)
    for node in ast.walk(tree):
        if not isinstance(node, documented):
            continue
        if ast.get_docstring(node, clean=False) is None:
            continue
        docstring = node.body[0]
        for number in range(docstring.lineno, docstring.end_lineno + 1):
            found[number] = lines[number - 1]
    return found


def find_marker_comments(lines, marker):
    """Find whole-line and trailing # or -- comments, skip a shebang on line 1, and follow a Lua --[[ block to its ]] line."""
    found = {}
    in_block = False
    for number, line in enumerate(lines, start=1):
        if number == 1 and line.startswith("#!"):
            continue
        if in_block:
            found[number] = line
            in_block = "]]" not in line
            continue
        start = find_comment_start(line, marker)
        if start is not None:
            found[number] = line[start:]
            opens_block = line.startswith(marker + "[[", start)
            in_block = opens_block and "]]" not in line[start:]
    return found


def find_comment_start(line, marker, quotes="'\""):
    """Return where a comment starts: the marker at the line start or after whitespace, outside quotes; quotes that span lines, such as heredocs, are not tracked."""
    quote = None
    previous = " "
    for index, char in enumerate(line):
        if quote:
            quote = None if char == quote else quote
        elif char in quotes:
            quote = char
        elif previous.isspace() and line.startswith(marker, index):
            return index
        previous = char
    return None


def find_template_comments(lines):
    """Find each line of a Go template comment, from {{/* or {{- /* anywhere in a line through the line that holds */, such as the .toml.tmpl line {{/* set a */}}."""
    found = {}
    in_block = False
    for number, line in enumerate(lines, start=1):
        if in_block:
            found[number] = line
            in_block = "*/" not in line
            continue
        match = TEMPLATE_COMMENT.search(line)
        if match:
            found[number] = line[match.start() :]
            in_block = "*/" not in line[match.start() :]
    return found


def find_slash_comments(lines):
    """Find // comments at the line start or after whitespace outside double quotes, and every line of a /* */ block that starts a line, and skip //go: directives, which are not comments."""
    found = {}
    in_block = False
    for number, line in enumerate(lines, start=1):
        text = line.strip()
        if in_block or text.startswith("/*"):
            found[number] = text
            in_block = not text.endswith("*/")
            continue
        start = find_comment_start(line, "//", quotes='"')
        if start is not None and not line.startswith("//go:", start):
            found[number] = line[start:]
    return found


def normalize(text):
    """Strip comment and docstring markers so a ledger line matches the code line: '# one' and 'one' both give 'one'."""
    text = LEADING_MARKER.sub("", text, count=1)
    return TRAILING_MARKER.sub("", text).strip()


if __name__ == "__main__":
    sys.exit(main())
