"""Behavior tests for pde_route.py, the script that picks agent models.

Each test runs the script as callers do, in a temporary home with a
fake ``opencode`` and a fake ``claude`` first on PATH. The fakes give
the answer a test sets and record each call, so a test can count
model-list refreshes and probes.
"""

import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/model-routing/scripts/pde_route.py"
OPUS_CC = "claude-code/claude-opus-5-5"
OPUS_GH = "github-copilot/claude-opus-5.5"
SONNET_CC = "claude-code/claude-sonnet-5"
SONNET_GH = "github-copilot/claude-sonnet-5"
LUNA_OA = "openai/gpt-6-luna"
LUNA_GO = "opencode-go/gpt-6-luna"
HAIKU_CC = "claude-code/claude-haiku-4-5-20251001"
QWEN_FLASH = "goog/qwen3.8-flash"
QWEN = "goog/qwen3.8"
ALL_NAMES = [
    OPUS_CC,
    OPUS_GH,
    SONNET_CC,
    SONNET_GH,
    LUNA_OA,
    LUNA_GO,
    HAIKU_CC,
    QWEN_FLASH,
    QWEN,
]
WITHOUT_OPUS_CC = [name for name in ALL_NAMES if name != OPUS_CC]
OPUS_HIGH = {
    "providerID": "claude-code",
    "id": "claude-opus-5-5",
    "variant": "high",
}
PICKED = "the picked model"
CHILD = "ses_child"
FAKE = """\
#!{python}
import json, os, sys, time
from pathlib import Path

state = Path(os.environ["FAKE_DIR"])
program = Path(sys.argv[0]).name
args = sys.argv[1:]
calls = state / "calls.jsonl"
earlier = calls.read_text().splitlines() if calls.exists() else []
with calls.open("a") as log:
    log.write(json.dumps([program, *args]) + "\\n")
answers = json.loads((state / "answers.json").read_text())


def fail_as(kind):
    if kind == "sleep":
        time.sleep(30)
    if kind == "exit-1":
        sys.exit(1)


if program == "opencode" and args[0] == "models":
    tries = sum(json.loads(c)[:2] == ["opencode", "models"] for c in earlier)
    reply = answers["models"][min(tries, len(answers["models"]) - 1)]
    if isinstance(reply, str):
        fail_as(reply)
    print("\\n".join(reply))
elif program == "opencode":
    model = args[args.index("-m") + 1].partition("#")[0]
    kind = answers["run"].get(model, "ok")
    fail_as(kind)
    if kind == "error-event":
        # Reply text first, so only the error event fails the probe.
        print(json.dumps({{"type": "text", "part": {{"text": "OK"}}}}))
        print(json.dumps({{"type": "error", "error": {{"name": "limit"}}}}))
    elif kind == "ok":
        print(json.dumps({{"type": "text", "part": {{"text": "OK"}}}}))
    else:
        print(json.dumps({{"type": "step_finish", "part": {{}}}}))
else:
    kind = answers["claude"].get(args[args.index("--model") + 1], "ok")
    fail_as(kind)
    result = "" if kind == "empty" else "OK"
    print(json.dumps({{"is_error": kind == "is-error", "result": result}}))
"""


