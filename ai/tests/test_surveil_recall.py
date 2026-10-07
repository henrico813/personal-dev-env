"""Measure navigation recall and agent search cost."""

from __future__ import annotations

import collections
import json
import os
import re
import shutil
import statistics
import subprocess
import time
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[2]
GOLDEN = ROOT / "ai/tests/surveil_golden_questions.json"
ARMS = ("grep", "grep+skill", "surveil", "repomix", "probe", "aider-map")
MODEL_TIMEOUT_SECONDS = 300
RESULTS_ENV_VAR = "PDE_NAVIGATION_RESULTS"
MODEL_ENV_VAR = "PDE_EVAL_MODEL"
QUESTION_ALIASES = {
    "Which planner files report missing base commits?": "Which planner files reference the missing-base-commit error or its constant, including tests?",
}


def load_questions() -> list[dict[str, Any]]:
    """Load the pinned evaluation questions."""
    return json.loads(GOLDEN.read_text(encoding="utf-8"))["questions"]


def source_root() -> Path:
    """Return the Git repository containing pinned commits."""
    return Path(os.environ.get("PDE_REPO_ROOT", ROOT))


def ensure_history() -> None:
    """Require pinned history in CI and skip it in exported trees."""
    commit = str(load_questions()[0]["commit"])
    available = subprocess.run(
        ["git", "-C", str(source_root()), "cat-file", "-e", commit],
        capture_output=True,
    ).returncode == 0
    if not available:
        message = "pinned commits are unavailable in the evaluation repository"
        if os.environ.get("CI"):
            raise AssertionError(message)
        pytest.skip(message)


def build_binary(tmp_path: Path) -> Path:
    """Build and return the repository's Surveil binary."""
    subprocess.run(
        ["cargo", "build", "--quiet", "--manifest-path", str(ROOT / "surveil/Cargo.toml")],
        check=True,
    )
    binary = tmp_path / "surveil"
    shutil.copy2(ROOT / "surveil/target/debug/surveil", binary)
    return binary


def materialize(commit: str, destination: Path) -> None:
    """Extract one pinned commit into a temporary project."""
    archive = subprocess.run(
        ["git", "archive", commit], cwd=source_root(), check=True, capture_output=True
    )
    subprocess.run(["tar", "-x", "-C", str(destination)], input=archive.stdout, check=True)


def write_task(task_path: Path, question: dict[str, Any]) -> None:
    """Write a Surveil task outside the searched project."""
    task_path.write_text(json.dumps({
        "summary": question["question"],
        "explicit_files": [],
        "search_areas": ["."],
        "query": [question["question"]],
        "terms": question["terms"],
    }), encoding="utf-8")


def run_search(binary: Path, repo: Path, task_dir: Path, question: dict[str, Any], indexed: bool) -> set[str]:
    """Run Surveil with artifacts stored outside its search area."""
    task_dir.mkdir()
    task = task_dir / "task.json"
    context = task_dir / "context.json"
    trace = task_dir / "trace.json"
    write_task(task, question)
    if indexed:
        subprocess.run([str(binary), "index", "--repo", str(repo)], check=True)
    gathered = subprocess.run(
        [str(binary), "gather", "--repo", str(repo), "--task-file", str(task)],
        check=True, capture_output=True, text=True,
    )
    context.write_text(gathered.stdout, encoding="utf-8")
    report = subprocess.run(
        [str(binary), "research", "--context", str(context), "--trace-out", str(trace)],
        check=True, capture_output=True, text=True,
    )
    value = json.loads(report.stdout)
    return {finding["path"] for answer in value["result"] for finding in answer["findings"]}


