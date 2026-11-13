"""Tests for pde-sweep, run with made-up requests, repos and tools."""

import json
import os
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pytest

# isort: split
import pde_sweep

SCRIPT = Path(__file__).with_name("pde_sweep.py")
NOW = datetime(2026, 11, 12, 18, 0, tzinfo=timezone.utc)
# Noon UTC falls on the same date in every time zone but the farthest.
COMMIT_DATE = "2026-11-01T12:00:00+00:00"
# A fake hub that sees two cards with no session, as after a restart.
FAKE_HUB = """#!/bin/sh
echo '{"no_session": [{"card": "ABC-KB-001", "stage": "active"},
  {"card": "ABC-KB-004", "stage": "active"}]}'
"""
# A fake card with one stuck card, one card with a next step, and two
# review cards, one linked to a merged pull request and one to an open
# one.
FAKE_CARD = """#!/bin/sh
case "$1 $2" in
  "list --stage")
    echo "ABC-KB-002 review   build    Some work"
    echo "ABC-KB-003 review   build    More work" ;;
  "show ABC-KB-001") printf -- '---\\nstage: active\\n---\\n# Work\\n' ;;
  "show ABC-KB-002")
    printf -- '---\\nprs:\\n  - "https://github.com/o/r/pull/7"\\n---\\n' ;;
  "show ABC-KB-003")
    printf -- '---\\nprs:\\n  - "https://github.com/o/r/pull/8"\\n---\\n' ;;
  "show ABC-KB-004")
    printf -- '---\\nstage: active\\n---\\n- note (hub): next: wait\\n' ;;
esac
"""
# A fake gh where pull request 8 is open, every other one merged, and
# no branch has one.
FAKE_GH = """#!/bin/sh
case "$2 $3" in
  "view https://github.com/o/r/pull/8") echo OPEN ;;
  "view "*) echo MERGED ;;
esac
"""


def format_go_time(moment: datetime) -> str:
    """Return a time written as Go writes it, with nanoseconds.

    Args:
        moment: The time to write.

    Returns:
        Text such as ``2026-11-12T10:00:00.123456789-08:00``.
    """
    pacific = moment.astimezone(timezone(timedelta(hours=-8)))
    return pacific.strftime("%Y-%m-%dT%H:%M:%S.123456789-08:00")


def write_executable(folder: Path, name: str, text: str) -> None:
    """Write a fake command into a folder that goes on PATH.

    Args:
        folder: The fake bin folder.
        name: The command name.
        text: The shell script.
    """
    path = folder / name
    path.write_text(text)
    path.chmod(0o755)


def run_git(cwd: Path, *args: str) -> None:
    """Run git in a test checkout, with a fixed author and date.

    Args:
        cwd: The folder to run in.
        *args: The git arguments.
    """
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        cwd=cwd,
        env={**os.environ, "GIT_COMMITTER_DATE": COMMIT_DATE},
        check=True,
        capture_output=True,
    )


def run_sweep(home: Path, bin_folder: Path) -> str:
    """Run pde-sweep as the timer does and return latest.md.

    Args:
        home: The made-up home folder.
        bin_folder: The folder of fake hub, card and gh commands.

    Returns:
        The text of latest.md.
    """
    env = {
        "HOME": str(home),
        "PATH": f"{bin_folder}:/usr/bin:/bin",
    }
    subprocess.run([sys.executable, str(SCRIPT)], env=env, check=True)
    return (home / ".local/state/pde/sweep/latest.md").read_text()


def read_items(report: str, kind: str) -> list[str]:
    """Return the item and note lines under one kind's heading.

    Args:
        report: The text of latest.md.
        kind: A heading, such as ``stuck-cards``.

    Returns:
        The lines after ``count:`` up to the next heading.
    """
    lines = report.splitlines()
    start = lines.index(f"## {kind}") + 2
    end = start
    while end < len(lines) and not lines[end].startswith("## "):
        end += 1
    return lines[start:end]