class Machine:
    """A temporary home with fake ``opencode`` and ``claude`` on PATH."""

    def __init__(self, tmp_path: Path) -> None:
        """Write the fakes and an environment that keeps state in tmp.

        Args:
            tmp_path: Empty folder for the home, the fakes and their
                call record.
        """
        self.home = tmp_path / "home"
        self.fake_dir = tmp_path / "fake"
        bin_dir = tmp_path / "bin"
        for folder in (self.home, self.fake_dir, bin_dir):
            folder.mkdir()
        for program in ("opencode", "claude"):
            fake = bin_dir / program
            fake.write_text(FAKE.format(python=sys.executable))
            fake.chmod(0o755)
        dropped = (
            "XDG_CACHE_HOME",
            "XDG_DATA_HOME",
            "XDG_STATE_HOME",
            "PDE_ROUTE_TABLE",
            "PDE_ROUTE_TIMEOUT",
        )
        self.env = {k: v for k, v in os.environ.items() if k not in dropped}
        self.env |= {
            "HOME": str(self.home),
            "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
            "FAKE_DIR": str(self.fake_dir),
        }
        self.copy_path = self.home / ".cache/pde/opencode-models.txt"
        self.log_path = self.home / ".local/state/pde/route.log"
        self.set_answers()

    def set_answers(
        self,
        models: list[Any] | None = None,
        run: dict[str, str] | None = None,
        claude: dict[str, str] | None = None,
    ) -> None:
        """Set what the fakes answer.

        Args:
            models: One answer per ``opencode models`` call, the last
                repeating. An answer is a list of names, ``exit-1`` or
                ``sleep``. The default lists every table name.
            run: ``opencode run`` answer by model, such as
                ``error-event``, ``no-reply``, ``exit-1`` or
                ``sleep``. Other models reply OK.
            claude: ``claude -p`` answer by alias, such as
                ``is-error``, ``empty``, ``exit-1`` or ``sleep``.
                Other aliases reply OK.
        """
        answers = {
            "models": models if models is not None else [ALL_NAMES],
            "run": run or {},
            "claude": claude or {},
        }
        (self.fake_dir / "answers.json").write_text(json.dumps(answers))

    def write_copy(self, names: list[str], minutes_old: float) -> None:
        """Write the cached model list, dated minutes_old ago."""
        self.copy_path.parent.mkdir(parents=True, exist_ok=True)
        self.copy_path.write_text("".join(f"{name}\n" for name in names))
        written = time.time() - minutes_old * 60
        os.utime(self.copy_path, (written, written))

    def run_script(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Run pde_route.py with args in this home."""
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args],
            env=self.env,
            capture_output=True,
            text=True,
            timeout=90,
        )

    def count_calls(self, program: str, first_arg: str | None = None) -> int:
        """Count calls to a fake, or those with first_arg as first argument."""
        calls_path = self.fake_dir / "calls.jsonl"
        if not calls_path.exists():
            return 0
        calls = [
            json.loads(line) for line in calls_path.read_text().splitlines()
        ]
        return sum(
            call[0] == program
            and (first_arg is None or call[1:2] == [first_arg])
            for call in calls
        )

    def read_log(self) -> list[dict[str, Any]]:
        """Return each line of the pick log."""
        return [
            json.loads(line) for line in self.log_path.read_text().splitlines()
        ]


@pytest.fixture
def machine(tmp_path: Path) -> Machine:
    """A machine whose fakes list every table name and answer OK."""
    return Machine(tmp_path)


def read_printed_model(result: subprocess.CompletedProcess[str]) -> str | None:
    """Return the model a pick printed, or None when it printed nothing."""
    if not result.stdout:
        return None
    return json.loads(result.stdout)["model"]


def run_pick(machine: Machine, kind: str, start: str) -> dict[str, str]:
    """Run one pick that must succeed and return its printed line."""
    result = machine.run_script(kind, "--start", start)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def open_opencode_db(machine: Machine) -> sqlite3.Connection:
    """Open OpenCode's database in the home, with the table the audit reads."""
    path = machine.home / ".local/share/opencode/opencode.db"
    path.parent.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(path)
    db.execute(
        "CREATE TABLE IF NOT EXISTS session_message (id TEXT PRIMARY KEY,"
        " session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER,"
        " time_updated INTEGER, data TEXT)"
    )
    return db


def add_message(
    db: sqlite3.Connection, session_id: str, data: dict[str, Any]
) -> None:
    """Add one assistant message, written now, to a session."""
    now = int(time.time() * 1000)
    number = db.execute("SELECT count(*) FROM session_message").fetchone()[0]
    db.execute(
        "INSERT INTO session_message VALUES (?, ?, 'assistant', ?, ?, ?, ?)",
        (f"msg_{number}", session_id, number, now, now, json.dumps(data)),
    )


def add_subagent_call(
    db: sqlite3.Connection, description: str, model: str | None, resume: bool
) -> None:
    """Add a parent message with one ``subagent`` call for the child.

    Args:
        db: OpenCode's database.
        description: The call's description.
        model: The model the call sets, or None for none.
        resume: Whether the call resumes the child instead of
            starting it.
    """
    call_input: dict[str, Any] = {
        "description": description,
        "prompt": "",
        "agent": "general",
    }
    if model:
        call_input["model"] = model
    if resume:
        call_input["sessionID"] = CHILD
    state = {
        "status": "completed",
        "input": call_input,
        "metadata": {"sessionID": CHILD},
    }
    content = [{"type": "tool", "name": "subagent", "state": state}]
    add_message(db, "ses_parent", {"model": OPUS_HIGH, "content": content})


def record_start(
    machine: Machine,
    start: str,
    description: str,
    model: str | None,
    child_model: Any,
    resume_model: str | None = None,
) -> None:
    """Store one subagent start in the store the start's tool writes.

    Args:
        machine: The home that holds the stores.
        start: ``opencode-subagent`` or ``claude-agent``.
        description: The description the caller gave.
        model: The model the call set, or None.
        child_model: The model the child's reply names, a dict for
            OpenCode or an ID string for Claude Code.
        resume_model: For OpenCode, the model of a later resume of
            the same child, or None for no resume.
    """
    if start == "opencode-subagent":
        with open_opencode_db(machine) as db:
            add_subagent_call(db, description, model, resume=False)
            add_message(db, CHILD, {"model": child_model, "content": []})
            if resume_model:
                add_subagent_call(
                    db, "plan review again", resume_model, resume=True
                )
        return
    folder = machine.home / ".claude/projects/project/session/subagents"
    folder.mkdir(parents=True, exist_ok=True)
    meta: dict[str, Any] = {
        "agentType": "general-purpose",
        "description": description,
    }
    if model:
        meta["model"] = model
    (folder / "agent-a1.meta.json").write_text(json.dumps(meta))
    stamp = datetime.now(timezone.utc).isoformat(timespec="milliseconds")
    reply = {
        "type": "assistant",
        "timestamp": stamp,
        "message": {"model": child_model},
    }
    (folder / "agent-a1.jsonl").write_text(json.dumps(reply) + "\n")


class TestPick:
    """A pick prints the first usable model of a kind and logs it."""

    def test_pick_skips_and_logs_missing_model(self, machine: Machine) -> None:
        """A name this machine lacks is skipped, and the log says why.

        A machine with Copilot and no claude-code must still get an
        Opus model, and the log is how the user sees that billing
        moved to another provider.
        """
        machine.write_copy(WITHOUT_OPUS_CC, minutes_old=20)
        machine.set_answers(models=[WITHOUT_OPUS_CC])

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent"
        )

        assert result.returncode == 0, result.stderr
        printed = json.loads(result.stdout)
        assert printed["model"] == f"{OPUS_GH}#high"
        [line] = machine.read_log()
        assert re.fullmatch(r"rt-[0-9a-f]{6}", line["run_id"])
        assert line["run_id"] == printed["run_id"]
        assert line["skipped"] == [
            {"model": OPUS_CC, "reason": "not on this machine"}
        ]

    @pytest.mark.parametrize(
        "answers",
        [
            pytest.param([ALL_NAMES], id="missing-from-copy"),
            pytest.param([[], ALL_NAMES], id="empty-first-answer"),
        ],
    )
    def test_pick_saves_and_uses_refreshed_list(
        self, machine: Machine, answers: list[Any]
    ) -> None:
        """A stale copy that lacks a name is refreshed before a skip.

        Without the refresh, a model added since the copy was written
        stays hidden. The retry matters because OpenCode lists
        nothing while a folder loads.
        """
        machine.write_copy(WITHOUT_OPUS_CC, minutes_old=20)
        machine.set_answers(models=answers)

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent"
        )

        assert read_printed_model(result) == f"{OPUS_CC}#high"
        assert OPUS_CC in machine.copy_path.read_text().split()

    def test_pick_skips_refresh_when_copy_recent(
        self, machine: Machine
    ) -> None:
        """A copy under 10 minutes old is trusted for a missing name.

        A machine without one provider misses a name on every pick.
        Refreshing each time would make every pick wait on
        ``opencode models``.
        """
        machine.write_copy(WITHOUT_OPUS_CC, minutes_old=5)

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent"
        )

        assert read_printed_model(result) == f"{OPUS_GH}#high"
        assert machine.count_calls("opencode", "models") == 0

    def test_pick_skips_models_caller_marked_failed(
        self, machine: Machine
    ) -> None:
        """``--failed`` with the printed name moves to the next provider.

        Callers pass the name as it was printed, effort included. If
        the suffix kept the name from matching, a limit error would
        bring back the same model.
        """
        machine.write_copy(ALL_NAMES, minutes_old=1)

        result = machine.run_script(
            "plan-review",
            "--start",
            "opencode-subagent",
            "--failed",
            f"{OPUS_CC}#high",
        )

        assert read_printed_model(result) == f"{OPUS_GH}#high"

    def test_pick_fails_listing_each_skipped_model(
        self, machine: Machine
    ) -> None:
        """With no name left, the pick exits 1 and says why each was skipped.

        The caller reports this error to the user, who needs each
        reason to fix the machine or the table.
        """
        machine.set_answers(
            models=[[name for name in ALL_NAMES if name != OPUS_GH]]
        )

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent", "--failed", OPUS_CC
        )

        assert result.returncode == 1
        assert result.stdout == ""
        assert f"{OPUS_CC}: failed" in result.stderr
        assert f"{OPUS_GH}: not on this machine" in result.stderr

    @pytest.mark.parametrize(
        ("kind", "flags", "expected"),
        [
            pytest.param(
                "local-read",
                ["--check-failed", QWEN_FLASH],
                LUNA_OA,
                id="local-kind",
            ),
            pytest.param(
                "code-review",
                ["--check-failed", SONNET_CC, "--failed", OPUS_CC],
                f"{OPUS_GH}#medium",
                id="with-failed-flag",
            ),
        ],
    )
    def test_pick_skips_group_after_check_failed(
        self, machine: Machine, kind: str, flags: list[str], expected: str
    ) -> None:
        """``--check-failed`` skips the rest of the model's group.

        A model of the same class would likely fail the check the
        same way, so retrying in the group doubles a failing job's
        tries.
        """
        machine.write_copy(ALL_NAMES, minutes_old=1)

        result = machine.run_script(
            kind, "--start", "opencode-subagent", *flags
        )

        assert read_printed_model(result) == expected

    def test_unknown_kind_exits_2_listing_kinds(
        self, machine: Machine
    ) -> None:
        """A mistyped kind fails and lists the kinds the table has.

        Printing a default model instead would start the agent on a
        model no one chose for the job.
        """
        result = machine.run_script(
            "plan-reveiw", "--start", "opencode-subagent"
        )

        assert result.returncode == 2
        assert result.stdout == ""
        assert "plan-review" in result.stderr
        assert "local-read" in result.stderr

    @pytest.mark.parametrize(
        ("kind", "start", "expected"),
        [
            pytest.param(
                "plan-review",
                "opencode-subagent",
                {"model": f"{OPUS_CC}#high", "effort": "high"},
                id="opencode-subagent",
            ),
            pytest.param(
                "plan-review",
                "claude-cli",
                {"model": "opus", "effort": "high"},
                id="claude-cli",
            ),
            pytest.param(
                "plan-review",
                "claude-agent",
                {"model": "opus"},
                id="claude-agent",
            ),
            pytest.param(
                "local-read",
                "opencode-subagent",
                {"model": QWEN_FLASH, "effort": "none"},
                id="effort-none",
            ),
            pytest.param(
                "cheap-draft",
                "claude-cli",
                {"model": "haiku"},
                id="claude-cli-effort-none",
            ),
        ],
    )
    def test_pick_formats_effort_per_start_form(
        self, machine: Machine, kind: str, start: str, expected: dict[str, str]
    ) -> None:
        """The effort appears in the form the start's tool takes.

        A wrong suffix fails with no message or runs at the wrong
        level. An effort the ``Agent`` tool cannot take invites the
        caller to guess where to put it.
        """
        printed = run_pick(machine, kind, start)

        del printed["run_id"]
        assert printed == expected

    @pytest.mark.parametrize(
        ("start", "answers", "expected"),
        [
            pytest.param(
                "opencode-cli",
                {"run": {SONNET_CC: "error-event"}},
                f"{SONNET_GH}#medium",
                id="opencode-error-event",
            ),
            pytest.param(
                "opencode-cli",
                {"run": {SONNET_CC: "no-reply"}},
                f"{SONNET_GH}#medium",
                id="opencode-no-reply",
            ),
            pytest.param(
                "opencode-cli",
                {"run": {SONNET_CC: "exit-1"}},
                f"{SONNET_GH}#medium",
                id="opencode-exit-1",
            ),
            pytest.param(
                "claude-cli",
                {"claude": {"sonnet": "is-error"}},
                "opus",
                id="claude-is-error",
            ),
            pytest.param(
                "claude-cli",
                {"claude": {"sonnet": "empty"}},
                "opus",
                id="claude-empty-result",
            ),
            pytest.param(
                "claude-cli",
                {"claude": {"sonnet": "exit-1"}},
                "opus",
                id="claude-exit-1",
            ),
            pytest.param(
                "opencode-cli",
                {"run": {SONNET_CC: "sleep"}},
                f"{SONNET_GH}#medium",
                id="timeout",
            ),
        ],
    )
    def test_cli_pick_skips_model_failing_probe(
        self,
        machine: Machine,
        start: str,
        answers: dict[str, Any],
        expected: str,
    ) -> None:
        """A ``-cli`` pick skips a model whose "reply OK" probe fails.

        A Herdr tab started on a model whose login fails dies out of
        the caller's sight, so the probe is the only check it gets.
        """
        machine.write_copy(ALL_NAMES, minutes_old=1)
        machine.set_answers(**answers)
        machine.env["PDE_ROUTE_TIMEOUT"] = "2"

        result = machine.run_script("code-review", "--start", start)

        assert read_printed_model(result) == expected

    @pytest.mark.parametrize(
        "answer",
        [
            pytest.param([], id="empty-list"),
            pytest.param("exit-1", id="exit-1"),
            pytest.param("sleep", id="timeout"),
        ],
    )
    def test_failed_refresh_keeps_list_with_warning(
        self, machine: Machine, answer: Any
    ) -> None:
        """A failed refresh leaves the old copy in place and in use.

        OpenCode lists nothing while a folder loads. Saving that empty
        answer would fail every later pick on the machine.
        """
        machine.write_copy(WITHOUT_OPUS_CC, minutes_old=20)
        before = machine.copy_path.read_bytes()
        machine.set_answers(models=[answer])
        machine.env["PDE_ROUTE_TIMEOUT"] = "2"

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent"
        )

        assert read_printed_model(result) == f"{OPUS_GH}#high"
        assert machine.copy_path.read_bytes() == before
        assert "warning" in result.stderr

    def test_pick_exits_2_without_model_list(self, machine: Machine) -> None:
        """With no copy and a failing ``opencode``, the pick exits 2.

        Printing a name it could not check would start an agent that
        may fail on a model the machine lacks.
        """
        machine.set_answers(models=["exit-1"])

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent"
        )

        assert result.returncode == 2
        assert result.stdout == ""
        assert "cannot read the model list" in result.stderr
        assert "exited 1" in result.stderr

    @pytest.mark.parametrize("start", ["claude-cli", "claude-agent"])
    def test_pick_exits_2_without_claude_list(
        self, machine: Machine, start: str
    ) -> None:
        """A Claude Code pick on a local kind exits 2 and names opencode-cli.

        A Claude Code session would otherwise start a local job on no
        model or on a cloud model.
        """
        result = machine.run_script("local-read", "--start", start)

        assert result.returncode == 2
        assert result.stdout == ""
        assert "opencode-cli" in result.stderr

    @pytest.mark.parametrize(
        ("start", "model", "code", "expected"),
        [
            pytest.param(
                "opencode-subagent",
                SONNET_GH,
                0,
                f"{SONNET_GH}#high",
                id="listed-name",
            ),
            pytest.param(
                "opencode-subagent", OPUS_CC, 1, None, id="unlisted-name"
            ),
            pytest.param("claude-cli", "haiku", 0, "haiku", id="claude-alias"),
        ],
    )
    def test_user_model_needs_listing_unless_claude(
        self,
        machine: Machine,
        start: str,
        model: str,
        code: int,
        expected: str | None,
    ) -> None:
        """A user's OpenCode name must be listed, and any Claude alias passes.

        A typo in the user's model would otherwise start nothing.
        Nothing lists Claude Code aliases, so checking them would
        refuse every one.
        """
        machine.write_copy(WITHOUT_OPUS_CC, minutes_old=1)

        result = machine.run_script(
            "plan-review", "--start", start, "--model", model
        )

        assert result.returncode == code
        assert read_printed_model(result) == expected

    @pytest.mark.parametrize(
        ("start", "model"),
        [
            pytest.param("opencode-cli", SONNET_GH, id="opencode-cli"),
            pytest.param("claude-cli", "sonnet", id="claude-cli"),
        ],
    )
    def test_user_model_skips_probe_on_cli(
        self, machine: Machine, start: str, model: str
    ) -> None:
        """A user's model is printed without a probe.

        The user chose the model, so a probe would only delay the
        start or reject a model that answers slowly.
        """
        machine.write_copy(ALL_NAMES, minutes_old=1)

        result = machine.run_script(
            "plan-review", "--start", start, "--model", model
        )

        assert result.returncode == 0, result.stderr
        assert machine.count_calls("opencode", "run") == 0
        assert machine.count_calls("claude") == 0


