"""Merge a pull request the user reviewed, for the hub's merge worker.

merge-worker.sh runs this in its own session and saves what it prints
as the summary the hub reads. It brings a branch that is behind its
base up to date with a merge commit, runs merge_prep.py, sends the
merge request through pde-gh-write, waits for the user's answer and
prints the merge commit. Read ``merge_pull_request`` first.

Example:
    python3 merge_worker.py 453 /tmp/job 'cd ai && uv run pytest'
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

MERGE_PREP = (
    Path(__file__).resolve().parents[2] / "pull-request/scripts/merge_prep.py"
)
PR_FIELDS = "baseRefName,headRefName,isCrossRepository,statusCheckRollup"
FILES_SHOWN = 5
CHECKS_WAIT_SECONDS = 30 * 60
# Tests set MERGE_WORKER_POLL_SECONDS to 0 so they need not wait.
CHECKS_POLL_SECONDS = float(os.environ.get("MERGE_WORKER_POLL_SECONDS", 15))
# pde-gh-write forgets a request after four hours, and the same command
# then asks again, so every rerun stays within the first request.
ANSWER_WAIT_SECONDS = 3 * 60 * 60
DISMISSED = "had been dismissed in the Herdr popup"
EXIT_WAITING = 3
RESULTS = {
    EXIT_WAITING: "no answer; the merge request waits in the popup",
    4: "the user declined the merge request",
    6: "the merge was interrupted; check the pull request on GitHub",
}


def merge_pull_request(
    number: int, folder: Path, commands: list[str], remote: str | None
) -> int:
    """Update, check and merge PR number, printing each outcome.

    Stops at the first step that fails, so a conflict or a failed
    check sends no merge request.

    Args:
        number: Pull request number on GitHub.
        folder: Existing folder for ``prep/`` and ``merge.log``.
        commands: Test commands for merge_prep.py.
        remote: Git remote given with ``--remote``, or None for the
            repository's only remote.

    Returns:
        0 when the pull request merged, 1 when it did not, and
        merge_prep.py's code when it failed.

    Raises:
        subprocess.CalledProcessError: When git or gh fails.
    """
    head = update_branch(number, remote or pick_remote())
    if head is None:
        return 1
    prep = folder / "prep"
    prep_args = [str(number), "--out", str(prep), *commands]
    if remote:
        prep_args += ["--remote", remote]
    code = subprocess.run(
        [sys.executable, str(MERGE_PREP), *prep_args]
    ).returncode
    if code != 0:
        return code
    code = send_merge_request(number, head, prep, folder / "merge.log")
    if code != 0:
        result = RESULTS.get(
            code, f"the merge request failed with exit {code}; see merge.log"
        )
        print(f"result: {result}")
        return 1
    commit = view_pr(number, "mergeCommit")["mergeCommit"] or {}
    print(f"merged: {commit.get('oid', 'unknown')}")
    return 0


def update_branch(number: int, remote: str) -> str | None:
    """Push a merge of the base into the PR's branch when it is behind.

    A merge commit keeps the reviewed commits as they are, unlike a
    rebase, and the squash merge flattens it later.

    Args:
        number: Pull request number.
        remote: Git remote that GitHub serves the PR from.

    Returns:
        The head commit to merge, or None after printing a conflict or
        not ready result.

    Raises:
        subprocess.CalledProcessError: When git or gh fails, such as
            a push refused because the branch moved.
    """
    pr = view_pr(number, PR_FIELDS)
    # A fork's branch cannot be pushed from here, so merge_prep.py
    # checks it as it is.
    if pr["isCrossRepository"]:
        return view_pr(number, "headRefOid")["headRefOid"]
    base, branch = pr["baseRefName"], pr["headRefName"]
    run("git", "fetch", "-q", remote, base, branch)
    base_commit = run("git", "rev-parse", f"{remote}/{base}").strip()
    head = run("git", "rev-parse", f"{remote}/{branch}").strip()
    behind = subprocess.run(
        ["git", "merge-base", "--is-ancestor", base_commit, head]
    )
    if behind.returncode == 0:
        return head
    merge = subprocess.run(
        [
            "git",
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            head,
            base_commit,
        ],
        capture_output=True,
        text=True,
    )
    lines = merge.stdout.splitlines()
    if merge.returncode == 1:
        files = ", ".join(lines[1 : FILES_SHOWN + 1])
        print(f"result: conflict with {base}: {files}")
        return None
    if merge.returncode != 0:
        raise subprocess.CalledProcessError(
            merge.returncode, merge.args, merge.stdout, merge.stderr
        )
    commit = run(
        "git",
        "commit-tree",
        lines[0],
        "-p",
        head,
        "-p",
        base_commit,
        "-m",
        f"Merge {base} into {branch}",
    ).strip()
    run("git", "push", "-q", remote, f"{commit}:refs/heads/{branch}")
    print(f"branch: merged {base} into {branch} as {commit[:8]}")
    if not wait_for_checks(number, commit, bool(pr["statusCheckRollup"])):
        return None
    return commit


def wait_for_checks(number: int, head: str, had_checks: bool) -> bool:
    """Wait until GitHub shows head and its checks have finished.

    Gives up after ``CHECKS_WAIT_SECONDS``. Checks still running then
    are left for merge_prep.py to report. Checks that never started
    would leave merge_prep.py nothing to report, so this prints a
    not ready result instead.

    Args:
        number: Pull request number.
        head: Commit just pushed to the PR's branch.
        had_checks: Whether the old head had checks, so the new head
            gets them too.

    Returns:
        False after printing a not ready result, otherwise True.
    """
    started = False
    deadline = time.monotonic() + CHECKS_WAIT_SECONDS
    while time.monotonic() < deadline:
        pr = view_pr(number, "headRefOid,statusCheckRollup")
        rollup = pr["statusCheckRollup"]
        # Right after a push GitHub lists no checks for a short while.
        started = pr["headRefOid"] == head and (bool(rollup) or not had_checks)
        if started and not any(is_pending(check) for check in rollup):
            return True
        time.sleep(CHECKS_POLL_SECONDS)
    if started:
        return True
    print(f"result: not ready: no checks started on {head[:8]}")
    return False


def is_pending(check: dict) -> bool:
    """Return whether one entry of ``statusCheckRollup`` is still running.

    Args:
        check: A check run or commit status from ``gh pr view``.

    Returns:
        True while it has no final result.
    """
    if check["__typename"] == "CheckRun":
        return check["status"] != "COMPLETED"
    return check["state"] in ("PENDING", "EXPECTED")


def send_merge_request(number: int, head: str, prep: Path, log: Path) -> int:
    """Send the squash merge request and wait for the user's answer.

    Reruns the identical command while pde-gh-write exits 3, since a
    rerun reads the answer and never asks twice. GitHub refuses the
    merge when the branch no longer ends at head, so a push after the
    checks cannot merge untested code.

    Args:
        number: Pull request number.
        head: Commit the checks ran on.
        prep: merge_prep.py's folder with ``squash.txt`` and
            ``body.txt``.
        log: File that gets pde-gh-write's output.

    Returns:
        pde-gh-write's exit code, or 3 when the user did not answer in
        ``ANSWER_WAIT_SECONDS`` or dismissed the request.
    """
    title = (prep / "squash.txt").read_text().splitlines()[0]
    subject = f"{title} (#{number})"
    command = [
        "pde-gh-write",
        "pr",
        "merge",
        str(number),
        "--squash",
        "--match-head-commit",
        head,
        "--subject",
        subject,
        "--body-file",
        str(prep / "body.txt"),
    ]
    print(f"merge request: {subject}")
    deadline = time.monotonic() + ANSWER_WAIT_SECONDS
    with log.open("a") as output:
        while True:
            result = subprocess.run(command, capture_output=True, text=True)
            output.write(result.stdout + result.stderr)
            output.flush()
            if result.returncode != EXIT_WAITING:
                return result.returncode
            # Each rerun reopens a dismissed request in the popup.
            if DISMISSED in result.stderr or time.monotonic() > deadline:
                return EXIT_WAITING


def view_pr(number: int, fields: str) -> dict:
    """Return the fields of PR number that ``gh pr view`` gives.

    Args:
        number: Pull request number.
        fields: Comma-separated field names for ``--json``.

    Returns:
        The fields by name.

    Raises:
        subprocess.CalledProcessError: When gh fails.
    """
    return json.loads(run("gh", "pr", "view", str(number), "--json", fields))


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
    """Merge the pull request and print what happened.

    Returns:
        0 when it merged, 1 when it did not, merge_prep.py's code when
        it failed, and 2 when git or gh fails.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("number", type=int, help="pull request number")
    parser.add_argument("folder", type=Path, help="folder for the results")
    parser.add_argument("commands", nargs="+", help="test commands")
    parser.add_argument("--remote", help="git remote of the PR")
    args = parser.parse_args()
    # merge_prep.py writes to the same summary, so lines go out in order.
    sys.stdout.reconfigure(line_buffering=True)
    try:
        return merge_pull_request(
            args.number, args.folder, args.commands, args.remote
        )
    except subprocess.CalledProcessError as error:
        detail = (error.stderr or "").strip().splitlines() or ["no output"]
        print(f"error: {' '.join(error.cmd)}: {detail[-1]}")
        return 2
    except (OSError, ValueError) as error:
        print(f"error: {error}")
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
