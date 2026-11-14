"""Behavior tests for the hub's merge worker script.

Each test runs merge-worker.sh in a clone of a local bare remote, with
fakes for gh, pde-gh-write and merge_prep.py on PATH or beside it.
"""

import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "ai/skills/herdr-hub/scripts"
FAILING_PREP = """\
import sys
print("args:", " ".join(sys.argv[1:]), flush=True)
print("error: gh failed", file=sys.stderr, flush=True)
print("result: not ready: tests")
sys.exit(1)
"""
READY_PREP = """\
import sys
from pathlib import Path
out = Path(sys.argv[sys.argv.index("--out") + 1])
out.mkdir(parents=True)
(out / "squash.txt").write_text("feat: add x\\n\\nAdds x.\\n")
(out / "body.txt").write_text("Adds x.\\n")
print("result: ready")
"""
FAKE_GH = """\
#!/bin/sh
echo "gh $*" >> "$FAKE_LOG"
next_checks() { head -n 1 "$FAKE_CHECKS"; sed -i 1d "$FAKE_CHECKS"; }
case "$*" in
  *mergeCommit*) echo '{"mergeCommit": {"oid": "5eed"}}' ;;
  *headRefOid*)
    head=$(git --git-dir="$FAKE_REMOTE" rev-parse feat)
    printf '{"headRefOid": "%s", "statusCheckRollup": [%s]}' \\
      "$head" "$(next_checks)" ;;
  *) printf '{"baseRefName": "main", "headRefName": "feat",
    "isCrossRepository": false, "statusCheckRollup": [%s]}' \\
      "$(next_checks)" ;;
esac
"""
RUNNING_CHECK = '{"__typename": "CheckRun", "status": "IN_PROGRESS"}'
PASSED_CHECK = '{"__typename": "CheckRun", "status": "COMPLETED"}'
FAKE_PDE_GH_WRITE = """\
#!/bin/sh
echo "pde-gh-write $*" >> "$FAKE_LOG"
codes=$(cat "$FAKE_EXITS")
echo "${codes#* }" > "$FAKE_EXITS"
exit "${codes%% *}"
"""
GIT_ENV = {
    **os.environ,
    "GIT_CONFIG_GLOBAL": os.devnull,
    "GIT_CONFIG_NOSYSTEM": "1",
    "GIT_AUTHOR_NAME": "Test",
    "GIT_AUTHOR_EMAIL": "test@example.com",
    "GIT_COMMITTER_NAME": "Test",
    "GIT_COMMITTER_EMAIL": "test@example.com",
}


@dataclass
class Job:
    """One merge worker run's script, repository, fakes and folder."""

    script: Path
    repo: Path
    remote: Path
    folder: Path
    log: Path
    env: dict[str, str]