class TestCheck:
    """``check`` compares the table with this machine's model list."""

    def test_check_passes_naming_missing_models(
        self, machine: Machine
    ) -> None:
        """Missing names print with their kinds, and the check still passes.

        Every install runs the check. A machine without claude-code
        is normal, so failing here would warn on each of its
        installs.
        """
        claude_code = [
            name for name in ALL_NAMES if name.startswith("claude-code/")
        ]
        machine.set_answers(
            models=[[n for n in ALL_NAMES if n not in claude_code]]
        )

        result = machine.run_script("check")

        assert result.returncode == 0, result.stdout + result.stderr
        lines = result.stdout.splitlines()
        for name in claude_code:
            [line] = [line for line in lines if name in line]
            assert "plan-review" in line or "cheap-draft" in line

    def test_check_fails_for_kind_with_none(
        self, machine: Machine, tmp_path: Path
    ) -> None:
        """A kind with no listed name fails the check and is named.

        Every pick of that kind would fail, so the install must warn.
        """
        table = {
            "plan-review": {
                "effort": "high",
                "skills": [],
                "opencode": [[OPUS_CC]],
            },
            "code-review": {
                "effort": "medium",
                "skills": [],
                "opencode": [[SONNET_GH]],
            },
        }
        (tmp_path / "table.json").write_text(json.dumps(table))
        machine.env["PDE_ROUTE_TABLE"] = str(tmp_path / "table.json")
        machine.set_answers(models=[[SONNET_GH, OPUS_GH]])

        result = machine.run_script("check")

        assert result.returncode == 1
        assert "kind plan-review" in result.stdout
        assert "kind code-review" not in result.stdout

    def test_check_exits_2_when_list_unavailable(
        self, machine: Machine
    ) -> None:
        """An unreadable list exits 2 and names no model as missing.

        Reporting every model missing would send the user hunting for
        providers that are fine.
        """
        machine.set_answers(models=["exit-1"])

        result = machine.run_script("check")

        assert result.returncode == 2
        assert "cannot read the model list" in result.stderr
        assert "claude-code/" not in result.stdout + result.stderr


