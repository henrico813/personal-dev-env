"""Run stored trials of a model against current rules and grade them.

Each case in ``<trials>/cases/*.json`` is the situation before a mistake
the user corrected, with that correction as its answer key. The script
sends the case's rules files and prompt to ``opencode run`` N times and
grades each answer with the case's patterns. Read ``main`` first.

Example:
    python3 ~/.agents/skills/session-review/scripts/trials.py -n 5
    python3 trials.py --case stale-approval --variant high
"""

import argparse
import contextlib
import datetime
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

TRIALS = Path.home() / ".local/state/pde/session-reviews/trials"
REQUIRED = {"id", "source", "rules", "prompt", "pass"}
OPTIONAL = {"fail"}
# Denying every permission takes all tools away from the model, so a
# case cannot act out the hub actions its prompt describes.
NO_TOOLS = '{"permission": {"*": "deny"}}'
# Without this note a model with no tools often writes a pretend tool call
# as its answer. The rules hash covers it, so a new note reads as new rules.
NO_TOOLS_NOTE = (
    "You have no tools. Answer in text, and name any command you would run."
)
PRETEND_TOOL_CALL = re.compile(r"<tool_call>|<function=")
CASE_ID = re.compile(r"[a-z0-9]+(-[a-z0-9]+)*")


class CaseError(Exception):
    """A case file or one of its rules files cannot be used."""


def main(argv: list[str] | None = None) -> int:
    """Run every chosen case N times and print one line per case.

    Args:
        argv: Command-line options, or None for ``sys.argv``.

    Returns:
        1 when a case cannot be used, else 0, whatever the pass rate.
    """
    args = parse_args(argv)
    try:
        cases = load_cases(args.trials, args.case)
    except CaseError as error:
        print(f"trials: {error}", file=sys.stderr)
        return 1
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    (args.trials / "answers").mkdir(parents=True, exist_ok=True)
    # The random suffix keeps two calls started in the same second apart.
    answers = Path(
        tempfile.mkdtemp(prefix=f"{stamp}-", dir=args.trials / "answers")
    )
    total = 0
    with open(args.trials / "results.jsonl", "a", encoding="utf-8") as results:
        for case in cases:
            failures = []
            for number in range(1, args.n + 1):
                result = run_trial(case, number, args, answers)
                results.write(json.dumps(result) + "\n")
                results.flush()
                failures += result["reasons"][:1]
                total += result["passed"]
            line = f"{case['id']} {args.n - len(failures)}/{args.n}"
            line += f" rules {case['rules_hash']}"
            print(f"{line} - {failures[0]}" if failures else line)
    model = args.model + (f"#{args.variant}" if args.variant else "")
    print(
        f"total {total}/{len(cases) * args.n} on {model}, answers in {answers}"
    )
    return 0


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Read the command-line options."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--trials", type=Path, default=TRIALS)
    parser.add_argument(
        "--case", action="append", default=[], help="case ID, repeatable"
    )
    parser.add_argument("-n", type=int, default=3, help="runs per case")
    parser.add_argument("--model", default="goog/qwen3.8")
    parser.add_argument("--variant", help="reasoning effort, such as high")
    parser.add_argument(
        "--timeout", type=int, default=600, help="seconds per run"
    )
    return parser.parse_args(argv)


def load_cases(trials: Path, wanted: list[str]) -> list[dict]:
    """Read and check every case, so a bad one stops the script early.

    Args:
        trials: The trials folder, holding ``cases/``.
        wanted: Case IDs to keep, or an empty list for all.

    Returns:
        The chosen cases, each with its rules text and both hashes added.

    Raises:
        CaseError: A case is broken, a rules file is missing, or a
            wanted ID names no case.
    """
    cases = [read_case(path) for path in sorted(trials.glob("cases/*.json"))]
    ids = [case["id"] for case in cases]
    repeated = sorted({id for id in ids if ids.count(id) > 1})
    unknown = [id for id in wanted if id not in ids]
    if repeated or unknown:
        raise CaseError(f"repeated IDs {repeated}, unknown IDs {unknown}")
    cases = [case for case in cases if not wanted or case["id"] in wanted]
    if not cases:
        raise CaseError(f"no cases in {trials / 'cases'}")
    return cases


def read_case(path: Path) -> dict:
    """Read one case file and the rules files it lists.

    Args:
        path: A file in ``cases/``.

    Returns:
        The case's fields plus ``rules_text``, ``rules_hash`` and
        ``case_hash``.

    Raises:
        CaseError: The file is not a case, or a rules file is missing.
    """
    try:
        case = json.loads(path.read_text(encoding="utf-8"))
    except ValueError as error:
        raise CaseError(f"{path}: {error}") from error
    if not isinstance(case, dict) or not REQUIRED <= set(case):
        raise CaseError(f"{path}: needs fields {sorted(REQUIRED)}")
    unknown = sorted(set(case) - REQUIRED - OPTIONAL)
    if unknown:
        raise CaseError(f"{path}: unknown fields {unknown}")
    # The ID names answer files, so ``../`` would write outside the folder.
    if not isinstance(case["id"], str) or not CASE_ID.fullmatch(case["id"]):
        raise CaseError(f"{path}: id must be lowercase letters, digits, -")
    for field in ("rules", "pass", "fail"):
        value = case.get(field, [])
        if not isinstance(value, list) or not all(
            isinstance(item, str) for item in value
        ):
            raise CaseError(f"{path}: {field} must be a list of strings")
    if not case["pass"]:
        raise CaseError(f"{path}: pass needs at least one pattern")
    try:
        for pattern in case["pass"] + case.get("fail", []):
            re.compile(pattern)
    except re.error as error:
        raise CaseError(f"{path}: bad pattern {pattern!r}: {error}") from error
    missing = [
        name for name in case["rules"] if not Path(name).expanduser().is_file()
    ]
    if missing:
        raise CaseError(f"{path}: missing rules files {missing}")
    case["rules_text"], case["rules_hash"] = read_rules(case["rules"])
    case["case_hash"] = hashlib.sha256(path.read_bytes()).hexdigest()[:12]
    return case


