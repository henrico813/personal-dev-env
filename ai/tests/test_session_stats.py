"""Behavior tests for the session-review skill's stats script."""

import json
import os
import sqlite3
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/session-review/scripts/session_stats.py"


def make_db(
    home: Path,
    messages: list[tuple[str, dict]],
    subagents: tuple[tuple[str, int], ...] = (("opus", 4),),
    model: str = "opus",
    tokens: tuple[int, int, int, int] = (1, 2, 3, 5),
) -> None:
    """Write a database with session ``ses_main``, its subagents and messages.

    Args:
        home: Folder that stands in for the home folder.
        messages: Type and data of each message of ``ses_main``.
        subagents: Model and cache reads of each subagent, named
            ``ses_sub1``, ``ses_sub2`` and so on.
        model: Model ID of ``ses_main``, under provider ``test``.
        tokens: Uncached input, output, reasoning and cache write tokens
            of every session.
    """
    path = home / ".local/share/opencode/opencode.db"
    path.parent.mkdir(parents=True)
    with sqlite3.connect(path) as db:
        db.execute(
            "create table session_v2 (id, parent_id, title, model,"
            " tokens_input, tokens_output, tokens_reasoning,"
            " tokens_cache_read, tokens_cache_write, time_created,"
            " time_updated)"
        )
        db.execute(
            "create table session_message"
            " (session_id, type, seq, data, time_created)"
        )
        rows = [("ses_main", None, "Main", model, 4)] + [
            (f"ses_sub{n}", "ses_main", f"Sub {n}", model, read)
            for n, (model, read) in enumerate(subagents, 1)
        ]
        db.executemany(
            "insert into session_v2 values (?,?,?,?,?,?,?,?,?,0,60000)",
            [
                (
                    id,
                    parent,
                    title,
                    json.dumps({"id": model, "providerID": "test"}),
                    *tokens[:3],
                    read,
                    tokens[3],
                )
                for id, parent, title, model, read in rows
            ],
        )
        db.executemany(
            "insert into session_message values ('ses_main', ?, ?, ?, 0)",
            [
                (kind, seq, json.dumps(data))
                for seq, (kind, data) in enumerate(messages, 1)
            ],
        )


def make_models(home: Path) -> None:
    """OpenCode's model list with ``opus`` from a cheap reseller and anthropic.

    Both give ``opus`` a 1M window. The reseller charges a thousandth of
    Anthropic's rates.
    """
    path = home / ".cache/opencode/models.json"
    path.parent.mkdir(parents=True)
    window = {"context": 1_000_000}
    price = {"input": 1, "output": 10, "cache_read": 0.1, "cache_write": 2}
    cheap = {key: rate / 1000 for key, rate in price.items()}
    path.write_text(
        json.dumps(
            {
                "cheapco": {
                    "models": {"opus": {"limit": window, "cost": cheap}}
                },
                "anthropic": {
                    "models": {"opus": {"limit": window, "cost": price}}
                },
            }
        )
    )


def make_plan_usage(home: Path, saved: dict) -> None:
    """The plan usage file the OpenCode Claude plugin keeps."""
    path = home / ".local/share/opencode-claude/rate-limit.json"
    path.parent.mkdir(parents=True)
    path.write_text(json.dumps(saved))


def make_utc_ms(*parts: int) -> int:
    """Milliseconds since the epoch of a UTC time, as the plugin saves it."""
    return int(datetime(*parts, tzinfo=timezone.utc).timestamp() * 1000)


def make_step(context: int, *calls: dict) -> tuple[str, dict]:
    """An assistant message that read ``context`` tokens and made calls."""
    tokens = {"input": 0, "cache": {"read": context, "write": 0}}
    return ("assistant", {"tokens": tokens, "content": list(calls)})


def make_call(
    command: str = "", output: str = "", name: str = "shell", path: str = ""
) -> dict:
    """A tool call as the database stores it, after it finished."""
    given = {"command": command, "path": path}
    state = {"input": {key: value for key, value in given.items() if value}}
    state["content"] = [{"type": "text", "text": output}]
    return {"type": "tool", "name": name, "state": state}


def find_line(lines: list[str], opening: str) -> str:
    """The one output line that starts with ``opening``."""
    found = [line for line in lines if line.startswith(opening)]
    assert len(found) == 1, (opening, lines)
    return found[0]