class TestAudit:
    """``audit`` pairs subagent starts in the stores with logged picks.

    The log lines come from real picks, so the writer and the reader
    are tested together. The stores hold made-up rows in the layout
    the harnesses use.
    """

    @pytest.mark.parametrize(
        ("kind", "start", "child_model", "resume_model"),
        [
            pytest.param(
                "plan-review",
                "opencode-subagent",
                OPUS_HIGH,
                None,
                id="effort-suffix",
            ),
            pytest.param(
                "plan-review",
                "claude-agent",
                "claude-opus-5-5",
                None,
                id="claude-alias",
            ),
            pytest.param(
                "local-read",
                "opencode-subagent",
                {
                    "providerID": "goog",
                    "id": "qwen3.8-flash",
                    "variant": "default",
                },
                None,
                id="default-variant",
            ),
            pytest.param(
                "plan-review",
                "opencode-subagent",
                OPUS_HIGH,
                f"{OPUS_CC}#high",
                id="resume-same-model",
            ),
        ],
    )
    def test_audit_passes_starts_matching_their_line(
        self,
        machine: Machine,
        kind: str,
        start: str,
        child_model: Any,
        resume_model: str | None,
    ) -> None:
        """A start on its logged model passes, however the store spells it.

        Each store names models its own way. A name rule that misses
        one form would flag every real start, and users would stop
        reading the audit.
        """
        printed = run_pick(machine, kind, start)
        record_start(
            machine,
            start,
            f"{printed['run_id']} review",
            printed["model"],
            child_model,
            resume_model,
        )

        result = machine.run_script("audit", "--since", "1d")

        assert result.returncode == 0, result.stdout + result.stderr
        assert "flagged" not in result.stdout

    @pytest.mark.parametrize(
        ("description", "model", "child_model", "resume_model", "flagged"),
        [
            pytest.param(
                "plan review",
                PICKED,
                OPUS_HIGH,
                None,
                "plan review",
                id="no-run-id",
            ),
            pytest.param(
                "rt-000000 plan review",
                PICKED,
                OPUS_HIGH,
                None,
                "rt-000000",
                id="unknown-run-id",
            ),
            pytest.param(
                "{run_id} plan review",
                f"{OPUS_GH}#high",
                OPUS_HIGH,
                None,
                OPUS_GH,
                id="call-model-differs",
            ),
            pytest.param(
                "{run_id} plan review",
                PICKED,
                {
                    "providerID": "claude-code",
                    "id": "claude-sonnet-5",
                    "variant": "high",
                },
                None,
                SONNET_CC,
                id="child-model-differs",
            ),
            pytest.param(
                "{run_id} plan review",
                PICKED,
                OPUS_HIGH,
                f"{SONNET_CC}#medium",
                "plan review again",
                id="resume-other-model",
            ),
        ],
    )
    def test_audit_flags_unlogged_or_mismatched_subagents(
        self,
        machine: Machine,
        description: str,
        model: str,
        child_model: dict[str, str],
        resume_model: str | None,
        flagged: str,
    ) -> None:
        """A start with no pick, or on another model, is named and fails.

        This is how the user learns that a skill skipped
        ``pde_route.py`` or that an agent ignored the printed model.
        """
        printed = run_pick(machine, "plan-review", "opencode-subagent")
        record_start(
            machine,
            "opencode-subagent",
            description.format(run_id=printed["run_id"]),
            printed["model"] if model == PICKED else model,
            child_model,
            resume_model,
        )

        result = machine.run_script("audit", "--since", "1d")

        assert result.returncode == 1
        [line] = [
            line for line in result.stdout.splitlines() if "flagged" in line
        ]
        assert flagged in line

    @pytest.mark.parametrize(
        ("start", "child_model"),
        [
            pytest.param("opencode-subagent", OPUS_HIGH, id="opencode"),
            pytest.param("claude-agent", "claude-opus-5-5", id="claude-code"),
        ],
    )
    def test_audit_flags_start_without_model(
        self, machine: Machine, start: str, child_model: Any
    ) -> None:
        """A logged start whose call set no model is named and fails.

        Such a start ran on the session's own model, even when that
        happens to match. In Claude Code it is the common mistake.
        """
        printed = run_pick(machine, "plan-review", start)
        record_start(
            machine,
            start,
            f"{printed['run_id']} plan review",
            None,
            child_model,
        )

        result = machine.run_script("audit", "--since", "1d")

        assert result.returncode == 1
        assert "set no model" in result.stdout

    @pytest.mark.parametrize(
        ("start", "code"),
        [
            pytest.param(
                "opencode-subagent", 2, id="subagent-picks-empty-store"
            ),
            pytest.param("opencode-cli", 0, id="cli-picks-only"),
        ],
    )
    def test_audit_exit_2_needs_subagent_picks(
        self, machine: Machine, start: str, code: int
    ) -> None:
        """An empty store fails only when the log has subagent picks for it.

        A store layout change looks like an empty store and would
        pass with nothing checked. A day of only ``-cli`` jobs has no
        starts to find and must not raise that alarm.
        """
        run_pick(machine, "local-read", start)
        open_opencode_db(machine).close()

        result = machine.run_script("audit", "--since", "1d")

        assert result.returncode == code, result.stdout + result.stderr