def read_rules(names: list[str]) -> tuple[str, str]:
    """Join the rules files into one text and hash them as listed.

    The hash covers each name as written, so it stays the same on
    another computer with the same rules. It also covers the no-tools
    note every prompt gets.

    Returns:
        The joined text and the first 12 hex digits of its sha256.
    """
    digest = hashlib.sha256(f"{NO_TOOLS_NOTE}\n".encode())
    parts = []
    for name in names:
        data = Path(name).expanduser().read_bytes()
        digest.update(f"{name}\n{len(data)}\n".encode())
        digest.update(data)
        parts.append(f"===== {name} =====\n{data.decode()}\n")
    return "\n".join(parts), digest.hexdigest()[:12]


def run_trial(
    case: dict, number: int, args: argparse.Namespace, answers: Path
) -> dict:
    """Run one trial in a new empty folder, grade it and save its answer.

    Args:
        case: A case from ``load_cases``.
        number: The run's number, from 1.
        args: The command-line options.
        answers: Folder for this call's answer files.

    Returns:
        The line to add to ``results.jsonl``.
    """
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="trial-") as folder:
        answer, reasons = ask_model(case, Path(folder), args)
    if not reasons:
        reasons = grade(case, answer)
    path = answers / f"{case['id']}-{number}.txt"
    path.write_text(answer, encoding="utf-8")
    return {
        "time": datetime.datetime.now().isoformat(timespec="seconds"),
        "case": case["id"],
        "run": number,
        "model": args.model,
        "variant": args.variant,
        "rules_hash": case["rules_hash"],
        "case_hash": case["case_hash"],
        "passed": not reasons,
        "reasons": reasons,
        "seconds": round(time.monotonic() - started, 1),
        "answer": str(path),
    }


def ask_model(
    case: dict, scratch: Path, args: argparse.Namespace
) -> tuple[str, list[str]]:
    """Send the rules and prompt to ``opencode run`` and read its answer.

    Returns:
        The answer text, and one reason when the run failed.
    """
    model = args.model + (f"#{args.variant}" if args.variant else "")
    command = ["opencode", "run", "--standalone", "-m", model]
    command += ["--format", "json"]
    command.append(f"{case['rules_text']}\n{NO_TOOLS_NOTE}\n{case['prompt']}")
    # A config file in a folder above the scratch folder, or one these
    # variables name, could change the model or give tools back.
    env = {
        name: value
        for name, value in os.environ.items()
        if name not in ("OPENCODE_CONFIG", "OPENCODE_CONFIG_DIR")
    }
    env |= {
        "OPENCODE_CONFIG_CONTENT": NO_TOOLS,
        "OPENCODE_DISABLE_PROJECT_CONFIG": "1",
        # OpenCode reads this newer name first, so an inherited value
        # would turn parent-folder config back on.
        "OPENCODE_CONFIG_PROJECT_DISABLE": "1",
    }
    code, output, error = run_bounded(command, scratch, env, args.timeout)
    if code != 0:
        return "", [f"opencode {describe_failure(code, error, args.timeout)}"]
    return read_answer(output), []


def run_bounded(
    command: list[str], scratch: Path, env: dict, timeout: int
) -> tuple[int | None, str, str]:
    """Run a command in the scratch folder, killing it after ``timeout``.

    The command gets its own process group, so the kill also stops any
    process it started that still holds its output open. Ctrl-C does not
    reach that group, so any error here kills the group and is raised
    again.

    Returns:
        The exit code, or None after a timeout, then stdout and stderr.
    """
    with subprocess.Popen(
        command,
        cwd=scratch,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    ) as process:
        try:
            output, error = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
            return None, "", ""
        except BaseException:
            # A group that already exited must not hide the Ctrl-C.
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, signal.SIGKILL)
            raise
    return process.returncode, output, error


def describe_failure(code: int | None, error: str, timeout: int) -> str:
    """Say how a command failed, with its last line of stderr."""
    if code is None:
        return f"timed out after {timeout} s"
    last = error.strip().splitlines()[-1:] or [""]
    return f"exited {code} {last[0]}".rstrip()


def read_answer(output: str) -> str:
    """Join the text parts of ``opencode run --format json`` output.

    Lines that are not a text event with text are skipped, so one odd
    line cannot stop the whole batch.
    """
    texts = []
    for line in output.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict) or event.get("type") != "text":
            continue
        part = event.get("part")
        if isinstance(part, dict) and isinstance(part.get("text"), str):
            texts.append(part["text"])
    return "\n".join(texts)


def grade(case: dict, answer: str) -> list[str]:
    """List why the answer fails the case's patterns, ignoring case.

    A pretend tool call fails on its own, since its text can match a pass
    pattern without the model answering at all.
    """
    if PRETEND_TOOL_CALL.search(answer):
        return ["tool call instead of answer"]
    missed = [
        f"missed pass /{pattern}/"
        for pattern in case["pass"]
        if not re.search(pattern, answer, re.I)
    ]
    matched = [
        f"matched fail /{pattern}/"
        for pattern in case.get("fail", [])
        if re.search(pattern, answer, re.I)
    ]
    return missed + matched


if __name__ == "__main__":
    sys.exit(main())
