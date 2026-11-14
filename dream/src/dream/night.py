"""One night of dreaming, from probe to morning report.

Read ``run_night`` first; it lays out the whole night. Every file it
writes is under the config's ``state_dir``. Nothing here changes a
card, an issue or a session.
"""

from __future__ import annotations

import dataclasses
import datetime
import hashlib
import json
import os
import tempfile
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from dream.checks import FORM_KEYS, check_answer, parse_rows
from dream.config import Config
from dream.forms import (
    BRIEF_A,
    BRIEF_B,
    IDLE_MINUTES,
    Fact,
    build_facts,
    prompt_a,
    prompt_b,
)
from dream.ports import (
    AdapterError,
    Answer,
    Model,
    Sessions,
    SessionStats,
    SessionSummary,
    WorkItem,
    WorkItems,
)

PROBE_SECONDS = 120
ASK_SECONDS = 600
SINCE_HOURS = 24

Clock = Callable[[], datetime.datetime]


@dataclass
class RunRecord:
    """One line of runs.jsonl, for one item tried."""

    run: str
    form: str
    item: str
    adapters: str
    model: str
    brief_sha: str
    input_sha: str
    tries: int = 0
    result: str = "fail"
    reasons: list[str] = field(default_factory=list)
    seconds: float = 0.0
    tokens_read: int = 0
    output: str = ""


def run_night(config: Config, only: str | None, now: Clock) -> int:
    """Probe, pick, ask, check, save and report; 0 once the report is written.

    One failed item or form goes in the report's errors and never stops
    the night. A crash outside an item still writes the report.
    """
    start = now()
    run_id = start.strftime("%Y%m%d-%H%M")
    deadline = next_deadline(start, config.stop_starting_at)
    state = config.state_dir
    state.mkdir(parents=True, exist_ok=True)
    records: list[RunRecord] = []
    errors: list[str] = []
    status = ""
    try:
        status = probe(config.model)
        if status:
            status = f"skipped: {status}"
        else:
            if only in (None, "A") and config.work_items is not None:
                dream_items(config, run_id, deadline, now, records, errors)
            if only in (None, "B") and config.sessions is not None:
                dream_sessions(config, run_id, deadline, now, records, errors)
    # The morning report must always exist, so any failure is caught.
    except Exception as error:  # noqa: BLE001
        status = f"crashed: {type(error).__name__}: {error}"
    write_report(
        state,
        run_id,
        describe_run(config, start, now(), status),
        records,
        errors,
    )
    return 0


def next_deadline(
    start: datetime.datetime, stop: datetime.time
) -> datetime.datetime:
    """Return the first ``stop`` after ``start``, so a day run has one too."""
    deadline = datetime.datetime.combine(start.date(), stop)
    if deadline <= start:
        deadline += datetime.timedelta(days=1)
    return deadline


def probe(model: Model) -> str:
    """Return "" when the model answers "reply OK", else the reason."""
    answer = model.ask("reply OK", timeout=PROBE_SECONDS)
    if answer.text.strip():
        return ""
    return answer.failure or "empty answer"


def dream_items(
    config: Config,
    run_id: str,
    deadline: datetime.datetime,
    now: Clock,
    records: list[RunRecord],
    errors: list[str],
) -> None:
    """Run form A on the night's picked work items."""
    assert config.work_items is not None
    state = config.state_dir
    try:
        items = pick_items(config.work_items, state / "proposals")
    except AdapterError as error:
        errors.append(f"form A skipped: {error}")
        return
    for item in items[: config.items_per_night]:
        if now() >= deadline:
            errors.append(f"form A stopped at {deadline:%H:%M}")
            return
        record = new_record(config, run_id, "A", item.key, BRIEF_A, item.text)
        answer, reasons, tries = dream_one(
            config.model, "A", prompt_a(item), item.text
        )
        finish_record(record, answer, reasons, tries)
        if not reasons:
            rows, _ = parse_rows(answer.text, FORM_KEYS["A"])
            try:
                path = save_proposal(
                    state,
                    item,
                    rows,
                    config.work_items.name,
                    config.model_name,
                    run_id,
                    now(),
                )
                record.output = str(path)
            except FileExistsError:
                record.result = "skipped"
        append_run_record(state, record)
        records.append(record)