def run_stats(
    home: Path, *args: str, session: str | None = "ses_main"
) -> subprocess.CompletedProcess[str]:
    """Run the script with ``home`` as the home folder.

    Args:
        home: Folder that stands in for the home folder.
        args: Options for the script.
        session: Value of ``OPENCODE_SESSION_ID``, or None to unset it.

    Returns:
        The finished process with captured output.
    """
    env = os.environ | {"HOME": str(home)}
    env.pop("OPENCODE_SESSION_ID", None)
    env.pop("XDG_DATA_HOME", None)
    if session:
        env["OPENCODE_SESSION_ID"] = session
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args],
        env=env,
        capture_output=True,
        text=True,
    )


class TestSessionLookup:
    """Finding the session to report on."""

    @pytest.mark.parametrize(
        "args,session,title",
        [
            pytest.param([], "ses_main", "Main", id="environment"),
            pytest.param(
                ["--session", "ses_sub1"],
                "ses_main",
                "Sub 1",
                id="option-wins",
            ),
        ],
    )
    def test_session_comes_from_option_or_environment(
        self, tmp_path: Path, args: list[str], session: str, title: str
    ) -> None:
        """Report on ``--session`` when given, else on the running session.

        In a subagent the environment holds the subagent's own ID, so a
        parent that runs the review through a subagent must be able to
        name itself, or the review measures the wrong session.
        """
        make_db(tmp_path, [])

        result = run_stats(tmp_path, *args, session=session)

        assert find_line(result.stdout.splitlines(), "session ").endswith(
            f" {title}"
        )

    @pytest.mark.parametrize(
        "make,session,error",
        [
            pytest.param(
                False, "ses_main", "no OpenCode database", id="no-db"
            ),
            pytest.param(True, "ses_gone", "is not in", id="unknown-session"),
            pytest.param(True, None, "no session ID", id="no-session-id"),
        ],
    )
    def test_missing_data_fails_with_reason(
        self, tmp_path: Path, make: bool, session: str | None, error: str
    ) -> None:
        """Exit 1 with a reason when there is nothing to read.

        The skill tells the agent to review from the conversation alone
        when the script fails, so a failure must never look like a
        session with zero tokens.
        """
        if make:
            make_db(tmp_path, [])

        result = run_stats(tmp_path, session=session)

        assert result.returncode == 1
        assert error in result.stderr
        assert result.stdout == ""