VALID_ENTRY = {
    "effort": "high",
    "skills": [],
    "opencode": [[OPUS_CC]],
    "claude": ["opus"],
}


class TestTable:
    """Every load checks the table's fields."""

    @pytest.mark.parametrize(
        ("change", "field"),
        [
            pytest.param({"colour": "blue"}, "colour", id="unknown-field"),
            pytest.param(
                {"opencode": [["claude-opus-5-5"]]},
                "opencode",
                id="no-provider",
            ),
            pytest.param({"effort": "max"}, "effort", id="unknown-effort"),
            pytest.param(
                {"opencode": []}, "opencode", id="empty-opencode-list"
            ),
            pytest.param(
                {"opencode": [[OPUS_CC], []]}, "opencode", id="empty-group"
            ),
        ],
    )
    def test_table_rejects_invalid_entries(
        self,
        machine: Machine,
        tmp_path: Path,
        change: dict[str, Any],
        field: str,
    ) -> None:
        """A bad entry exits 2, naming its kind and field.

        A bad table that loaded would pick wrong or crash mid-pick.
        CI runs the same check, so the message must point at the line
        to fix.
        """
        table = {
            "plan-review": VALID_ENTRY,
            "broken-kind": VALID_ENTRY | change,
        }
        (tmp_path / "table.json").write_text(json.dumps(table))
        machine.env["PDE_ROUTE_TABLE"] = str(tmp_path / "table.json")

        result = machine.run_script(
            "plan-review", "--start", "opencode-subagent"
        )

        assert result.returncode == 2
        assert f"kind broken-kind, field {field}" in result.stderr