def test_golden_recall_is_index_independent(tmp_path: Path) -> None:
    """Ensure ranking never hides complete scoped answers.

    Each expected set comes from `git grep -l` at the pinned commit. Keeping
    artifacts outside the project prevents the test from creating matches.
    """
    ensure_history()
    binary = build_binary(tmp_path)
    literal_questions = [question for question in load_questions() if question["kind"] == "literal"]
    for number, question in enumerate(literal_questions):
        plain = tmp_path / f"plain-{number}"
        indexed = tmp_path / f"indexed-{number}"
        plain.mkdir()
        indexed.mkdir()
        materialize(str(question["commit"]), plain)
        materialize(str(question["commit"]), indexed)
        expected = set(question["expected"])
        assert run_search(binary, plain, tmp_path / f"plain-task-{number}", question, False) == expected
        assert run_search(binary, indexed, tmp_path / f"indexed-task-{number}", question, True) == expected


def walk_values(value: Any) -> list[dict[str, Any]]:
    """Collect nested JSON objects for usage and event inspection."""
    if isinstance(value, dict):
        result = [value]
        for child in value.values():
            result.extend(walk_values(child))
        return result
    if isinstance(value, list):
        return [item for child in value for item in walk_values(child)]
    return []


def parse_events(output: str) -> list[dict[str, Any]]:
    """Parse JSON event lines emitted by OpenCode."""
    return [json.loads(line) for line in output.splitlines() if line.startswith("{")]


def tool_counts(events: list[dict[str, Any]]) -> dict[str, int]:
    """Count tool-use events by reported tool name."""
    counts: collections.Counter[str] = collections.Counter()
    for event in events:
        if event.get("type") != "tool_use":
            continue
        for item in walk_values(event):
            if isinstance(item.get("tool"), str):
                counts[item["tool"]] += 1
    return dict(sorted(counts.items()))


def tool_call_inputs(events: list[dict[str, Any]]) -> list[dict[str, str]]:
    """Save bounded tool names and inputs for later arm verification."""
    calls = []
    for event in events:
        if event.get("type") != "tool_use":
            continue
        for item in walk_values(event):
            tool = item.get("tool")
            if not isinstance(tool, str):
                continue
            value = item.get("input", item.get("arguments", item.get("args", "")))
            calls.append({"tool": tool, "input": str(value)[:2000]})
    return calls


def token_total(events: list[dict[str, Any]]) -> int:
    """Sum token fields reported in usage objects."""
    total = 0
    for item in walk_values(events):
        for key, value in item.items():
            if isinstance(value, (int, float)) and key.lower() in {"input", "output", "reasoning", "cache_read", "cache_write"}:
                total += int(value)
    return total


def final_text(events: list[dict[str, Any]]) -> str:
    """Return the last text response from OpenCode events."""
    texts = [item["text"] for item in walk_values(events) if isinstance(item.get("text"), str)]
    return texts[-1] if texts else ""


def listed_paths(text: str, repo: Path) -> set[str]:
    """Normalize relative and absolute paths listed by the model."""
    candidates = re.findall(r"(?<![A-Za-z0-9_.-])(?:/[^\s`]+|(?:[A-Za-z0-9_.-]+/)*[A-Za-z0-9_.-]+(?:\.[A-Za-z0-9_.-]+)?)", text)
    root = repo.resolve()
    paths = {path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file()}
    normalized = set()
    for candidate in candidates:
        if candidate.startswith("/"):
            path = Path(candidate).resolve()
            try:
                candidate = path.relative_to(root).as_posix()
            except ValueError:
                continue
        else:
            candidate = candidate.removeprefix("./")
        if candidate in paths:
            normalized.add(candidate)
    return normalized


def test_listed_paths_normalizes_locations(tmp_path: Path) -> None:
    """Normalize every supported path representation.

    Absolute model output must score like relative output, while a path outside
    the project must never become a finding.
    """
    path = tmp_path / "src/result.txt"
    path.parent.mkdir()
    path.write_text("result\n", encoding="utf-8")
    text = f"{path} ./src/result.txt src/result.txt /outside/result.txt"
    assert listed_paths(text, tmp_path) == {"src/result.txt"}