class TestReportLines:
    """The numbers the review judges from."""

    def test_zero_steps_count_but_skip_peak(self, tmp_path: Path) -> None:
        """Count zero-token steps but leave the next step out of the peak.

        The claude-code provider logs some steps with zero tokens and
        adds them to the next one, which then shows about twice the real
        context. Counting it would make the review chase a peak that
        never happened, while dropping the step would hide a real model
        call.
        """
        make_db(
            tmp_path,
            [
                make_step(100_000),
                make_step(0),
                make_step(250_000),
                make_step(120_000),
            ],
        )

        lines = run_stats(tmp_path).stdout.splitlines()

        assert find_line(lines, "user messages").endswith(
            "model steps 4, steps logged with zero tokens 1"
        )
        assert find_line(lines, "peak context") == (
            "peak context about 120,000 tokens at seq 4"
        )

    def test_tokens_read_and_compactions_lines(self, tmp_path: Path) -> None:
        """Count cache writes in tokens read, and list each compaction.

        Cache writes are tokens the model read in full, so leaving them
        out understates what a session cost. Compactions mark where a
        session lost its context, which the review needs to explain a
        jump in reads.
        """
        make_db(tmp_path, [make_step(1), ("compaction", {"reason": "auto"})])

        lines = run_stats(tmp_path).stdout.splitlines()

        assert find_line(lines, "tokens read") == (
            "tokens read 10 (cache read 4, cache write 5, uncached 1)"
        )
        assert find_line(lines, "compactions") == "compactions 1 (seq 2 auto)"

    def test_skills_listed_once_from_first_load(self, tmp_path: Path) -> None:
        """List each installed skill once, at the step that first loaded it.

        Agents load skills through the skill tool or by reading
        SKILL.md, and the review needs to know when each skill first
        came in. A name that is not an installed skill, such as another
        file's frontmatter, is left out, and so is a skill tag quoted in
        the middle of other output, such as a test file being read.
        """
        for name in ("create-plan", "python-development", "review-plan"):
            skill = tmp_path / ".agents/skills" / name / "SKILL.md"
            skill.parent.mkdir(parents=True)
            skill.touch()
        make_db(
            tmp_path,
            [
                ("user", {}),
                make_step(
                    1, make_call(output='<skill_content name="create-plan">')
                ),
                make_step(1, make_call(output="2: name: notes-template")),
                make_step(
                    1,
                    make_call(
                        output="x = '<skill_content name=\"review-plan\">'"
                    ),
                ),
                make_step(
                    1, make_call(output="1: ---\n2: name: python-development")
                ),
                make_step(
                    1, make_call(output='<skill_content name="create-plan">')
                ),
            ],
        )

        lines = run_stats(tmp_path).stdout.splitlines()

        assert find_line(lines, "skills loaded") == (
            "skills loaded create-plan (seq 2), python-development (seq 5)"
        )

    def test_subagents_grouped_by_model_and_capped(
        self, tmp_path: Path
    ) -> None:
        """Sum subagents by model and list only the five largest.

        Some sessions start hundreds of subagents, and one line each
        made the output hundreds of lines long, which costs the review
        the tokens the script exists to save.
        """
        opus = tuple(("opus", read) for read in (10, 20, 30, 40, 50, 60))
        make_db(tmp_path, [], subagents=opus + (("qwen", 1000),))

        lines = run_stats(tmp_path).stdout.splitlines()

        start = lines.index("subagents 7")
        end = lines.index("  2 more") + 1
        assert lines[start:end] == [
            "subagents 7",
            "  test/qwen count 1 read 1,006 written 5",
            "  test/opus count 6 read 246 written 30",
            "largest subagents by tokens read",
            "  ses_sub7 test/qwen read 1,006 written 5 Sub 7",
            "  ses_sub6 test/opus read 66 written 5 Sub 6",
            "  ses_sub5 test/opus read 56 written 5 Sub 5",
            "  ses_sub4 test/opus read 46 written 5 Sub 4",
            "  ses_sub3 test/opus read 36 written 5 Sub 3",
            "  2 more",
        ]

    def test_repeats_count_only_identical_commands(
        self, tmp_path: Path
    ) -> None:
        """Count a command as repeated only when its full text matches.

        The review proposes scripts for commands an agent ran again and
        again, so near matches with other arguments must not inflate
        the count.
        """
        make_db(
            tmp_path,
            [
                make_step(1, make_call("git status"), make_call("git status")),
                make_step(1, make_call("git status --short"), make_call("ls")),
            ],
        )

        lines = run_stats(tmp_path).stdout.splitlines()

        assert find_line(lines, "repeated commands") == "repeated commands 1"
        assert "  2x git status" in lines

    def test_carry_by_tool_counts_later_steps(self, tmp_path: Path) -> None:
        """Sum each tool's carry from later steps that logged tokens.

        Carry is what a review ranks outputs by, so it must count only
        steps that re-read the output. A compaction drops the output.
        A step logged with zero tokens does not count, but the step
        after it does. Each 4 characters of output is one token here.
        """
        make_db(
            tmp_path,
            [
                make_step(1, make_call(output="x" * 400)),
                make_step(0),
                make_step(1, make_call(name="read", output="x" * 1200)),
                make_step(1, make_call(output="x" * 40)),
                ("compaction", {"reason": "auto"}),
                make_step(1),
            ],
        )

        lines = run_stats(tmp_path).stdout.splitlines()

        start = lines.index("carry by tool")
        assert lines[start : start + 3] == [
            "carry by tool",
            "  read calls 1 tokens 300 carry 300",
            "  shell calls 2 tokens 110 carry 200",
        ]

    def test_largest_carried_outputs_capped_and_labeled(
        self, tmp_path: Path
    ) -> None:
        """List the five calls with the most carry, each with its label.

        The review proposes cutting the outputs that cost the most, so
        it needs to know which command or file made each one. A long
        list would cost the tokens the script exists to save, so only
        five are shown, and a script command shows its first line.
        """
        make_db(
            tmp_path,
            [
                make_step(1, make_call("pytest\n-q", output="x" * 400)),
                make_step(1, make_call(name="read", path="/plan.md")),
                *(make_step(1, make_call(f"ls {n}")) for n in range(3, 7)),
                make_step(1),
            ],
        )

        lines = run_stats(tmp_path).stdout.splitlines()

        start = lines.index("largest carried outputs")
        assert lines[start:] == [
            "largest carried outputs",
            "  carry 636 tokens 106 later steps 6 seq 1 shell pytest",
            "  carry 25 tokens 5 later steps 5 seq 2 read /plan.md",
            "  carry 16 tokens 4 later steps 4 seq 3 shell ls 3",
            "  carry 12 tokens 4 later steps 3 seq 4 shell ls 4",
            "  carry 8 tokens 4 later steps 2 seq 5 shell ls 5",
        ]