def dream_sessions(
    config: Config,
    run_id: str,
    deadline: datetime.datetime,
    now: Clock,
    records: list[RunRecord],
    errors: list[str],
) -> None:
    """Run form B on the night's picked sessions."""
    assert config.sessions is not None
    state = config.state_dir
    try:
        summaries = pick_sessions(
            config.sessions, state / "digests", config.min_cloud_tokens
        )
    except AdapterError as error:
        errors.append(f"form B skipped: {error}")
        return
    for summary in summaries[: config.sessions_per_night]:
        if now() >= deadline:
            errors.append(f"form B stopped at {deadline:%H:%M}")
            return
        try:
            stats = config.sessions.read_stats(summary)
        except AdapterError as error:
            errors.append(f"form B {summary.id}: {error}")
            continue
        facts = build_facts(stats)
        record = new_record(
            config, run_id, "B", summary.id, BRIEF_B, facts_to_json(facts)
        )
        answer, reasons, tries = dream_one(
            config.model, "B", prompt_b(stats, facts), facts
        )
        finish_record(record, answer, reasons, tries)
        if not reasons:
            rows, _ = parse_rows(answer.text, FORM_KEYS["B"])
            record.output = str(save_digest(state, stats, facts, rows))
        append_run_record(state, record)
        records.append(record)


def pick_items(work: WorkItems, staged: Path) -> list[WorkItem]:
    """Drafts with no criteria and no staged proposal, oldest first.

    An item with any proposal, judged or not, is skipped, so a dropped
    proposal does not come back every night.
    """
    items = [
        item
        for item in work.list_drafts()
        if not item.has_criteria
        and not (staged / f"{item.file_key}.md").exists()
    ]
    return sorted(items, key=lambda item: (item.created, item.key))


def pick_sessions(
    sessions: Sessions, staged: Path, min_cloud_tokens: int
) -> list[SessionSummary]:
    """Finished large sessions with no current digest, most cloud tokens first.

    A session idle under an hour may still be running. A digest newer
    than the session's last step is current.
    """
    picked = []
    for summary in sessions.list_large(SINCE_HOURS, min_cloud_tokens):
        if summary.idle_minutes < IDLE_MINUTES:
            continue
        digest = staged / f"{summary.id}.jsonl"
        last_step = time.time() - summary.idle_minutes * 60
        if digest.exists() and digest.stat().st_mtime > last_step:
            continue
        picked.append(summary)
    return sorted(picked, key=lambda s: s.cloud_tokens, reverse=True)


def dream_one(
    model: Model, form: str, prompt: str, given: str | list[Fact]
) -> tuple[Answer, list[str], int]:
    """Ask, check, and retry once on the same model; return the last try.

    Returns:
        The last answer, its reasons (empty on a pass) and the tries,
        whose ``tokens_read`` and ``seconds`` cover both tries.
    """
    tokens = 0
    seconds = 0.0
    for tries in (1, 2):
        answer = model.ask(prompt, timeout=ASK_SECONDS)
        tokens += answer.tokens_read
        seconds += answer.seconds
        reasons = (
            [answer.failure]
            if answer.failure
            else check_answer(form, answer.text, given)
        )
        if not reasons:
            break
    total = dataclasses.replace(answer, tokens_read=tokens, seconds=seconds)
    return total, reasons, tries


def new_record(
    config: Config, run_id: str, form: str, item: str, brief: str, given: str
) -> RunRecord:
    """Start a run record with the adapters and the brief and input hashes."""
    adapters = " ".join(
        port.name
        for port in (config.work_items, config.sessions, config.model)
        if port is not None
    )
    return RunRecord(
        run=run_id,
        form=form,
        item=item,
        adapters=adapters,
        model=config.model_name,
        brief_sha=sha256(brief),
        input_sha=sha256(given),
    )


def finish_record(
    record: RunRecord, answer: Answer, reasons: list[str], tries: int
) -> None:
    """Copy the last try's result into the record."""
    record.tries = tries
    record.result = "fail" if reasons else "pass"
    record.reasons = reasons
    record.seconds = round(answer.seconds, 1)
    record.tokens_read = answer.tokens_read


