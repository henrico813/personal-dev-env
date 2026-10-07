"""Rescore saved navigation runs without calling a model."""

from __future__ import annotations

import argparse
from pathlib import Path

from test_surveil_recall import (
    load_result_records,
    model_name,
    print_gate,
    print_rollups,
    print_summary,
    rollup_results,
    summarize_results,
)


def main() -> None:
    """Print per-run summaries and gate results from a JSONL file."""
    parser = argparse.ArgumentParser()
    parser.add_argument("results", type=Path)
    parser.add_argument("--baseline-tokens", type=int, default=0)
    args = parser.parse_args()
    records = load_result_records(args.results)
    question_rows = summarize_results(records, args.baseline_tokens)
    all_rollups = rollup_results(records, args.baseline_tokens)
    code_rollups = rollup_results(records, args.baseline_tokens, "code")
    print(f"model | {records[0].get('model', model_name()) if records else model_name()}")
    print_summary(question_rows)
    print_rollups(code_rollups, "Code roll-ups (all runs)")
    print_rollups(all_rollups, "All-question roll-ups (all runs)")
    print_rollups(rollup_results(records, args.baseline_tokens, "code", True), "Code roll-ups (assigned-tool runs)")
    print_rollups(rollup_results(records, args.baseline_tokens, None, True), "All-question roll-ups (assigned-tool runs)")
    print_gate(code_rollups + all_rollups)


if __name__ == "__main__":
    main()