class TestContextLine:
    """The one line ``--context`` prints, which agents hand off from."""

    @pytest.mark.parametrize(
        "steps,expected",
        [
            pytest.param(
                [150_000, 120_000], "120k of 1M (12%)", id="latest-step"
            ),
            pytest.param(
                [100_000, 0, 250_000],
                "100k of 1M (10%)",
                id="skips-step-after-zero",
            ),
        ],
    )
    def test_context_reports_latest_trusted_step_share(
        self, tmp_path: Path, steps: list[int], expected: str
    ) -> None:
        """Show the latest trusted step as a share of the model's window.

        Agents hand off to a new session when this line passes the
        level. Context drops below its peak after a compaction, and the
        doubled step after a zero step must not set off an early
        handoff. The claude-code provider's IDs carry a suffix,
        such as ``opus[1m]``, which must still find the window.
        """
        make_db(tmp_path, [make_step(n) for n in steps], model="opus[1m]")
        make_models(tmp_path)

        result = run_stats(tmp_path, "--context")

        assert result.stdout == f"{expected}\n"

    def test_context_without_models_still_reports_level(
        self, tmp_path: Path
    ) -> None:
        """Print the context and level when OpenCode's model list is missing.

        The list is a cache OpenCode downloads, so a new machine can
        lack it. The handoff level needs only the token count, so the
        command must not fail there.
        """
        make_db(tmp_path, [make_step(250_000)])

        result = run_stats(tmp_path, "--context")

        assert result.stdout == (
            "250k, window unknown, above the 200k handoff level\n"
        )


class TestUsageOption:
    """The lines ``--usage`` prints for the user to read."""

    def test_usage_lists_plan_and_priced_subagents(
        self, tmp_path: Path
    ) -> None:
        """Price each subagent at its maker's rates, and skip unknown prices.

        Plan models store cost 0, so this price is the only cost the
        user sees. Each rate adds its own cents to the $3.90, so a rate
        left out changes it, and a reseller's lower price would too. A
        local model has no price, and a made-up $0.00 would look free.
        """
        make_db(
            tmp_path,
            [make_step(120_000)],
            subagents=(("opus[1m]", 10_000_000), ("qwen", 1000)),
            model="opus[1m]",
            tokens=(1_000_000, 100_000, 50_000, 200_000),
        )
        make_models(tmp_path)

        result = run_stats(tmp_path, "--usage")

        assert result.stdout.splitlines() == [
            "120k of 1M (12%)",
            "claude plan usage unknown",
            "subagents 2, prices at API rates",
            "  test/opus[1m] count 1 read 11.2M wrote 150k $3.90",
            "  test/qwen count 1 read 1.2M wrote 150k",
            "largest subagents",
            "  Sub 1",
            "    test/opus[1m] read 11.2M wrote 150k $3.90",
            "  Sub 2",
            "    test/qwen read 1.2M wrote 150k",
        ]

    @pytest.mark.parametrize(
        "saved,expected",
        [
            pytest.param(
                {
                    "rateLimitType": "seven_day",
                    "utilization": 0.96,
                    "resetsAt": make_utc_ms(2100, 11, 18, 2),
                    "updatedAt": make_utc_ms(2026, 11, 12, 8, 3),
                },
                "claude plan weekly 96% used, resets Nov 18 02:00,"
                " as of Nov 12 08:03",
                id="share-used",
            ),
            pytest.param(
                {
                    "rateLimitType": "five_hour",
                    "utilization": 1.0,
                    "limited": True,
                    "limitedUntil": make_utc_ms(2100, 1, 1),
                },
                "claude plan 5-hour, limit reached until Jan 01 00:00",
                id="limit-reached",
            ),
            pytest.param(
                {
                    "rateLimitType": "seven_day",
                    "utilization": 0.5,
                    "limited": True,
                    "limitedUntil": make_utc_ms(2026, 11, 4, 10),
                    "resetsAt": make_utc_ms(2026, 11, 4, 10),
                },
                "claude plan weekly 50% used",
                id="old-limit-and-reset",
            ),
        ],
    )
    def test_plan_line_shows_share_or_limit(
        self,
        tmp_path: Path,
        monkeypatch: pytest.MonkeyPatch,
        saved: dict,
        expected: str,
    ) -> None:
        """Show the share used, or the limit while it still holds.

        The user checks this line to decide whether to keep using
        Claude models. The plugin keeps a limit's end time and a reset
        time after they pass, and showing them would claim a limit or
        reset that is already over.
        """
        monkeypatch.setenv("TZ", "UTC")
        make_db(tmp_path, [make_step(1)])
        make_plan_usage(tmp_path, saved)

        lines = run_stats(tmp_path, "--usage").stdout.splitlines()

        assert find_line(lines, "claude plan") == expected