def run_opencode(prompt: str, cwd: Path, binary: Path) -> list[dict[str, Any]] | None:
    """Run OpenCode, returning ``None`` when the model call times out."""
    try:
        result = subprocess.run(
            ["opencode", "run", "--standalone", "--auto", "--model", os.environ.get(MODEL_ENV_VAR, "goog/qwen3.8"), "--format", "json", prompt],
            cwd=cwd, env={**os.environ, "PATH": f"{binary.parent}:{os.environ['PATH']}", "PWD": str(cwd)},
            capture_output=True, text=True, timeout=MODEL_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return None
    if result.returncode != 0:
        raise AssertionError(f"OpenCode failed: {result.stderr}")
    return parse_events(result.stdout)


def configured_tool(name: str) -> Path | None:
    """Return a scratch override or PATH executable for an evaluation tool."""
    override = os.environ.get(f"PDE_{name.upper().replace('-', '_')}")
    candidate = Path(override) if override else shutil.which(name)
    return Path(candidate) if candidate else None


def tool_version(name: str, path: Path) -> str:
    """Read a pinned tool version without contacting a model service."""
    args = ["--version"]
    result = subprocess.run([str(path), *args], capture_output=True, text=True)
    output = (result.stdout or result.stderr).strip()
    if name == "aider":
        return output.splitlines()[0] if output else "unknown"
    return output.splitlines()[0] if output else "unknown"


def preflight_tool(name: str, path: Path, repo: Path) -> dict[str, Any]:
    """Run a neutral map or outline command and record elapsed time."""
    if name == "repomix":
        candidates = subprocess.run(["rg", "-l", "", "."], cwd=repo, check=True, capture_output=True, text=True).stdout.splitlines()
        command = [str(path), "--compress", "--include", ",".join(candidates), "--stdout", "."]
    elif name == "probe":
        command = [str(path), "search", "the question", str(repo), "--max-tokens", "1024"]
    else:
        command = [str(path), "--no-check-update", "--no-check-model-accepts-settings", "--model", "gpt-4o-mini", "--show-repo-map", "--map-tokens", "1024"]
    started = time.monotonic()
    result = subprocess.run(command, cwd=repo, capture_output=True, text=True, timeout=120)
    elapsed = time.monotonic() - started
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or f"{name} exited {result.returncode}")
    return {"command": command, "seconds": round(elapsed, 3), "version": tool_version(name, path), "output": result.stdout.splitlines()[:30]}


def arm_status(mode: str, repo: Path) -> dict[str, Any] | None:
    """Return tool metadata or skip a tool-dependent arm clearly."""
    tools = {"repomix": "repomix", "probe": "probe", "aider-map": "aider"}
    if mode not in tools:
        return {"version": mode, "seconds": 0}
    name = tools[mode]
    path = configured_tool(name)
    if path is None:
        return {"skip": f"{name} is not installed"}
    try:
        return preflight_tool(name, path, repo)
    except (OSError, subprocess.SubprocessError, RuntimeError) as error:
        return {"skip": f"{name} preflight failed: {error}"}


def arm_instruction(mode: str, binary: Path, repo: Path, task: Path, context: Path, trace: Path) -> str:
    """Build a tool-only instruction before the shared question is appended."""
    if mode == "surveil":
        return (
            f"Use only {binary}. Write a task JSON to {task} with summary and query "
            "filled from the question at the end of this prompt, explicit_files "
            "empty, search_areas set to ['.'], and terms empty. Then run exactly: "
            f"{binary} index --repo {repo}; {binary} gather --repo {repo} "
            f"--task-file {task} > {context}; then {binary} research "
            f"--context {context} --trace-out {trace}."
        )
    if mode == "grep":
        return "Use only grep -r from the current project directory to find relevant files. Do not use git grep."
    if mode == "grep+skill":
        return "Follow the code-search skill. Start with rg -l --sort path ., then read relevant ranges and report paths."
    if mode == "repomix":
        path = configured_tool("repomix") or Path("repomix")
        return f"Run rg -l --sort path .; then run {path} --compress --include <the comma-separated candidate files> --stdout .; read only needed ranges afterward."
    if mode == "probe":
        path = configured_tool("probe") or Path("probe")
        return f'Run {path} search "<your query>" . --max-tokens 1024, then use {path} extract file:line for only the needed ranges.'
    path = configured_tool("aider") or Path("aider")
    return f"Run {path} --no-check-update --no-check-model-accepts-settings --model gpt-4o-mini --show-repo-map --map-tokens 1024. Use the printed repository map up front, then choose searches and report paths."