class TestIsRequestLost:
    """A request is lost when nobody will approve it or run it."""

    @pytest.mark.parametrize(
        ("state", "dismissed", "age_hours", "lost"),
        [
            pytest.param("pending", True, 0.1, True, id="dismissed"),
            pytest.param("pending", False, 5, True, id="expired"),
            pytest.param("pending", False, 1, False, id="waiting"),
            pytest.param("approved", False, 2, True, id="runner-gone"),
            pytest.param("running", False, 0.5, False, id="running"),
            pytest.param("ran", False, 72, False, id="ran"),
        ],
    )
    def test_lost_depends_on_state_and_age(
        self, state: str, dismissed: bool, age_hours: float, lost: bool
    ) -> None:
        """Lost requests are the ones the user must hear about.

        A pull request create sat dismissed for 16 hours after a Herdr
        restart, and nothing said so. Missing one loses work; listing
        a live one sends the user after a request that is fine. The
        times are Go's nanosecond format, which Python 3.10 cannot
        read unchanged.
        """
        record = {
            "state": state,
            "dismissed": dismissed,
            "updated": format_go_time(NOW - timedelta(hours=age_hours)),
        }

        assert pde_sweep.is_request_lost(record, NOW) is lost


class TestClassifyWorktree:
    """Only worktrees with unsaved work get their own line."""

    @pytest.mark.parametrize(
        ("status_text", "finished", "verdict"),
        [
            pytest.param(
                "# branch.head main\n# branch.ab +0 -0\n? notes.txt\n",
                False,
                pde_sweep.WorktreeVerdict.SKIP,
                id="untracked-on-main",
            ),
            pytest.param(
                "# branch.head feat/x\n# branch.ab +0 -0\n? new.py\n",
                False,
                pde_sweep.WorktreeVerdict.LIST,
                id="untracked-on-feature",
            ),
            pytest.param(
                "# branch.head main\n# branch.ab +0 -0\n"
                "1 .M N... 100644 100644 100644 a b app.py\n",
                False,
                pde_sweep.WorktreeVerdict.LIST,
                id="changed-on-main",
            ),
            pytest.param(
                "# branch.head feat/x\n# branch.ab +3 -0\n",
                False,
                pde_sweep.WorktreeVerdict.LIST,
                id="unpushed-commits",
            ),
            pytest.param(
                "# branch.head feat/x\n# branch.ab +3 -0\n",
                True,
                pde_sweep.WorktreeVerdict.FINISHED,
                id="commits-on-merged-branch",
            ),
            pytest.param(
                "# branch.head feat/x\n# branch.ab +3 -0\n"
                "1 .M N... 100644 100644 100644 a b app.py\n",
                True,
                pde_sweep.WorktreeVerdict.LIST,
                id="changed-on-merged-branch",
            ),
            pytest.param(
                "# branch.head feat/x\n# branch.ab +0 -2\n",
                False,
                pde_sweep.WorktreeVerdict.CLEAN,
                id="behind-only",
            ),
        ],
    )
    def test_classify_worktree_lists_only_unsaved_work(
        self,
        status_text: str,
        finished: bool,
        verdict: pde_sweep.WorktreeVerdict,
    ) -> None:
        """Scratch files and finished branches are counted, not listed.

        The first sweep listed 116 worktrees, most of them main
        checkouts with a stray untracked file. The next listed 105,
        most of them squash-merged branches whose commits look
        unpushed. Both buried the real ones. A new untracked file on a
        feature branch, or an edit on a merged one, can be a worker's
        only copy of its work, so it must still show.
        """
        status = pde_sweep.read_worktree_status(status_text)

        result = pde_sweep.classify_worktree(status, ("main",), finished)

        assert result is verdict


class TestFindPushedBranches:
    """Only branches pushed under their own name can lack a PR."""

    def test_skips_default_and_local_upstreams(self) -> None:
        """Branches that track main or a local branch are left out.

        The first sweep listed six such branches as pushed with no pull
        request, though none was ever pushed. Each false line costs the
        user a check by hand.
        """
        branch_refs = "\n".join(
            [
                "feat/a\trefs/remotes/github/feat/a\trefs/heads/feat/a\t9",
                "old\trefs/remotes/github/main\trefs/heads/main\t9",
                "sketch\trefs/heads/main\trefs/heads/main\t9",
                "main\trefs/remotes/github/main\trefs/heads/main\t9",
                "scratch\t\t\t9",
            ]
        )

        pushed = pde_sweep.find_pushed_branches(
            branch_refs, {"refs/remotes/github/main"}
        )

        assert pushed == [pde_sweep.PushedBranch("feat/a", "feat/a", 9)]


