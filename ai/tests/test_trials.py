"""Behavior tests for the session-review skill's trials script."""

import datetime
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "ai/skills/session-review/scripts/trials.py"
# Stands in for ``opencode run``. It logs each call except a version call,
# then fails or prints the next answer as OpenCode's JSON events.
FAKE_OPENCODE = f"""#!{sys.executable}
import json, os, pathlib, subprocess, sys, time
if sys.argv[1:] == ["--version"]:
    print("opencode v0.0.1")
    sys.exit()
log = pathlib.Path(os.environ["FAKE_LOG"])
calls = log.read_text().splitlines() if log.exists() else []
names = [
    "OPENCODE_CONFIG_CONTENT",
    "OPENCODE_DISABLE_PROJECT_CONFIG",
    "OPENCODE_CONFIG_PROJECT_DISABLE",
    "OPENCODE_CONFIG",
    "OPENCODE_CONFIG_DIR",
]
call = {{
    "argv": sys.argv[1:],
    "env": {{name: os.environ.get(name) for name in names}},
}}
log.write_text("\\n".join(calls + [json.dumps(call)]))
sleep = os.environ.get("FAKE_SLEEP", "0")
if sleep != "0":
    # A child holds the output open, so killing only the fake would hang.
    subprocess.Popen(["sleep", sleep])
    time.sleep(float(sleep))
if os.environ.get("FAKE_EXIT", "0") != "0":
    print("Error: model not found", file=sys.stderr)
    sys.exit(int(os.environ["FAKE_EXIT"]))
answers = json.loads(os.environ["FAKE_ANSWERS"])
text = answers[len(calls) % len(answers)]
print(json.dumps({{"type": "step_start", "part": {{}}}}))
for line in json.loads(os.environ["FAKE_LINES"]):
    print(line)
print(json.dumps({{"type": "text", "part": {{"type": "text", "text": text}}}}))
"""


def make_trials(home: Path, **fields: object) -> Path:
    """A trials folder with one case, ``stale-ask``, and its rules file.

    Args:
        home: Folder that holds the trials folder and the fake.
        fields: Case fields that replace or add to the defaults.

    Returns:
        The trials folder.
    """
    rules = home / "hub-rules.md"
    rules.write_text("Recheck an approval ask before listing it.\n")
    case = {
        "id": "stale-ask",
        "source": "a hub run's notes.md, line 12",
        "rules": [str(rules)],
        "prompt": "Your last check was six minutes ago. What next?",
        "pass": ["recheck"],
        "fail": [r"approve #\d+ now"],
    } | fields
    cases = home / "trials/cases"
    cases.mkdir(parents=True)
    (cases / "stale-ask.json").write_text(json.dumps(case))
    return home / "trials"


