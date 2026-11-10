"""Run the merge checks of the pull-request skill and print a summary.

Fetches the PR, runs the tests on it merged into its base with
merge-check.sh, lists the changed files, counts GitHub's checks, and
builds and lints the squash message. Prints at most twenty plain
lines, so a small local model can run merges. Read ``prepare_merge``
first.

Example:
    python3 ~/.agents/skills/pull-request/scripts/merge_prep.py \\
        453 'cd ai && uv run pytest -m "not local_ai"'
"""

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
MERGE_CHECK = SCRIPTS / "merge-check.sh"
MSG = SCRIPTS.parents[1] / "git-messages/scripts/msg"
PR_FIELDS = (
    "title,body,state,isDraft,mergeable,baseRefName,headRefOid,"
    "statusCheckRollup"
)
TESTS_SHOWN = 5
FILES_SHOWN = 5
CHECKS_SHOWN = 3
LINE_WIDTH = 120
# A check run that ends in one of these does not block a merge.
PASSING_CONCLUSIONS = {"SUCCESS", "NEUTRAL", "SKIPPED"}


def prepare_merge(
    number: int, commands: list[str], remote: str, out: Path
) -> tuple[list[str], bool]:
    """Run every merge check on PR number and build the summary.

    Runs each check even after one fails, so one run names every
    problem. Writes ``pr.txt``, ``files.txt``, ``squash.txt``,
    ``body.txt`` and the test logs in ``tests/`` to out.

    Args:
        number: Pull request number on GitHub.
        commands: Test commands, each run with sh from the merged
            tree's root.
        remote: Git remote that GitHub serves the PR from.
        out: Existing folder for the files the summary names.

    Returns:
        The summary lines and whether the PR is ready to merge.

    Raises:
        subprocess.CalledProcessError: When git or gh fails.
    """
    pr = json.loads(run("gh", "pr", "view", str(number), "--json", PR_FIELDS))
    base = pr["baseRefName"]
    run("git", "fetch", "-q", remote, base)
    run("git", "fetch", "-q", remote, f"pull/{number}/head")
    head = run("git", "rev-parse", "FETCH_HEAD").strip()
    base_commit = run("git", "rev-parse", f"{remote}/{base}").strip()
    problems = []
    if head != pr["headRefOid"]:
        problems.append("PR head moved")
    lines = [
        f"pr {number}: {pr['title']}",
        f"base {base} {base_commit[:8]}, head {head[:8]}",
        summarize_state(pr, problems),
    ]
    lines += summarize_tests(f"{remote}/{base}", head, commands, out, problems)
    lines += summarize_files(number, out)
    lines += summarize_checks(pr["statusCheckRollup"], problems)
    lines.append(summarize_squash(pr, out, problems))
    lines = [line[:LINE_WIDTH] for line in lines]
    # The skill reads the folder from this line, so it is never cut.
    lines.append(f"out: {out}")
    if problems:
        lines.append("result: not ready: " + "; ".join(problems))
    else:
        lines.append("result: ready")
    return lines, not problems


def summarize_state(pr: dict, problems: list[str]) -> str:
    """Return the state line, adding a problem when GitHub blocks a merge.

    Args:
        pr: Fields from ``gh pr view``.
        problems: Reasons the PR is not ready, appended to.

    Returns:
        One line, such as ``state: OPEN, MERGEABLE``.
    """
    if pr["state"] != "OPEN":
        problems.append(f"PR is {pr['state'].lower()}")
    if pr["isDraft"]:
        problems.append("PR is a draft")
    # GitHub says UNKNOWN while it works the answer out, and the merge
    # check below tests the merge itself.
    if pr["mergeable"] == "CONFLICTING":
        problems.append("PR conflicts with its base")
    draft = ", draft" if pr["isDraft"] else ""
    return f"state: {pr['state']}, {pr['mergeable']}{draft}"


def summarize_tests(
    base: str, head: str, commands: list[str], out: Path, problems: list[str]
) -> list[str]:
    """Run the test commands on head merged into base with merge-check.sh.

    Args:
        base: Base commit, such as ``github/main``.
        head: Commit of the PR.
        commands: Test commands for merge-check.sh.
        out: Folder whose ``tests`` folder gets the logs.
        problems: Reasons the PR is not ready, appended to.

    Returns:
        A count line, then one line per command, at most
        ``TESTS_SHOWN`` of them.
    """
    logs = out / "tests"
    summary = logs / "summary.txt"
    # A reused out folder must not show the last run's results.
    summary.unlink(missing_ok=True)
    result = subprocess.run(
        ["bash", str(MERGE_CHECK), base, head, str(logs), *commands],
        capture_output=True,
        text=True,
        check=False,
    )
    results = (
        summary.read_text().splitlines()[2:-1] if summary.exists() else []
    )
    if not results:
        problems.append("tests")
        error = (result.stderr.strip().splitlines() or ["no output"])[-1]
        return [f"tests: merge check failed: {error}"]
    if results[0].startswith("no clean merge"):
        problems.append("merge conflict")
        return [f"tests: {results[0]}"]
    if result.returncode != 0:
        problems.append("tests")
    passed = sum(line.startswith("exit 0:") for line in results)
    return [
        f"tests on the merge into {base}: {passed} of {len(results)} passed,"
        f" logs in {logs}",
        *("  " + line for line in results[:TESTS_SHOWN]),
    ]