def run_git(cwd: Path, *args: str) -> str:
    """Run git in cwd and return its output without the last newline.

    Args:
        cwd: Folder to run git in.
        args: Git's arguments.

    Returns:
        What git printed.
    """
    return subprocess.run(
        ["git", *args],
        cwd=cwd,
        env=GIT_ENV,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()


def make_job(
    tmp_path: Path, prep: str, exits: str = "0", checks: tuple[str, ...] = ()
) -> Job:
    """Set up a merge worker run whose PR branch is up to date.

    The remote's ``main`` has ``shared.txt``, and its branch ``feat``
    changes that file in one commit on top.

    Args:
        tmp_path: Empty folder for the run.
        prep: Source of the fake merge_prep.py.
        exits: pde-gh-write's exit codes, one per call, such as
            ``"3 0"``.
        checks: The check the fake gh lists for each PR view that asks
            for checks, in order. Views past the end list no checks.

    Returns:
        The run, ready for ``run_worker``.
    """
    skills = tmp_path / "skills"
    (skills / "herdr-hub").mkdir(parents=True)
    shutil.copytree(SCRIPTS, skills / "herdr-hub/scripts")
    fake_prep = skills / "pull-request/scripts/merge_prep.py"
    fake_prep.parent.mkdir(parents=True)
    fake_prep.write_text(prep)
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for name, source in (("gh", FAKE_GH), ("pde-gh-write", FAKE_PDE_GH_WRITE)):
        fake = bin_dir / name
        fake.write_text(source)
        fake.chmod(0o755)
    (tmp_path / "exits").write_text(exits)
    (tmp_path / "checks").write_text("".join(f"{c}\n" for c in checks))

    remote = tmp_path / "remote.git"
    repo = tmp_path / "repo"
    run_git(tmp_path, "init", "-q", "--bare", str(remote))
    run_git(tmp_path, "clone", "-q", str(remote), str(repo))
    (repo / "shared.txt").write_text("base\n")
    run_git(repo, "add", "shared.txt")
    run_git(repo, "commit", "-q", "-m", "base")
    run_git(repo, "push", "-q", "origin", "HEAD:main")
    run_git(repo, "checkout", "-q", "-b", "feat")
    (repo / "shared.txt").write_text("feat\n")
    run_git(repo, "commit", "-q", "-a", "-m", "feat")
    run_git(repo, "push", "-q", "origin", "feat")

    return Job(
        script=skills / "herdr-hub/scripts/merge-worker.sh",
        repo=repo,
        remote=remote,
        folder=tmp_path / "job",
        log=tmp_path / "calls.log",
        env={
            **GIT_ENV,
            "PATH": f"{bin_dir}:{os.environ['PATH']}",
            "FAKE_LOG": str(tmp_path / "calls.log"),
            "FAKE_REMOTE": str(remote),
            "FAKE_EXITS": str(tmp_path / "exits"),
            "FAKE_CHECKS": str(tmp_path / "checks"),
            "MERGE_WORKER_POLL_SECONDS": "0",
        },
    )


def push_to_main(job: Job, name: str, text: str) -> str:
    """Push a commit that writes one file to the remote's main.

    Args:
        job: The run whose remote gets the commit.
        name: File the commit writes.
        text: The file's new text.

    Returns:
        The new commit of main.
    """
    run_git(job.repo, "checkout", "-q", "--detach", "origin/main")
    (job.repo / name).write_text(text)
    run_git(job.repo, "add", name)
    run_git(job.repo, "commit", "-q", "-m", f"change {name}")
    run_git(job.repo, "push", "-q", "origin", "HEAD:main")
    return run_git(job.repo, "rev-parse", "HEAD")


def run_worker(job: Job, *options: str) -> subprocess.CompletedProcess:
    """Run merge-worker.sh for PR 453 from the job's repository.

    Args:
        job: The run to start.
        options: Options after the test command, such as
            ``--update``.

    Returns:
        The finished script.
    """
    return subprocess.run(
        [
            "bash",
            str(job.script),
            "453",
            str(job.folder),
            "make test",
            *options,
        ],
        cwd=job.repo,
        env=job.env,
        capture_output=True,
        text=True,
        timeout=60,
    )


def list_merge_requests(job: Job) -> list[str]:
    """Return each pde-gh-write call the fake logged.

    Args:
        job: The finished run.

    Returns:
        One line per call, starting with ``pde-gh-write``.
    """
    if not job.log.exists():
        return []
    lines = job.log.read_text().splitlines()
    return [line for line in lines if line.startswith("pde-gh-write ")]


def test_merge_worker_stops_after_prep_failure(tmp_path: Path) -> None:
    """A failed merge prep sends nothing and ends with exit and done.

    The hub trusts only a summary whose last line is ``done``, never
    the local model's words. If a failed run left no ``done`` or no
    exit code, the hub would wait for a result that never comes. A
    merge request after failed checks would ask the user to approve a
    merge whose tests fail. The script exits with merge_prep.py's
    code and keeps its error lines, so the hub sees why.
    """
    job = make_job(tmp_path, FAILING_PREP)

    result = run_worker(job)

    assert result.returncode == 1
    assert (job.folder / "summary.txt").read_text() == (
        f"args: 453 --out {job.folder}/prep make test\n"
        "error: gh failed\n"
        "result: not ready: tests\n"
        "exit: 1\n"
        "done\n"
    )
    assert list_merge_requests(job) == []


def test_merge_worker_merges_stale_branch(tmp_path: Path) -> None:
    """A branch behind its base gets a merge commit, then merges.

    In a merge order each merge leaves the next PR's branch behind its
    base. A model once spent about a million tokens rebasing one by
    hand. The update must add one commit on top of the reviewed ones
    and push without force, since a reviewed PR is never rewritten.
    The summary must name the merge commit, which the hub checks. The
    request pins that commit, so a later push cannot merge untested code.
    """
    job = make_job(tmp_path, READY_PREP)
    old_head = run_git(job.remote, "rev-parse", "feat")
    new_base = push_to_main(job, "other.txt", "main\n")

    result = run_worker(job)

    assert result.returncode == 0
    new_head = run_git(job.remote, "rev-parse", "feat")
    assert run_git(job.remote, "rev-parse", "feat^1") == old_head
    assert run_git(job.remote, "rev-parse", "feat^2") == new_base
    assert list_merge_requests(job) == [
        f"pde-gh-write pr merge 453 --squash --match-head-commit {new_head}"
        f" --subject feat: add x (#453) --body-file {job.folder}/prep/body.txt"
    ]
    assert (job.folder / "summary.txt").read_text() == (
        f"branch: merged main into feat as {new_head[:8]}\n"
        "result: ready\n"
        "merge request: feat: add x (#453)\n"
        "merged: 5eed\n"
        "exit: 0\n"
        "done\n"
    )


def test_merge_worker_waits_for_new_checks(tmp_path: Path) -> None:
    """After the push the merge request waits for the new checks to pass.

    Pushing the merge commit starts the checks again, and merge_prep.py
    reads them only once. Without the wait it would find them running
    and stop as not ready, so every stale branch would go back to the
    user, or a merge could go out before its checks passed.
    """
    # The old head has a passed check; the new head's runs, then passes.
    job = make_job(
        tmp_path,
        READY_PREP,
        checks=(PASSED_CHECK, RUNNING_CHECK, PASSED_CHECK),
    )
    push_to_main(job, "other.txt", "main\n")

    result = run_worker(job)

    assert result.returncode == 0
    calls = job.log.read_text().splitlines()
    polls_and_requests = [
        "poll" if "headRefOid" in call else "merge request"
        for call in calls
        if "headRefOid" in call or call.startswith("pde-gh-write ")
    ]
    assert polls_and_requests == ["poll", "poll", "merge request"]


def test_merge_worker_stops_on_conflict(tmp_path: Path) -> None:
    """A conflict with the base pushes nothing, sends nothing, names files.

    A local model cannot resolve a conflict, so the hub hands it to a
    cloud model by the ``result: conflict`` line and its file names. A
    push or merge request at this point would put a broken branch in
    front of the user.
    """
    job = make_job(tmp_path, READY_PREP)
    old_head = run_git(job.remote, "rev-parse", "feat")
    push_to_main(job, "shared.txt", "main\n")

    result = run_worker(job)

    assert result.returncode == 1
    assert (job.folder / "summary.txt").read_text() == (
        "result: conflict with main: shared.txt\nexit: 1\ndone\n"
    )
    assert run_git(job.remote, "rev-parse", "feat") == old_head
    assert list_merge_requests(job) == []


def test_merge_worker_waits_through_unanswered_request(tmp_path: Path) -> None:
    """pde-gh-write's exit 3 means no answer yet, so the script asks again.

    The user may take hours to approve a merge, and pde-gh-write gives
    up after 90 seconds with exit 3. Only the identical command reads
    the answer later. A script that stopped at exit 3 would never
    record the merge, and one that changed the command would ask the
    user a second time.
    """
    job = make_job(tmp_path, READY_PREP, exits="3 3 0")

    result = run_worker(job)

    assert result.returncode == 0
    requests = list_merge_requests(job)
    assert len(requests) == 3
    assert len(set(requests)) == 1
    summary = (job.folder / "summary.txt").read_text()
    assert summary.endswith("merged: 5eed\nexit: 0\ndone\n")


@pytest.mark.parametrize(
    "options",
    [
        pytest.param((), id="plain"),
        pytest.param(("--update",), id="update"),
    ],
)
def test_merge_worker_stops_when_user_declines(
    tmp_path: Path, options: tuple[str, ...]
) -> None:
    """pde-gh-write's exit 4 means the user said no, so the script stops.

    A rerun of a declined request reads the same refusal again and
    again with no pause, and the hub must hear that the user said no
    instead of waiting on a loop. With --update, a declined merge must
    not install either, since main does not hold the pull request.
    """
    job = make_job(tmp_path, READY_PREP, exits="4 0")
    write_fake_installer(tmp_path, "update installed: main is at 5eed", 0)

    result = run_worker(job, *options)

    assert result.returncode == 1
    assert len(list_merge_requests(job)) == 1
    assert "pde-installer update" not in job.log.read_text()
    summary = (job.folder / "summary.txt").read_text()
    assert "merged:" not in summary
    assert summary.endswith(
        "result: the user declined the merge request\nexit: 1\ndone\n"
    )


def write_fake_installer(tmp_path: Path, last_line: str, code: int) -> None:
    """Put a pde-installer on PATH that logs its call and prints a result.

    Args:
        tmp_path: The run's folder, whose ``bin`` comes first on PATH.
        last_line: The line the fake prints last.
        code: The fake's exit code.
    """
    fake = tmp_path / "bin/pde-installer"
    fake.write_text(
        "#!/bin/sh\n"
        'echo "pde-installer $*" >> "$FAKE_LOG"\n'
        "echo installing\n"
        f"echo '{last_line}'\n"
        f"exit {code}\n"
    )
    fake.chmod(0o755)


@pytest.mark.parametrize(
    ("update_line", "update_exit", "code", "last_line"),
    [
        pytest.param(
            "update installed: main is at 5eed",
            0,
            0,
            "update installed: main is at 5eed",
            id="installed",
        ),
        pytest.param(
            "update skipped: the checkout is on feat, not main",
            1,
            1,
            "update: failed with exit 1: update skipped: the checkout is"
            " on feat, not main; see update.log",
            id="skipped",
        ),
    ],
)
def test_merge_worker_updates_after_merge(
    tmp_path: Path,
    update_line: str,
    update_exit: int,
    code: int,
    last_line: str,
) -> None:
    """With --update, the same run updates once the PR has merged.

    The hub once started separate cloud sessions to install after a
    local merge, which cost over a million tokens on one day. The hub
    trusts only the summary, so it must show the merge and the
    update's result, and the exit code must show an update that left
    the newest main uninstalled.
    """
    job = make_job(tmp_path, READY_PREP)
    write_fake_installer(tmp_path, update_line, update_exit)

    result = run_worker(job, "--update")

    assert result.returncode == code
    assert "pde-installer update" in job.log.read_text().splitlines()
    assert (job.folder / "update.log").read_text() == (
        f"installing\n{update_line}\n"
    )
    summary = (job.folder / "summary.txt").read_text()
    assert summary.endswith(f"merged: 5eed\n{last_line}\nexit: {code}\ndone\n")