class TestMergeNoSessionCards:
    """Cards with no session come from every live hub."""

    def test_lists_each_card_once_across_hubs(self) -> None:
        """Each live hub adds its cards, and a shared card appears once.

        hub status needs --run when two hubs run, so the first sweep
        saw no cards at all. Reading each hub and joining the lists
        keeps a card from going missing or showing twice.
        """
        first = json.dumps(
            {"no_session": [{"card": "B-KB-2", "stage": "review"}]}
        )
        second = json.dumps(
            {
                "no_session": [
                    {"card": "A-KB-1", "stage": "active"},
                    {"card": "B-KB-2", "stage": "review"},
                ]
            }
        )

        cards = pde_sweep.merge_no_session_cards([first, second])

        assert cards == [("A-KB-1", "active"), ("B-KB-2", "review")]


class TestRun:
    """The whole sweep, run as the timer runs it, with fake tools."""

    @pytest.fixture
    def home(self, tmp_path: Path) -> Path:
        """A home folder with a repo, a hub run and one lost request.

        Args:
            tmp_path: pytest's temporary folder.

        Returns:
            The home folder.
        """
        home = tmp_path / "home"
        repo = home / "Projects/app"
        repo.mkdir(parents=True)
        run_git(tmp_path, "init", "--bare", "-b", "main", "remote.git")
        run_git(repo, "init", "-b", "main")
        (repo / "app.py").write_text("print('hi')\n")
        run_git(repo, "add", "app.py")
        run_git(repo, "commit", "-m", "start")
        run_git(repo, "remote", "add", "origin", str(tmp_path / "remote.git"))
        run_git(repo, "push", "-u", "origin", "main")
        (repo / "notes.txt").write_text("scratch\n")
        feature = tmp_path / "worktrees/feat-x"
        run_git(repo, "worktree", "add", "-b", "feat/x", str(feature))
        run_git(feature, "push", "-u", "origin", "feat/x")
        (feature / "app.py").write_text("print('bye')\n")

        state = home / ".local/state"
        requests = state / "pde/pr-write"
        requests.mkdir(parents=True)
        five_hours_ago = datetime.now(timezone.utc) - timedelta(hours=5)
        (requests / "ab5246d8ffff.json").write_text(
            json.dumps(
                {
                    "id": "ab5246d8ffff",
                    "args": ["pr", "create"],
                    "state": "pending",
                    "updated": format_go_time(five_hours_ago),
                    "repo": "o/r",
                    "pr_title": "Add the thing",
                }
            )
        )
        hub_run = state / "herdr-hub.abc123"
        hub_run.mkdir()
        (hub_run / "events.jsonl").write_text("{}\n")
        report = hub_run / "report-worker-1.md"
        report.write_text("done\n")
        two_hours_ago = time.time() - 2 * 3600
        os.utime(report, (two_hours_ago, two_hours_ago))
        return home

    def test_sweep_lists_every_lost_work_kind(
        self, home: Path, tmp_path: Path
    ) -> None:
        """One run finds every kind of lost work and ends with done.

        The report is the only place the user or a hub learns about
        lost work, so each kind must reach it. A reader trusts the
        file only when its last line is done.
        """
        bin_folder = tmp_path / "bin"
        bin_folder.mkdir()
        write_executable(bin_folder, "hub", FAKE_HUB)
        write_executable(bin_folder, "card", FAKE_CARD)
        write_executable(bin_folder, "gh", FAKE_GH)

        report = run_sweep(home, bin_folder)

        assert read_items(report, "lost-requests")[0].startswith(
            "- ab5246d8 pending"
        )
        assert read_items(report, "stuck-cards") == ["- ABC-KB-001 active"]
        assert read_items(report, "review-cards-pr-done") == [
            "- ABC-KB-002 https://github.com/o/r/pull/7 MERGED"
        ]
        assert read_items(report, "dirty-worktrees") == [
            "skipped: 1 on the default branch with only untracked files",
            f"- 2026-11-01 {tmp_path}/worktrees/feat-x feat/x changed=1"
            " untracked=0 ahead=0",
        ]
        assert read_items(report, "branches-no-pr") == [
            "- 2026-11-01 ~/Projects/app feat/x"
        ]
        assert read_items(report, "hub-leftovers") == [
            "- herdr-hub.abc123 report-worker-1.md 2h"
        ]
        assert report.endswith("\ndone\n")

    def test_sweep_skips_cards_still_in_progress(
        self, home: Path, tmp_path: Path
    ) -> None:
        """Cards with an open PR or a next step stay off the report.

        A reader acts on each card the report lists. Listing a card whose
        pull request is still open, or whose next step is written down,
        sends someone to finish or restart work that is not lost.
        """
        bin_folder = tmp_path / "bin"
        bin_folder.mkdir()
        write_executable(bin_folder, "hub", FAKE_HUB)
        write_executable(bin_folder, "card", FAKE_CARD)
        write_executable(bin_folder, "gh", FAKE_GH)

        report = run_sweep(home, bin_folder)

        stuck = "\n".join(read_items(report, "stuck-cards"))
        assert "ABC-KB-004" not in stuck
        done = "\n".join(read_items(report, "review-cards-pr-done"))
        assert "ABC-KB-003" not in done

    def test_sweep_caps_each_kind_at_ten(
        self, home: Path, tmp_path: Path
    ) -> None:
        """A long kind shows its true count but only the 10 newest items.

        The dry run found 105 worktrees, and a report that long buried
        the real ones. The count must still be true, so a reader knows
        how much is hidden.
        """
        bin_folder = tmp_path / "bin"
        bin_folder.mkdir()
        write_executable(bin_folder, "hub", FAKE_HUB)
        write_executable(bin_folder, "card", FAKE_CARD)
        write_executable(bin_folder, "gh", FAKE_GH)
        hub_run = home / ".local/state/herdr-hub.abc123"
        for hours in range(3, 14):
            leftover = hub_run / f"report-worker-{hours}.md"
            leftover.write_text("done\n")
            hours_ago = time.time() - hours * 3600
            os.utime(leftover, (hours_ago, hours_ago))

        report = run_sweep(home, bin_folder)

        lines = report.splitlines()
        assert lines[lines.index("## hub-leftovers") + 1] == "count: 12"
        items = read_items(report, "hub-leftovers")
        assert len(items) == 10
        assert items[0] == "- herdr-hub.abc123 report-worker-1.md 2h"
        assert items[-1] == "- herdr-hub.abc123 report-worker-11.md 11h"

    def test_sweep_prints_report_with_stdout_flag(
        self, home: Path, tmp_path: Path
    ) -> None:
        """With --stdout the report goes to the terminal, not latest.md.

        The manual check runs the sweep with --stdout and reads what it
        prints. If the flag were ignored, that check would show nothing
        and replace the timer's report with a hand run.
        """
        bin_folder = tmp_path / "bin"
        bin_folder.mkdir()
        write_executable(bin_folder, "hub", FAKE_HUB)
        write_executable(bin_folder, "card", FAKE_CARD)
        write_executable(bin_folder, "gh", FAKE_GH)
        env = {"HOME": str(home), "PATH": f"{bin_folder}:/usr/bin:/bin"}

        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--stdout"],
            env=env,
            check=True,
            capture_output=True,
            text=True,
        )

        assert result.stdout.endswith("\ndone\n")
        assert not (home / ".local/state/pde/sweep/latest.md").exists()

    def test_sweep_finishes_when_a_tool_fails(
        self, home: Path, tmp_path: Path
    ) -> None:
        """A failing tool is counted as an error and the sweep goes on.

        The first sweep ran where hub status failed. A sweep that
        stopped there would hide every other kind of lost work.
        """
        bin_folder = tmp_path / "bin"
        bin_folder.mkdir()
        write_executable(bin_folder, "hub", "#!/bin/sh\nexit 1\n")
        write_executable(bin_folder, "card", FAKE_CARD)
        write_executable(bin_folder, "gh", FAKE_GH)

        report = run_sweep(home, bin_folder)

        assert read_items(report, "errors")[0].startswith(
            "- hub status --json --run"
        )
        assert read_items(report, "branches-no-pr") == [
            "- 2026-11-01 ~/Projects/app feat/x"
        ]
        assert report.endswith("\ndone\n")