def assigned_tool_used(mode: str, tool_inputs: list[dict[str, str]] | None, tool_counts_value: dict[str, int] | None = None) -> bool | None:
    """Check whether saved inputs show the arm's assigned search tool."""
    if tool_inputs is None:
        if mode in {"surveil", "repomix", "probe"} and tool_counts_value and "grep" in tool_counts_value:
            return False
        return None
    text = json.dumps(tool_inputs).lower()
    if mode == "grep":
        return "grep" in text
    if mode == "grep+skill":
        return "grep" in text or "rg" in text
    if mode == "surveil":
        return "surveil" in text
    if mode == "repomix":
        return "repomix" in text
    if mode == "probe":
        return "probe search" in text or "probe extract" in text
    return "aider" in text and "--show-repo-map" in text


def model_name() -> str:
    """Return the configured evaluation model name."""
    return os.environ.get(MODEL_ENV_VAR, "goog/qwen3.8")


def timed_out_result(question: dict[str, Any], status: dict[str, Any] | None, mode: str) -> dict[str, Any]:
    """Score a timed-out model call without contributing cost metrics."""
    return {
        "tool": status,
        "model": model_name(),
        "timed_out": True,
        "used_assigned_tool": None,
        "tool_call_inputs": [],
        "found": [],
        "missed": sorted(question["expected"]),
        "extra": [],
        "tool_calls": {},
        "tokens": None,
    }


def run_agent(binary: Path, repo: Path, task_dir: Path, question: dict[str, Any], mode: str) -> dict[str, Any]:
    """Run one explicit search instruction and score its final answer."""
    task_dir.mkdir()
    task = task_dir / "task.json"
    context = task_dir / "context.json"
    trace = task_dir / "trace.json"
    if mode != "surveil":
        task.unlink(missing_ok=True)
    status = arm_status(mode, repo)
    if status and status.get("skip"):
        return {"skip": status["skip"], "model": model_name(), "used_assigned_tool": None}
    instruction = arm_instruction(mode, binary, repo, task, context, trace)
    prompt = f"{instruction} List every matching path for: {question['question']}"
    events = run_opencode(prompt, repo, binary)
    if events is None:
        return timed_out_result(question, status, mode)
    counts = tool_counts(events)
    inputs = tool_call_inputs(events)
    expected = set(question["expected"])
    found = listed_paths(final_text(events), repo)
    return {
        "tool": status,
        "model": model_name(),
        "used_assigned_tool": assigned_tool_used(mode, inputs, counts),
        "tool_call_inputs": inputs,
        "found": sorted(found & expected),
        "missed": sorted(expected - found),
        "extra": sorted(found - expected),
        "tool_calls": counts,
        "tokens": token_total(events),
    }


def median_tool_count(runs: list[dict[str, Any]]) -> int | None:
    """Return the median tool calls from completed runs only."""
    usable = [run for run in runs if not run.get("timed_out") and "tool_calls" in run]
    return int(statistics.median(sum(run["tool_calls"].values()) for run in usable)) if usable else None


