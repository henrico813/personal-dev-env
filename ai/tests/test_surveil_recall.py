"""Measure Surveil recall and agent search cost."""

from __future__ import annotations

import collections
import json
import os
import re
import shutil
import statistics
import subprocess
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parents[2]
GOLDEN = ROOT / "ai/tests/surveil_golden_questions.json"


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


def run_opencode(prompt: str, cwd: Path, binary: Path) -> list[dict[str, Any]]:
    """Run OpenCode with the freshly built binary first on PATH."""
    result = subprocess.run(
        ["opencode", "run", "--standalone", "--auto", "--model", "goog/qwen3.8", "--format", "json", prompt],
        cwd=cwd, env={**os.environ, "PATH": f"{binary.parent}:{os.environ['PATH']}", "PWD": str(cwd)},
        capture_output=True, text=True, timeout=180,
    )
    if result.returncode != 0:
        raise AssertionError(f"OpenCode failed: {result.stderr}")
    return parse_events(result.stdout)


def run_agent(binary: Path, repo: Path, task_dir: Path, question: dict[str, Any], mode: str) -> dict[str, Any]:
    """Run one explicit search instruction and score its final answer."""
    task_dir.mkdir()
    task = task_dir / "task.json"
    context = task_dir / "context.json"
    trace = task_dir / "trace.json"
    write_task(task, question)
    if mode == "surveil":
        instruction = (
            f"Use only {binary} and run exactly: {binary} index --repo {repo}; "
            f"{binary} gather --repo {repo} --task-file {task} > {context}; then "
            f"{binary} research --context {context} --trace-out {trace}. "
            f"The task JSON is already at {task} and contains this question."
        )
    else:
        instruction = f"Use only grep -rl -- {question['terms'][0]} . from the current project directory. Do not use git grep."
    events = run_opencode(f"{instruction} List every matching path for: {question['question']}", repo, binary)
    found = listed_paths(final_text(events), repo)
    expected = set(question["expected"])
    return {
        "found": sorted(found & expected),
        "missed": sorted(expected - found),
        "extra": sorted(found - expected),
        "tool_calls": tool_counts(events),
        "tokens": token_total(events),
    }


def median_tool_count(runs: list[dict[str, Any]]) -> int:
    """Return the median total tool calls across repetitions."""
    return int(statistics.median(sum(run["tool_calls"].values()) for run in runs))


def print_summary(results: list[dict[str, Any]]) -> None:
    """Print one compact recall and cost row per question and mode."""
    print("question | kind | mode | found/missed/extra | median tools | median tokens above baseline")
    for result in results:
        print(f"{result['question']} | {result['kind']} | {result['mode']} | {len(result['found'])}/{len(result['missed'])}/{len(result['extra'])} | {result['median_tool_calls']} | {result['median_tokens_above_baseline']}")


@pytest.mark.local_ai
def test_agent_search_comparison(tmp_path: Path) -> None:
    """Compare agent recall and cost for equivalent search instructions.

    Three repetitions expose run variance, while the no-op baseline removes
    fixed prompt overhead from the reported median search cost.
    """
    ensure_history()
    binary = build_binary(tmp_path)
    baseline = token_total(run_opencode("Reply exactly: ok. Do not use tools.", tmp_path, binary))
    results = []
    code_questions = [question for question in load_questions() if question["kind"] == "code"]
    literal_questions = [question for question in load_questions() if question["kind"] == "literal"][:2]
    for number, question in enumerate(code_questions + literal_questions):
        for mode in ("grep", "surveil"):
            project = tmp_path / f"{mode}-{number}"
            project.mkdir()
            materialize(str(question["commit"]), project)
            runs = [run_agent(binary, project, tmp_path / f"{mode}-task-{number}-{attempt}", question, mode) for attempt in range(3)]
            found = sorted(set().union(*(set(run["found"]) for run in runs)))
            missed = sorted(set(question["expected"]) - set(found))
            extra = sorted(set().union(*(set(run["extra"]) for run in runs)))
            raw_tokens = [max(0, run["tokens"] - baseline) for run in runs]
            results.append({
                "question": question["question"], "kind": question["kind"], "mode": mode,
                "found": found, "missed": missed, "extra": extra,
                "tool_calls": [run["tool_calls"] for run in runs],
                "tokens_above_baseline": raw_tokens,
                "median_tool_calls": median_tool_count(runs),
                "median_tokens_above_baseline": int(statistics.median(raw_tokens)),
            })
    output = tmp_path / "surveil-agent-results.json"
    output.write_text(json.dumps({"baseline_tokens": baseline, "results": results}, indent=2) + "\n", encoding="utf-8")
    print_summary(results)
    print(output.read_text(encoding="utf-8"))