def summarize_files(number: int, out: Path) -> list[str]:
    """List the PR's changed files to files.txt and return the first few.

    Args:
        number: Pull request number.
        out: Folder for ``files.txt``.

    Returns:
        A count line, then at most ``FILES_SHOWN`` file names.
    """
    files = run("gh", "pr", "diff", str(number), "--name-only").splitlines()
    (out / "files.txt").write_text("".join(f"{name}\n" for name in files))
    return [
        f"files: {len(files)}, all in {out / 'files.txt'}",
        *("  " + name for name in files[:FILES_SHOWN]),
    ]


def summarize_checks(rollup: list[dict], problems: list[str]) -> list[str]:
    """Count GitHub's checks and name the first ones that block a merge.

    Args:
        rollup: ``statusCheckRollup`` from ``gh pr view``, which holds
            check runs and commit statuses.
        problems: Reasons the PR is not ready, appended to.

    Returns:
        A count line, and a line naming failed or pending checks when
        there are any.
    """
    passed, failed, pending = [], [], []
    for check in rollup:
        if check["__typename"] == "CheckRun":
            name = check["name"]
            if check["status"] != "COMPLETED":
                pending.append(name)
            elif check["conclusion"] in PASSING_CONCLUSIONS:
                passed.append(name)
            else:
                failed.append(name)
        else:
            name = check["context"]
            if check["state"] == "SUCCESS":
                passed.append(name)
            elif check["state"] in ("PENDING", "EXPECTED"):
                pending.append(name)
            else:
                failed.append(name)
    lines = [
        f"checks: {len(passed)} passed, {len(failed)} failed,"
        f" {len(pending)} pending"
    ]
    if failed or pending:
        problems.append("checks")
        blocking = failed + pending
        lines.append("  not passed: " + ", ".join(blocking[:CHECKS_SHOWN]))
    return lines


def summarize_squash(pr: dict, out: Path, problems: list[str]) -> str:
    """Build the squash message and its body, then lint the message.

    Writes the PR's title and description to ``pr.txt``, the squash
    message to ``squash.txt``, and the message without its title to
    ``body.txt``, the file the merge command takes.

    Args:
        pr: Fields from ``gh pr view``.
        out: Folder for the three files.
        problems: Reasons the PR is not ready, appended to.

    Returns:
        One line, ``squash lint: ok`` or the first error.
    """
    # A reused out folder must not keep the last run's message.
    for name in ("squash.txt", "body.txt"):
        (out / name).unlink(missing_ok=True)
    message = out / "pr.txt"
    body = pr["body"].replace("\r\n", "\n")
    message.write_text(f"{pr['title']}\n\n{body}\n")
    squash = subprocess.run(
        [sys.executable, str(MSG), "squash", str(message)],
        capture_output=True,
        text=True,
        check=False,
    )
    if squash.returncode != 0:
        problems.append("squash message")
        return f"squash: {squash.stderr.strip()}"
    (out / "squash.txt").write_text(squash.stdout)
    _, _, rest = squash.stdout.partition("\n\n")
    (out / "body.txt").write_text(rest)
    lint = subprocess.run(
        [sys.executable, str(MSG), "lint", str(out / "squash.txt")],
        capture_output=True,
        text=True,
        check=False,
    )
    if lint.returncode != 0:
        problems.append("squash message")
        errors = lint.stdout.strip().splitlines() or ["no output"]
        return f"squash lint: {len(errors)} errors; first: {errors[0]}"
    return "squash lint: ok"


def run(*command: str) -> str:
    """Run command and return its output, raising when it fails.

    Args:
        command: Program and arguments.

    Returns:
        The command's standard output.

    Raises:
        subprocess.CalledProcessError: When the command fails.
    """
    return subprocess.run(
        command, capture_output=True, text=True, check=True
    ).stdout


def pick_remote() -> str:
    """Return the repository's only remote.

    Returns:
        The remote's name.

    Raises:
        ValueError: When the repository has no remote or several.
    """
    remotes = run("git", "remote").split()
    if len(remotes) != 1:
        raise ValueError(
            f"pass --remote; the repository has {len(remotes)} remotes"
        )
    return remotes[0]


def main() -> int:
    """Run the merge checks and print the summary.

    Returns:
        0 when the PR is ready to merge, 1 when it is not, and 2 when
        git or gh fails.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("number", type=int, help="pull request number")
    parser.add_argument("commands", nargs="+", help="test commands")
    parser.add_argument("--remote", help="git remote of the PR")
    parser.add_argument("--out", type=Path, help="folder for the results")
    args = parser.parse_args()
    try:
        remote = args.remote or pick_remote()
        out = args.out or Path(tempfile.mkdtemp(prefix="merge-prep."))
        out.mkdir(parents=True, exist_ok=True)
        lines, ready = prepare_merge(args.number, args.commands, remote, out)
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or "").strip().splitlines() or ["no output"]
        print(f"error: {' '.join(error.cmd)}: {detail[-1]}")
        return 2
    except (OSError, ValueError) as error:
        print(f"error: {error}")
        return 2
    print("\n".join(lines))
    return 0 if ready else 1


if __name__ == "__main__":
    raise SystemExit(main())