def make_skill_checkout(home: Path) -> Path:
    """A checkout whose skill ``garden`` has one case and an edited SKILL.md.

    The committed SKILL.md says "Water at dusk." and the working tree
    says "Water at dawn.", so HEAD holds the old text.

    Args:
        home: Folder that holds the checkout.

    Returns:
        The checkout.
    """
    repo = home / "repo"
    skill = repo / "ai/skills/garden"
    (skill / "evals").mkdir(parents=True)
    (skill / "SKILL.md").write_text("# Garden\n\nWater at dusk.\n")
    case = {
        "id": "water-time",
        "source": "a made-up correction",
        "rule": "Garden",
        "prompt": "When do you water the beds?",
        "pass": ["dawn"],
    }
    (skill / "evals/water-time.json").write_text(json.dumps(case))
    env = os.environ | {
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_AUTHOR_NAME": "Test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "Test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }
    for command in (["init", "-q"], ["add", "."], ["commit", "-qm", "base"]):
        subprocess.run(["git", *command], cwd=repo, env=env, check=True)
    (skill / "SKILL.md").write_text("# Garden\n\nWater at dawn.\n")
    return repo


def run_trials(
    home: Path,
    *args: str,
    answers: tuple[str, ...] = ("I recheck first.",),
    sleep: float = 0,
    exit_code: int = 0,
    lines: tuple[str, ...] = (),
    cwd: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    """Run the script with a fake ``opencode`` first on PATH.

    The script gets ``OPENCODE_CONFIG`` and ``OPENCODE_CONFIG_DIR`` set,
    and ``OPENCODE_CONFIG_PROJECT_DISABLE`` set to "0", as a user's shell
    may set them, so tests see whether it passes them on.

    Args:
        home: Folder from ``make_trials``.
        args: Options for the script after ``--trials``.
        answers: The fake model's answers, used in turn.
        sleep: Seconds the fake waits before it answers.
        exit_code: The fake's exit code, with an error line, when not 0.
        lines: Raw output lines the fake prints before each answer.
        cwd: Folder to run in, such as a checkout for ``--skill``.

    Returns:
        The finished process with captured output.
    """
    bin = home / "fake-bin"
    bin.mkdir(exist_ok=True)
    fake = bin / "opencode"
    fake.write_text(FAKE_OPENCODE)
    fake.chmod(0o755)
    env = os.environ | {
        "PATH": f"{bin}{os.pathsep}{os.environ['PATH']}",
        "FAKE_LOG": str(home / "calls.jsonl"),
        "FAKE_ANSWERS": json.dumps(answers),
        "FAKE_SLEEP": str(sleep),
        "FAKE_EXIT": str(exit_code),
        "FAKE_LINES": json.dumps(lines),
        "OPENCODE_CONFIG": str(home / "stray.json"),
        "OPENCODE_CONFIG_DIR": str(home / "stray"),
        "OPENCODE_CONFIG_PROJECT_DISABLE": "0",
    }
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--trials", str(home / "trials"), *args],
        env=env,
        capture_output=True,
        text=True,
        cwd=cwd,
    )


def read_calls(home: Path) -> list[dict]:
    """The fake's log of each ``opencode run`` call, oldest first."""
    log = home / "calls.jsonl"
    return [json.loads(line) for line in log.read_text().splitlines()]


def read_results(trials: Path) -> list[dict]:
    """The lines of ``results.jsonl``, oldest first."""
    lines = (trials / "results.jsonl").read_text().splitlines()
    return [json.loads(line) for line in lines]


class TestGrading:
    """Grading each run by the case's answer key."""

    def test_runs_graded_by_case_patterns(self, tmp_path: Path) -> None:
        """Pass a run only when every pass pattern and no fail pattern matches.

        The pass rate over N runs is the review's evidence that a model
        can do a job, so a run must fail both when it misses the
        corrected behavior and when it repeats the logged mistake. A
        model with no tools often writes a pretend tool call instead,
        which must fail even when it holds a pass word. Each result must
        name the model, effort and rules it ran on, or two reviews'
        rates cannot be compared.
        """
        trials = make_trials(tmp_path)

        result = run_trials(
            tmp_path,
            "-n",
            "4",
            "--model",
            "test/model",
            "--variant",
            "high",
            answers=(
                "I Recheck first.",
                "I recheck, then approve #509 now.",
                "I list it.",
                "<tool_call> <function=execute> recheck",
            ),
        )

        results = read_results(trials)
        assert [r["passed"] for r in results] == [True, False, False, False]
        assert results[1]["reasons"] == [r"matched fail /approve #\d+ now/"]
        assert results[2]["reasons"] == ["missed pass /recheck/"]
        assert results[3]["reasons"] == ["tool call instead of answer"]
        assert {(r["model"], r["variant"]) for r in results} == {
            ("test/model", "high")
        }
        assert len({r["rules_hash"] for r in results}) == 1
        assert Path(results[1]["answer"]).read_text() == (
            "I recheck, then approve #509 now."
        )
        assert read_calls(tmp_path)[0]["argv"][:5] == [
            "run",
            "--standalone",
            "-m",
            "test/model#high",
            "--format",
        ]
        assert result.stdout.splitlines()[0] == (
            f"stale-ask 1/4 rules {results[0]['rules_hash']}"
            r" - matched fail /approve #\d+ now/"
        )

    @pytest.mark.parametrize(
        "options,reason",
        [
            pytest.param(
                {"sleep": 30},
                "opencode timed out after 1 s",
                id="timeout",
            ),
            pytest.param(
                {"exit_code": 3},
                "opencode exited 3 Error: model not found",
                id="exit-code",
            ),
        ],
    )
    def test_opencode_failure_counts_as_failed_run(
        self, tmp_path: Path, options: dict, reason: str
    ) -> None:
        """Count a run that hangs or exits nonzero as failed, with why.

        A local model can hang on a long prompt, and OpenCode exits
        nonzero on a bad model name. Without the limit one stuck run
        would stall the whole review, and without the failure a broken
        run would not lower the pass rate or say what went wrong.
        """
        trials = make_trials(tmp_path)

        run_trials(tmp_path, "-n", "1", "--timeout", "1", **options)

        [result] = read_results(trials)
        assert result["passed"] is False
        assert result["reasons"] == [reason]
        assert result["seconds"] < 10

    def test_read_answer_skips_events_without_text(
        self, tmp_path: Path
    ) -> None:
        """Skip output lines that are not a text event with text.

        A batch runs for an hour or more. One output line in a shape the
        script did not expect, such as a JSON list or a text event with
        no text, must not stop every later run.
        """
        trials = make_trials(tmp_path)

        run_trials(
            tmp_path,
            "-n",
            "2",
            lines=("[]", '{"type": "text", "part": {}}'),
        )

        results = read_results(trials)
        assert [r["passed"] for r in results] == [True, True]
        assert Path(results[0]["answer"]).read_text() == "I recheck first."


class TestRules:
    """Reading the rules a case names."""

    def test_hashes_follow_current_rules_and_case(
        self, tmp_path: Path
    ) -> None:
        """Send the current rules, and change each hash with its own file.

        The old quiz copied one rules snapshot unchanged across 13 runs,
        so its results said nothing about the rules in use. Each run
        must read the current file, and the rules and case hashes must
        each show a change to their own file, so a reviewer can tell an
        edited rule from an edited case.
        """
        trials = make_trials(tmp_path)
        run_trials(tmp_path, "-n", "1")
        (tmp_path / "hub-rules.md").write_text(
            "Recheck an approval ask before merging it.\n"
        )
        run_trials(tmp_path, "-n", "1")
        case_file = trials / "cases/stale-ask.json"
        case_file.write_text(case_file.read_text() + "\n")

        run_trials(tmp_path, "-n", "1")

        first, second, third = read_results(trials)
        assert first["rules_hash"] != second["rules_hash"]
        assert second["rules_hash"] == third["rules_hash"]
        assert len(first["case_hash"]) == 12
        assert first["case_hash"] == second["case_hash"]
        assert second["case_hash"] != third["case_hash"]
        assert "before merging" in read_calls(tmp_path)[1]["argv"][-1]

    @pytest.mark.parametrize(
        "fields,error",
        [
            pytest.param(
                {"pass": "recheck"}, "pass must be", id="pass-not-list"
            ),
            pytest.param(
                {"rules": ["/no/such/rules.md"]},
                "missing rules files",
                id="missing-rules",
            ),
            pytest.param(
                {"answer": "x"}, "unknown fields", id="unknown-field"
            ),
            pytest.param({"id": "../../x"}, "id must be", id="id-not-slug"),
            pytest.param({"pass": []}, "pass needs", id="empty-pass"),
        ],
    )
    def test_broken_case_stops_before_any_run(
        self, tmp_path: Path, fields: dict, error: str
    ) -> None:
        """Exit 1 with the reason before the model runs at all.

        A local model run takes minutes. A typo in a hand-written case
        found after an hour of runs wastes that hour, and a misspelled
        field such as ``fial`` would grade every answer without its key.
        An ID such as ``../../x`` would write answers outside the trials
        folder, and an empty ``pass`` list would pass every answer.
        """
        make_trials(tmp_path, **fields)

        result = run_trials(tmp_path)

        assert result.returncode == 1
        assert error in result.stderr
        assert not (tmp_path / "calls.jsonl").exists()


class TestTools:
    """What the model may do during a trial."""

    def test_model_gets_every_tool_denied(self, tmp_path: Path) -> None:
        """Run OpenCode with a config that denies every permission.

        A trial prompt describes real hub actions, such as moving cards
        or merging pull requests, and OpenCode's shell runs with the
        user's full access. With any tool allowed, a model acting out
        the prompt could change real cards or pull requests. A config
        file above the scratch folder, or one the user's variables name,
        could give tools back, so the run must ignore both. The prompt
        must tell the model it has no tools, or it often answers with a
        pretend tool call.
        """
        make_trials(tmp_path)

        run_trials(tmp_path, "-n", "2")

        calls = read_calls(tmp_path)
        assert [call["env"] for call in calls] == [
            {
                "OPENCODE_CONFIG_CONTENT": '{"permission": {"*": "deny"}}',
                "OPENCODE_DISABLE_PROJECT_CONFIG": "1",
                "OPENCODE_CONFIG_PROJECT_DISABLE": "1",
                "OPENCODE_CONFIG": None,
                "OPENCODE_CONFIG_DIR": None,
            }
        ] * 2
        assert "You have no tools." in calls[0]["argv"][-1]


class TestSkillCases:
    """Cases in a skill's ``evals/``, run on new, old and no skill text."""

    def test_each_skill_text_sends_its_own_text(self, tmp_path: Path) -> None:
        """Send the checkout's SKILL.md, the old one, and no skill text.

        The author compares the skill texts' pass counts to judge a skill
        change. If the old skill text got the checkout's text, or the no-skill
        skill text got any skill text, the counts would match for the wrong
        reason and a harmful change would look safe.
        """
        repo = make_skill_checkout(tmp_path)

        result = run_trials(
            tmp_path,
            "-n",
            "1",
            "--skill",
            "garden",
            "--at",
            "HEAD",
            "--without",
            answers=("I water at dawn.",),
            cwd=repo,
        )

        prompts = [call["argv"][-1] for call in read_calls(tmp_path)]
        assert result.returncode == 0, result.stderr
        assert len(prompts) == 3
        assert "Water at dawn." in prompts[0]
        assert "Water at dusk." in prompts[1]
        assert "Water at" not in prompts[2]
        assert "When do you water the beds?" in prompts[2]
        results = read_results(tmp_path / "trials")
        assert [r["skill_text"] for r in results] == ["new", "old", "none"]

    def test_uncommitted_skill_logs_no_commit(self, tmp_path: Path) -> None:
        """Log no commit for a SKILL.md edited since HEAD.

        A result names the commit its text came from. Naming HEAD for
        an edit that HEAD does not hold would send a reader to the
        wrong text.
        """
        repo = make_skill_checkout(tmp_path)
        head = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()

        result = run_trials(
            tmp_path,
            "-n",
            "1",
            "--skill",
            "garden",
            "--at",
            "HEAD",
            answers=("I water at dawn.",),
            cwd=repo,
        )

        assert result.returncode == 0, result.stderr
        results = read_results(tmp_path / "trials")
        assert [r["commit"] for r in results] == [None, head]

    def test_none_reused_only_for_same_inputs(self, tmp_path: Path) -> None:
        """Reuse saved no-skill runs only for the same case, note and model.

        A no-skill run depends only on the case, the no-tools note and
        the model, and reusing it halves the cost of each later
        comparison. Reusing it for another model, after the case's
        prompt or patterns changed, or after the note changed, would
        compare against runs that got another prompt or answer key.
        """
        repo = make_skill_checkout(tmp_path)
        options = ("-n", "2", "--skill", "garden", "--without")
        case_file = repo / "ai/skills/garden/evals/water-time.json"
        trials = tmp_path / "trials"
        log = trials / "results.jsonl"
        run_trials(tmp_path, *options, cwd=repo)

        again = run_trials(tmp_path, *options, cwd=repo)
        run_trials(tmp_path, *options, "--model", "test/other", cwd=repo)
        case_file.write_text(case_file.read_text() + "\n")
        run_trials(tmp_path, *options, cwd=repo)
        # A saved line with another rules hash stands for a changed note.
        saved = [r | {"rules_hash": "0" * 12} for r in read_results(trials)]
        log.write_text("".join(json.dumps(r) + "\n" for r in saved))
        run_trials(tmp_path, *options, cwd=repo)

        skill_texts = [r["skill_text"] for r in read_results(trials)]
        new, none = ["new"] * 2, ["none"] * 2
        assert skill_texts == new + none + new + (new + none) * 3
        assert "none 0/2 saved" in again.stdout

    def test_none_reuse_skips_failed_runs(self, tmp_path: Path) -> None:
        """Run the no-skill skill text again when its saved runs failed to run.

        A timeout or an OpenCode error leaves no answer. Reusing such
        runs would report the case as failing with no skill on every
        later comparison, with no answer behind the count.
        """
        repo = make_skill_checkout(tmp_path)
        options = ("-n", "1", "--skill", "garden", "--without")
        run_trials(tmp_path, *options, exit_code=1, cwd=repo)

        again = run_trials(tmp_path, *options, cwd=repo)

        results = read_results(tmp_path / "trials")
        skill_texts = [r["skill_text"] for r in results]
        assert skill_texts == ["new", "none", "new", "none"]
        assert "saved" not in again.stdout

    @pytest.mark.parametrize(
        "answers,summary",
        [
            pytest.param(
                ("I water at dawn.",),
                "water-time new 1/1 none 1/1"
                " - every run passed, so no difference",
                id="all-pass",
            ),
            pytest.param(
                ("I water at dawn.", "I water at noon."),
                "water-time new 1/1 none 0/1 - none: missed pass /dawn/",
                id="none-fails",
            ),
        ],
    )
    def test_summary_marks_case_passing_every_skill_text(
        self, tmp_path: Path, answers: tuple[str, ...], summary: str
    ) -> None:
        """Mark a case only when every run of every skill text passed.

        A case that passes with no skill shows nothing the skill
        changes, so the author should rewrite or drop it. Marking a
        case that one skill text failed would hide the very difference the
        case exists to show.
        """
        repo = make_skill_checkout(tmp_path)

        result = run_trials(
            tmp_path,
            "-n",
            "1",
            "--skill",
            "garden",
            "--without",
            answers=answers,
            cwd=repo,
        )

        assert result.stdout.splitlines()[0] == summary

    def test_all_skills_runs_every_evals_folder(self, tmp_path: Path) -> None:
        """Run the cases of each skill that has ``evals/``, and skip the rest.

        The weekly timer runs ``--all-skills`` with no one watching. If
        it missed a skill's folder, that skill would look checked while
        no run tested it, and a skill with no cases must not stop the
        other skills' runs.
        """
        repo = make_skill_checkout(tmp_path)
        orchard = repo / "ai/skills/orchard"
        (orchard / "evals").mkdir(parents=True)
        (orchard / "SKILL.md").write_text("# Orchard\n\nPrune in winter.\n")
        case = {
            "id": "prune-time",
            "source": "a made-up correction",
            "rule": "Orchard",
            "prompt": "When do you prune?",
            "pass": ["winter"],
        }
        (orchard / "evals/prune-time.json").write_text(json.dumps(case))
        (repo / "ai/skills/notes").mkdir()
        (repo / "ai/skills/notes/SKILL.md").write_text("# Notes\n")

        result = run_trials(tmp_path, "-n", "1", "--all-skills", cwd=repo)

        assert result.returncode == 0, result.stderr
        cases = [r["case"] for r in read_results(tmp_path / "trials")]
        assert sorted(cases) == ["prune-time", "water-time"]

    def test_last_validated_counts_last_call_only(
        self, tmp_path: Path
    ) -> None:
        """Report each skill's last call, counting only its own text's runs.

        The user reads this line to decide whether a skill still works
        on the model. Counting an older call would mix in runs of text
        or cases that have since changed, and counting no-skill runs
        would credit the skill with answers it did not shape. Every real
        log starts with lines from before skill cases, which have no
        skill or skill text, and they must not break the report.
        """
        repo = make_skill_checkout(tmp_path)
        trials = tmp_path / "trials"
        trials.mkdir()
        old_line = {
            "case": "stale-ask",
            "model": "test/model",
            "variant": None,
            "rules_hash": "4f1c2a9e0b7d",
            "case_hash": "9d8e7f6a5b4c",
            "passed": True,
        }
        (trials / "results.jsonl").write_text(json.dumps(old_line) + "\n")
        options = ("--skill", "garden", "--model", "test/model")
        run_trials(tmp_path, "-n", "1", *options, answers=("noon",), cwd=repo)
        run_trials(
            tmp_path,
            "-n",
            "2",
            *options,
            "--without",
            answers=("I water at dawn.",),
            cwd=repo,
        )

        result = run_trials(tmp_path, "--last-validated")

        results = read_results(trials)[1:]
        new_runs = [r for r in results if r["skill_text"] == "new"]
        rules = new_runs[-1]["rules_hash"]
        today = datetime.date.today().isoformat()
        assert result.stdout.splitlines() == [
            f"garden {today} test/model 2/2 rules {rules}"
        ]

    def test_cut_short_log_line_is_skipped(self, tmp_path: Path) -> None:
        """Read past a saved line cut short, so later runs still run.

        The weekly timer runs unattended, and a run killed while writing
        leaves half a line at the end of ``results.jsonl``. One such line
        must not break every later run.
        """
        repo = make_skill_checkout(tmp_path)
        trials = tmp_path / "trials"
        trials.mkdir()
        old_line = {
            "case": "stale-ask",
            "model": "test/model",
            "variant": None,
            "rules_hash": "4f1c2a9e0b7d",
            "case_hash": "9d8e7f6a5b4c",
            "passed": True,
        }
        (trials / "results.jsonl").write_text(
            json.dumps(old_line) + '\n{"case": "stale'
        )

        result = run_trials(
            tmp_path,
            "--skill",
            "garden",
            "-n",
            "1",
            "--without",
            "--model",
            "test/model",
            answers=("I water at dawn.",),
            cwd=repo,
        )

        lines = (trials / "results.jsonl").read_text().splitlines()
        assert result.returncode == 0, result.stderr
        assert lines[1] == '{"case": "stale'
        runs = [json.loads(line) for line in lines[2:]]
        assert json.loads(lines[0]) == old_line
        assert [run["case"] for run in runs] == ["water-time"] * 2