def append_result(path: Path, result: dict[str, Any]) -> None:
    """Append and flush one scored run so partial evaluations survive."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps(result) + "\n")
        stream.flush()


def load_result_records(path: Path) -> list[dict[str, Any]]:
    """Load scored runs from the durable JSONL results file."""
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def normalize_record(record: dict[str, Any]) -> dict[str, Any]:
    """Normalize legacy question text and infer unknown old tool use."""
    value = dict(record)
    value["question"] = QUESTION_ALIASES.get(value["question"], value["question"])
    if "used_assigned_tool" not in value:
        value["used_assigned_tool"] = assigned_tool_used(value["mode"], value.get("tool_call_inputs"), value.get("tool_calls"))
    return value


def tool_use_summary(runs: list[dict[str, Any]]) -> str:
    """Show assigned-tool true, false, and unknown counts."""
    true_count = sum(run.get("used_assigned_tool") is True for run in runs)
    false_count = sum(run.get("used_assigned_tool") is False for run in runs)
    unknown_count = len(runs) - true_count - false_count
    return f"{true_count}/{false_count}/{unknown_count}"


def run_precision(run: dict[str, Any]) -> float:
    """Return found-path precision, including zero for empty output."""
    found = len(run.get("found", []))
    extra = len(run.get("extra", []))
    return found / (found + extra) if found + extra else 0.0


def run_f1(run: dict[str, Any]) -> float:
    """Return the harmonic mean of one run's precision and recall."""
    if run.get("timed_out"):
        return 0.0
    recall = len(run.get("found", [])) / len(run["expected"])
    precision = run_precision(run)
    return 2 * precision * recall / (precision + recall) if precision + recall else 0.0


def complete_run(run: dict[str, Any]) -> bool:
    """Return whether a run found every expected path without timing out."""
    return not run.get("timed_out") and not run.get("missed")


def summarize_results(records: list[dict[str, Any]], baseline: int) -> list[dict[str, Any]]:
    """Summarize each question and arm without unioning run recall."""
    groups: dict[tuple[str, str], list[dict[str, Any]]] = collections.defaultdict(list)
    for raw in records:
        record = normalize_record(raw)
        groups[(record["question"], record["mode"])].append(record)
    results = []
    for runs in groups.values():
        first = runs[0]
        if any("skip" in run for run in runs):
            results.append({"question": first["question"], "kind": first["kind"], "mode": first["mode"], "skip": next(run["skip"] for run in runs if "skip" in run)})
            continue
        expected_count = len(first["expected"])
        completed = [run for run in runs if not run.get("timed_out")]
        complete_runs = [run for run in runs if complete_run(run)]
        recalls = [len(run.get("found", [])) / expected_count for run in runs]
        precisions = [run_precision(run) for run in runs]
        f1_values = [run_f1(run) for run in runs]
        token_values = [max(0, run["tokens"] - baseline) for run in completed]
        results.append({
            "question": first["question"], "kind": first["kind"], "mode": first["mode"],
            "model": first.get("model", "unknown"), "tool": first.get("tool"),
            "mean_recall": statistics.mean(recalls),
            "mean_precision": statistics.mean(precisions),
            "mean_f1": statistics.mean(f1_values),
            "complete": f"{len(complete_runs)}/{len(runs)}",
            "median_extra": statistics.median(len(run.get("extra", [])) for run in runs),
            "median_tool_calls": median_tool_count(runs),
            "median_tokens": int(statistics.median(token_values)) if token_values else None,
            "timeout_count": sum(run.get("timed_out", False) for run in runs),
            "used_assigned_tool": tool_use_summary(runs),
            "_runs": runs,
        })
    return results