def save_proposal(
    state: Path,
    item: WorkItem,
    rows: list[dict[str, Any]],
    source: str,
    model: str,
    run_id: str,
    now: datetime.datetime,
) -> Path:
    """Create proposals/<file_key>.md with exclusive create.

    Raises:
        FileExistsError: Another run already wrote this item's proposal.
    """
    folder = state / "proposals"
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"{item.file_key}.md"
    criteria = [r for r in rows if r.get("kind") == "criterion"]
    questions = [r for r in rows if r.get("kind") == "question"]
    lines = [
        "---",
        f"item: {item.key}",
        f"source: {source}",
        f"item_sha: {sha256(item.text)}",
        f"model: {model}",
        f"run: {run_id}",
        f"date_created: {now.date().isoformat()}",
        "outcome:",
        "judged:",
        "---",
        "",
        f"# Proposal for {item.key}",
        "",
        "## Criteria",
        "",
        *(f'- [ ] {r["text"]} Based on: "{r["based_on"]}"' for r in criteria),
        "",
        "## Questions",
        "",
        *(
            f'{n}. {r["text"]} Based on: "{r["based_on"]}"'
            for n, r in enumerate(questions, start=1)
        ),
    ]
    with path.open("x") as handle:
        handle.write("\n".join(lines) + "\n")
    return path


def save_digest(
    state: Path,
    stats: SessionStats,
    facts: list[Fact],
    rows: list[dict[str, Any]],
) -> Path:
    """Replace <id>.facts.json, then <id>.jsonl, each atomically.

    The facts go first, so a reader never finds a digest whose facts
    are missing.
    """
    folder = state / "digests"
    folder.mkdir(parents=True, exist_ok=True)
    session_id = stats.summary.id
    write_atomic(folder / f"{session_id}.facts.json", facts_to_json(facts))
    digest = folder / f"{session_id}.jsonl"
    write_atomic(digest, "".join(json.dumps(r) + "\n" for r in rows))
    return digest


def facts_to_json(facts: list[Fact]) -> str:
    """Return the facts file's text, which ``dream check B`` reads back."""
    return json.dumps(
        [
            {"id": f.id, "line": f.line, "numbers": sorted(f.numbers)}
            for f in facts
        ]
    )


def append_run_record(state: Path, record: RunRecord) -> None:
    """Append one JSON line to runs.jsonl with O_APPEND."""
    line = json.dumps(dataclasses.asdict(record)) + "\n"
    fd = os.open(
        state / "runs.jsonl", os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644
    )
    try:
        os.write(fd, line.encode())
    finally:
        os.close(fd)


def describe_run(
    config: Config,
    start: datetime.datetime,
    end: datetime.datetime,
    status: str,
) -> str:
    """Return the report's run line."""
    line = (
        f"Run {start:%Y-%m-%d %H:%M} to {end:%H:%M}, model {config.model_name}"
    )
    return f"{line}, {status}" if status else line


def write_report(
    state: Path,
    run_id: str,
    run_line: str,
    records: list[RunRecord],
    errors: list[str],
) -> Path:
    """Write latest.md atomically, ending in ``done``."""
    lines = [f"# Dream report {run_id}", "", run_line, "", "## Proposals", ""]
    for r in records:
        if r.form == "A":
            lines.append(
                f"- {r.item}: {r.output or r.result} "
                f"{'; '.join(r.reasons)}".rstrip()
            )
    digests = [r for r in records if r.form == "B"]
    passed = sum(r.result == "pass" for r in digests)
    lines += ["", "## Digests", "", f"{passed} of {len(digests)} passed", ""]
    lines += [
        f"- {r.item}: {r.result} {'; '.join(r.reasons)}".rstrip()
        for r in digests
    ]
    lines += ["", "## Errors", ""]
    lines += [f"- {e}" for e in errors] or ["None"]
    lines += ["", "done"]
    path = state / "latest.md"
    write_atomic(path, "\n".join(lines) + "\n")
    return path


def write_atomic(path: Path, text: str) -> None:
    """Replace a file through a temporary file, so no reader sees half."""
    handle, temp = tempfile.mkstemp(dir=path.parent, prefix=".tmp-")
    with os.fdopen(handle, "w") as out:
        out.write(text)
    os.replace(temp, path)


def sha256(text: str) -> str:
    """Return the SHA-256 of text, as hex."""
    return hashlib.sha256(text.encode()).hexdigest()
