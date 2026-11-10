"""Behavior tests for the hub's merge worker script."""

import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MERGE_WORKER = ROOT / "ai/skills/herdr-hub/scripts/merge-worker.sh"
FAILING_PREP = """\
import sys
print("args:", " ".join(sys.argv[1:]), flush=True)
print("error: gh failed", file=sys.stderr, flush=True)
print("result: not ready: tests")
sys.exit(1)
"""


def install_skills(skills: Path) -> Path:
    """Install the script beside a merge_prep.py whose tests fail.

    Args:
        skills: Empty folder laid out like ``~/.agents/skills``.

    Returns:
        The installed merge-worker.sh.
    """
    script = skills / "herdr-hub/scripts/merge-worker.sh"
    script.parent.mkdir(parents=True)
    shutil.copy(MERGE_WORKER, script)
    prep = skills / "pull-request/scripts/merge_prep.py"
    prep.parent.mkdir(parents=True)
    prep.write_text(FAILING_PREP)
    return script


def test_merge_worker_reports_prep_failure(tmp_path: Path) -> None:
    """A failed merge prep ends its summary with its exit and done.

    The hub trusts only a summary whose last line is ``done``, never
    the local model's words. If a failed run left no ``done`` or no
    exit code, the hub would wait for a result that never comes, or
    a model might rerun the checks it was told to run once. The
    script also exits with merge_prep.py's code, so the shell shows
    the failure, and keeps its error lines so the hub sees why.
    """
    script = install_skills(tmp_path / "skills")
    folder = tmp_path / "job"

    result = subprocess.run(
        ["bash", str(script), "453", str(folder), "make test"],
        capture_output=True,
        text=True,
    )

    assert result.returncode == 1
    assert (folder / "summary.txt").read_text() == (
        f"args: 453 --out {folder}/prep make test\n"
        "error: gh failed\n"
        "result: not ready: tests\n"
        "exit: 1\n"
        "done\n"
    )