def rollup_results(records: list[dict[str, Any]], baseline: int, kind: str | None = None, used_only: bool = False) -> list[dict[str, Any]]:
    """Roll up per-run measurements by arm for code or all questions."""
    selected = [normalize_record(record) for record in records]
    if kind:
        selected = [record for record in selected if record["kind"] == kind]
    if used_only:
        selected = [record for record in selected if record.get("used_assigned_tool") is True]
    result = []
    for mode in ARMS:
        runs = [record for record in selected if record["mode"] == mode]
        if not runs:
            continue
        completed = [run for run in runs if not run.get("timed_out")]
        complete_runs = [run for run in runs if complete_run(run)]
        recalls = [len(run.get("found", [])) / len(run["expected"]) for run in runs]
        precisions = [run_precision(run) for run in runs]
        f1_values = [run_f1(run) for run in runs]
        tokens = [max(0, run["tokens"] - baseline) for run in completed]
        result.append({
            "kind": kind or "all", "mode": mode, "model": runs[0].get("model", "unknown"),
            "mean_recall": statistics.mean(recalls), "mean_precision": statistics.mean(precisions), "mean_f1": statistics.mean(f1_values), "complete": f"{len(complete_runs)}/{len(runs)}",
            "median_extra": statistics.median(len(run.get("extra", [])) for run in runs),
            "median_tool_calls": int(statistics.median(sum(run["tool_calls"].values()) for run in completed)) if completed else None,
            "median_tokens": int(statistics.median(tokens)) if tokens else None,
            "timeout_count": sum(run.get("timed_out", False) for run in runs),
            "used_assigned_tool": tool_use_summary(runs),
        })
    return result


def print_summary(rows: list[dict[str, Any]], title: str = "Per-question") -> None:
    """Print rows using the same columns for question and arm summaries."""
    print(title)
    print("question/kind | mode | mean recall | mean precision | mean F1 | complete | median extra | median tools | median tokens | timeouts | used assigned tool (true/false/unknown)")
    for row in rows:
        if "skip" in row:
            print(f"{row['question']} | {row['mode']} | SKIP: {row['skip']}")
            continue
        print(f"{row['question']} [{row['kind']}] | {row['mode']} | {row['mean_recall']:.3f} | {row['mean_precision']:.3f} | {row['mean_f1']:.3f} | {row['complete']} | {row['median_extra']} | {row['median_tool_calls']} | {row['median_tokens']} | {row['timeout_count']} | {row['used_assigned_tool']}")


def print_rollups(rows: list[dict[str, Any]], title: str) -> None:
    """Print arm roll-ups with the per-run measurement columns."""
    print(title)
    print("kind | mode | mean recall | mean precision | mean F1 | complete | median extra | median tools | median tokens | timeouts | used assigned tool (true/false/unknown)")
    for row in rows:
        print(f"{row['kind']} | {row['mode']} | {row['mean_recall']:.3f} | {row['mean_precision']:.3f} | {row['mean_f1']:.3f} | {row['complete']} | {row['median_extra']} | {row['median_tool_calls']} | {row['median_tokens']} | {row['timeout_count']} | {row['used_assigned_tool']}")


def print_gate(rollups: list[dict[str, Any]]) -> None:
    """Apply the adoption gate to all-run arm roll-ups."""
    baseline = next(row for row in rollups if row["kind"] == "all" and row["mode"] == "grep+skill")
    print("Gate verdicts (all runs)")
    for row in rollups:
        if row["mode"] in {"grep", "grep+skill"} or row["kind"] != "all":
            continue
        token_delta = (row["median_tokens"] - baseline["median_tokens"]) / baseline["median_tokens"]
        code_base = next(item for item in rollups if item["kind"] == "code" and item["mode"] == "grep+skill")
        code_row = next(item for item in rollups if item["kind"] == "code" and item["mode"] == row["mode"])
        passes = (row["mean_recall"] >= baseline["mean_recall"] and token_delta <= -0.25) or (code_row["mean_recall"] > code_base["mean_recall"] and token_delta <= 0.25)
        print(f"{row['mode']} | median tokens {row['median_tokens']} vs grep+skill {token_delta:+.1%} | code recall {code_row['mean_recall']:.3f} vs {code_base['mean_recall']:.3f} | overall recall {row['mean_recall']:.3f} vs {baseline['mean_recall']:.3f} | mean F1 {row['mean_f1']:.3f} vs {baseline['mean_f1']:.3f} | {'PASS' if passes else 'FAIL'}")


def test_git_key_commands_match_code_questions() -> None:
    """Machine-check only keys fully derivable from their pinned git command."""
    ensure_history()
    for question in load_questions():
        command = question.get("key_command")
        if not command:
            continue
        result = subprocess.run(
            ["git", "-C", str(source_root()), *command],
            check=True,
            capture_output=True,
            text=True,
        )
        commit = str(question["commit"])
        actual = sorted(
            line.removeprefix(f"{commit}:")
            for line in result.stdout.splitlines()
            if line
        )
        assert actual == sorted(question["expected"]), question["question"]


def test_replacement_arms_have_exact_commands() -> None:
    """Keep each replacement instruction tied to its documented tool."""
    question = {"question": "Where is state_dir?"}
    values = {mode: arm_instruction(mode, Path("surveil"), Path("."), Path("task"), Path("context"), Path("trace")) for mode in ARMS}
    assert "grep -r" in values["grep"]
    assert "rg -l" in values["grep+skill"]
    assert "--compress" in values["repomix"] and "--stdout" in values["repomix"]
    assert "probe search" in values["probe"] and "probe extract" in values["probe"]
    assert "--show-repo-map" in values["aider-map"] and "--map-tokens 1024" in values["aider-map"]


def test_missing_replacement_tool_skips(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """Explain a missing optional tool instead of failing the suite."""
    monkeypatch.delenv("PDE_REPOMIX", raising=False)
    monkeypatch.setattr(shutil, "which", lambda name: None)
    status = arm_status("repomix", tmp_path)
    assert status == {"skip": "repomix is not installed"}


def test_arms_share_question_without_terms() -> None:
    """Keep arm prompts fair by sharing only the question text."""
    questions = load_questions()
    for question in questions:
        prompts = []
        for mode in ARMS:
            instruction = arm_instruction(mode, Path("surveil"), Path("."), Path("task"), Path("context"), Path("trace"))
            prompt = f"{instruction} List every matching path for: {question['question']}"
            prompts.append(prompt)
            for term in question["terms"]:
                if term not in question["question"]:
                    assert term not in instruction
            assert prompt.endswith(question["question"])
        assert all(prompt.endswith(question["question"]) for prompt in prompts)


def test_per_run_recall_is_not_union() -> None:
    """Keep one successful run from masking misses in other runs."""
    records = [{"question": "q", "kind": "code", "mode": "probe", "expected": ["a", "b"], "found": ["a"], "extra": [], "tool_calls": {}, "tokens": 10}, {"question": "q", "kind": "code", "mode": "probe", "expected": ["a", "b"], "found": ["a", "b"], "extra": [], "tool_calls": {}, "tokens": 20}]
    summary = summarize_results(records, 0)
    assert summary[0]["mean_recall"] == 0.75


def test_precision_and_f1_cover_extras_and_timeouts() -> None:
    """Score extras as false positives and timeouts as zero."""
    run = {"question": "q", "kind": "code", "mode": "probe", "expected": ["a", "b"], "found": ["a", "b"], "missed": [], "extra": ["x"], "tool_calls": {}, "tokens": 1}
    timeout = {"question": "q", "kind": "code", "mode": "probe", "expected": ["a", "b"], "found": [], "missed": ["a", "b"], "extra": [], "timed_out": True, "tool_calls": {}, "tokens": None}
    assert run_precision(run) == 2 / 3
    assert run_f1(run) == 0.8
    assert run_precision(timeout) == 0.0 and run_f1(timeout) == 0.0


def test_complete_requires_no_misses() -> None:
    """Do not call an incomplete answer complete."""
    records = [{"question": "q", "kind": "code", "mode": "probe", "expected": ["a"], "found": [], "missed": ["a"], "extra": [], "tool_calls": {}, "tokens": 1}]
    assert summarize_results(records, 0)[0]["complete"] == "0/1"


def test_tool_use_reads_saved_inputs() -> None:
    """Verify assigned-tool checks from persisted command inputs."""
    assert assigned_tool_used("probe", [{"tool": "shell", "input": "probe search query ."}], {}) is True
    assert assigned_tool_used("repomix", [{"tool": "shell", "input": "rg -l query ."}], {}) is False
    assert assigned_tool_used("aider-map", [{"tool": "shell", "input": "aider --show-repo-map"}], {}) is True
    assert assigned_tool_used("aider-map", None, {"grep": 1}) is None
    assert assigned_tool_used("surveil", None, {"grep": 1}) is False


def test_rescore_accepts_old_records() -> None:
    """Keep old records usable when tool inputs were not saved."""
    record = {"question": "Which planner files report missing base commits?", "kind": "code", "mode": "surveil", "tool_calls": {"grep": 1}}
    normalized = normalize_record(record)
    assert normalized["question"] == QUESTION_ALIASES[record["question"]]
    assert normalized["used_assigned_tool"] is False


def test_results_append_per_run(tmp_path: Path) -> None:
    """Keep completed runs when a later evaluation step fails."""
    path = tmp_path / "runs.jsonl"
    append_result(path, {"attempt": 0, "mode": "grep"})
    append_result(path, {"attempt": 1, "mode": "grep"})
    assert load_result_records(path) == [
        {"attempt": 0, "mode": "grep"},
        {"attempt": 1, "mode": "grep"},
    ]


@pytest.mark.local_ai
def test_agent_search_comparison(tmp_path: Path) -> None:
    """Compare agent recall and cost for equivalent search instructions.

    Three repetitions expose run variance, while the no-op baseline removes
    fixed prompt overhead from the reported median search cost.
    """
    ensure_history()
    binary = build_binary(tmp_path)
    baseline = token_total(run_opencode("Reply exactly: ok. Do not use tools.", tmp_path, binary))
    results_path = Path(os.environ.get(RESULTS_ENV_VAR, str(tmp_path / "navigation-agent-results.jsonl")))
    results_path.write_text("", encoding="utf-8")
    code_questions = [question for question in load_questions() if question["kind"] == "code"]
    literal_questions = [question for question in load_questions() if question["kind"] == "literal"][:2]
    for number, question in enumerate(code_questions + literal_questions):
        for mode in ARMS:
            project = tmp_path / f"{mode}-{number}"
            project.mkdir()
            materialize(str(question["commit"]), project)
            for attempt in range(3):
                run = run_agent(binary, project, tmp_path / f"{mode}-task-{number}-{attempt}", question, mode)
                append_result(results_path, {"question": question["question"], "kind": question["kind"], "mode": mode, "attempt": attempt, "expected": question["expected"], **run})
    records = load_result_records(results_path)
    question_rows = summarize_results(records, baseline)
    rollups = rollup_results(records, baseline) + rollup_results(records, baseline, "code")
    output = results_path.with_suffix(".json")
    output.write_text(json.dumps({"baseline_tokens": baseline, "model": model_name(), "question_rows": question_rows, "rollups": rollups}, indent=2, default=str) + "\n", encoding="utf-8")
    print_summary(question_rows)
    print_rollups(rollup_results(records, baseline, "code"), "Code roll-ups (all runs)")
    print_rollups(rollup_results(records, baseline), "All-question roll-ups (all runs)")
    print_rollups(rollup_results(records, baseline, "code", True), "Code roll-ups (assigned-tool runs)")
    print_rollups(rollup_results(records, baseline, None, True), "All-question roll-ups (assigned-tool runs)")
    print_gate(rollups)
