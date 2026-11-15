"""Run stored trials of a model against current rules and grade them.

Each case is the situation before a mistake, with the corrected
behavior as its answer key. A case in ``<trials>/cases/*.json`` lists
its rules files. A skill's case, in ``ai/skills/<skill>/evals/*.json``
of the current Git repository, gets that skill's SKILL.md instead.
The script sends the rules and prompt to ``opencode run`` N times and
grades each answer with the case's patterns. ``--at`` and
``--without`` add runs on the skill's old text and on no rules, to
compare. Read ``main`` first.

Example:
    python3 ~/.agents/skills/session-review/scripts/trials.py -n 5
    python3 trials.py --case stale-approval --variant high
    python3 trials.py --skill skill-development --at main --without
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
# A case in the trials folder also needs "rules", the files it lists.
# A skill case needs "rule" instead, the SKILL.md heading it tests.
REQUIRED = {"id", "source", "prompt", "pass"}
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
    """Run every chosen case N times per skill text; print a line per case.

    Args:
        argv: Command-line options, or None for ``sys.argv``.

    Returns:
        1 when a case cannot be used, else 0, whatever the pass rate.
    """
    args = parse_args(argv)
    try:
        cases = load_cases(args)
    except CaseError as error:
        print(f"trials: {error}", file=sys.stderr)
        return 1
    stamp = datetime.datetime.now().strftime("%Y%m%d-%H%M%S")
    (args.trials / "answers").mkdir(parents=True, exist_ok=True)
    # The random suffix keeps two calls started in the same second apart.
    answers = Path(
        tempfile.mkdtemp(prefix=f"{stamp}-", dir=args.trials / "answers")
    )
    log = args.trials / "results.jsonl"
    saved = read_saved_runs(log) if args.without else []
    version = read_opencode_version()
    total = count = 0
    with open(log, "a", encoding="utf-8") as results:
        for case in cases:
            graded = []
            for skill_text in case["skill_texts"]:
                runs = find_saved_runs(saved, case, skill_text, args)
                reused = bool(runs)
                if not reused:
                    for number in range(1, args.n + 1):
                        result = run_trial(
                            case, skill_text, number, args, answers
                        )
                        result["opencode"] = version
                        results.write(json.dumps(result) + "\n")
                        results.flush()
                        runs.append(result)
                graded.append((skill_text, runs, reused))
                total += sum(run["passed"] for run in runs)
                count += len(runs)
            print(summarize_case(case, graded))
    model = args.model + (f"#{args.variant}" if args.variant else "")
    print(f"total {total}/{count} on {model}, answers in {answers}")
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
    parser.add_argument(
        "--skill",
        action="append",
        default=[],
        help="run this skill's cases in this Git repository, repeatable",
    )
    parser.add_argument(
        "--at", help="also run each skill case on its SKILL.md at this commit"
    )
    parser.add_argument(
        "--without",
        action="store_true",
        help="also run each case with no rules, reusing saved runs",
    )
    args = parser.parse_args(argv)
    if args.at and not args.skill:
        parser.error("--at needs --skill")
    return args


def load_cases(args: argparse.Namespace) -> list[dict]:
    """Read and check every chosen case, so a bad one stops the script early.

    Args:
        args: The command-line options. With ``--skill``, the cases
            come from each skill's ``evals/`` in the checkout instead
            of the trials folder's ``cases/``.

    Returns:
        The chosen cases, each with its case hash and skill texts added.

    Raises:
        CaseError: A case is broken, its rules are missing, or a
            wanted ID names no case.
    """
    if args.skill:
        top = Path(run_git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
        folders = [
            (top / "ai/skills" / name / "evals", name) for name in args.skill
        ]
    else:
        top = None
        folders = [(args.trials / "cases", None)]
    cases = [
        read_case(path, skill)
        for folder, skill in folders
        for path in sorted(folder.glob("*.json"))
    ]
    ids = [case["id"] for case in cases]
    repeated = sorted({id for id in ids if ids.count(id) > 1})
    unknown = [id for id in args.case if id not in ids]
    if repeated or unknown:
        raise CaseError(f"repeated IDs {repeated}, unknown IDs {unknown}")
    cases = [
        case for case in cases if not args.case or case["id"] in args.case
    ]
    if not cases:
        names = ", ".join(str(folder) for folder, _ in folders)
        raise CaseError(f"no cases in {names}")
    for case in cases:
        case["skill_texts"] = list_skill_texts(case, args, top)
    return cases


def read_case(path: Path, skill: str | None) -> dict:
    """Read and check one case file.

    Args:
        path: A file in ``cases/``, or in a skill's ``evals/``.
        skill: The skill whose ``evals/`` holds the file, or None.

    Returns:
        The case's fields plus ``skill`` and ``case_hash``.

    Raises:
        CaseError: The file is not a case.
    """
    required = REQUIRED | {"rule" if skill else "rules"}
    try:
        case = json.loads(path.read_text(encoding="utf-8"))
    except ValueError as error:
        raise CaseError(f"{path}: {error}") from error
    if not isinstance(case, dict) or not required <= set(case):
        raise CaseError(f"{path}: needs fields {sorted(required)}")
    unknown = sorted(set(case) - required - OPTIONAL)
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
    case["skill"] = skill
    case["case_hash"] = hashlib.sha256(path.read_bytes()).hexdigest()[:12]
    return case


def list_skill_texts(
    case: dict, args: argparse.Namespace, top: Path | None
) -> list[dict]:
    """Return each skill text of the case, a set of runs on one rules text.

    The first skill text, "new", gets the current rules: the case's rules
    files, or its skill's SKILL.md in the checkout. ``--at`` adds
    "old", that SKILL.md at the given commit, and ``--without`` adds
    "none", with no rules.

    Args:
        case: A case from ``read_case``.
        args: The command-line options.
        top: The checkout's top folder for a skill case, or None for
            a case in the trials folder.

    Returns:
        Each skill text from ``make_skill_text``.

    Raises:
        CaseError: A rules file is missing, or git cannot read the old
            text.
    """
    if top is None:
        paths = [Path(name).expanduser() for name in case["rules"]]
        missing = [str(path) for path in paths if not path.is_file()]
        if missing:
            raise CaseError(f"{case['id']}: missing rules files {missing}")
        files = [(n, p.read_bytes()) for n, p in zip(case["rules"], paths)]
        skill_texts = [make_skill_text("new", files, None)]
    else:
        name = f"ai/skills/{case['skill']}/SKILL.md"
        # An uncommitted SKILL.md comes from no commit, so log none.
        changed = run_git(top, "status", "--porcelain", "--", name).strip()
        head = None if changed else run_git(top, "rev-parse", "HEAD").strip()
        text = [(name, (top / name).read_bytes())]
        skill_texts = [make_skill_text("new", text, head)]
        if args.at:
            ref = f"{args.at}^{{commit}}"
            commit = run_git(top, "rev-parse", "--verify", ref).strip()
            old = run_git(top, "show", f"{commit}:{name}")
            old_text = [(name, old.encode())]
            skill_texts.append(make_skill_text("old", old_text, commit))
    if args.without:
        skill_texts.append(make_skill_text("none", [], None))
    return skill_texts


def make_skill_text(
    name: str, files: list[tuple[str, bytes]], commit: str | None
) -> dict:
    """Join one skill text's rules files into one text and hash them as listed.

    The hash covers each file name as written, so it stays the same on
    another computer with the same rules. It also covers the no-tools
    note every prompt gets.

    Args:
        name: The skill text's name, "new", "old" or "none".
        files: Each rules file's name and contents, in order.
        commit: The commit the rules came from, or None.

    Returns:
        The skill text's name, rules text, commit and the first 12 hex digits
        of the rules' sha256.
    """
    digest = hashlib.sha256(f"{NO_TOOLS_NOTE}\n".encode())
    parts = []
    for file, data in files:
        digest.update(f"{file}\n{len(data)}\n".encode())
        digest.update(data)
        parts.append(f"===== {file} =====\n{data.decode()}\n")
    return {
        "name": name,
        "text": "\n".join(parts),
        "hash": digest.hexdigest()[:12],
        "commit": commit,
    }


def run_git(top: Path, *args: str) -> str:
    """Run git in a folder and return its output.

    Raises:
        CaseError: Git fails, such as for a commit that does not exist.
    """
    done = subprocess.run(
        ["git", *args], cwd=top, capture_output=True, text=True
    )
    if done.returncode != 0:
        raise CaseError(f"git {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout


def read_saved_runs(log: Path) -> list[dict]:
    """Return every line of ``results.jsonl``, oldest first."""
    if not log.exists():
        return []
    lines = log.read_text(encoding="utf-8").splitlines()
    return [json.loads(line) for line in lines]


def find_saved_runs(
    saved: list[dict], case: dict, skill_text: dict, args: argparse.Namespace
) -> list[dict]:
    """Return N saved runs of the "none" skill text to use instead of new ones.

    A run with no rules depends only on the case and the model, so runs
    of the same case file, model and effort give the same evidence. A
    run whose ``opencode`` call failed, such as by a timeout, holds no
    answer, so it is never reused.

    Returns:
        The last N matching runs, or an empty list when fewer are
        saved or the skill text has rules.
    """
    if skill_text["name"] != "none":
        return []
    found = [
        run
        for run in saved
        if run["case_hash"] == case["case_hash"]
        and run["rules_hash"] == skill_text["hash"]
        and (run["model"], run["variant"]) == (args.model, args.variant)
        and not any(r.startswith("opencode ") for r in run["reasons"])
    ]
    return found[-args.n :] if len(found) >= args.n else []


def read_opencode_version() -> str | None:
    """Return what ``opencode --version`` prints, or None when it fails."""
    done = subprocess.run(
        ["opencode", "--version"], capture_output=True, text=True
    )
    return done.stdout.strip() if done.returncode == 0 else None


def summarize_case(
    case: dict, graded: list[tuple[dict, list[dict], bool]]
) -> str:
    """Return the case's summary line, ending with the first failure.

    One skill text gives its passed runs and rules hash. Several give
    each skill text's passed runs, marking reused ones "saved". A case
    that passed every run of every skill text is marked, since it cannot
    show what the rules change.

    Args:
        case: A case from ``load_cases``.
        graded: Each skill_text, its runs and whether they were reused.

    Returns:
        One line, such as ``plan-owner new 3/3 none 1/3 - none:
        missed pass /owner/``.
    """
    failures = [
        (skill_text["name"], reason)
        for skill_text, runs, _ in graded
        for run in runs
        for reason in run["reasons"][:1]
    ]
    if len(graded) == 1:
        [(skill_text, runs, _)] = graded
        line = f"{case['id']} {len(runs) - len(failures)}/{len(runs)}"
        line += f" rules {skill_text['hash']}"
        return f"{line} - {failures[0][1]}" if failures else line
    parts = [case["id"]]
    for skill_text, runs, reused in graded:
        passed = sum(run["passed"] for run in runs)
        parts.append(f"{skill_text['name']} {passed}/{len(runs)}")
        parts += ["saved"] if reused else []
    line = " ".join(parts)
    if not failures:
        return f"{line} - every run passed, so no difference"
    return f"{line} - {failures[0][0]}: {failures[0][1]}"


def run_trial(
    case: dict,
    skill_text: dict,
    number: int,
    args: argparse.Namespace,
    answers: Path,
) -> dict:
    """Run one trial in a new empty folder, grade it and save its answer.

    Args:
        case: A case from ``load_cases``.
        skill_text: One of the case's skill_texts, whose rules the run gets.
        number: The run's number, from 1.
        args: The command-line options.
        answers: Folder for this call's answer files.

    Returns:
        The line to add to ``results.jsonl``.
    """
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="trial-") as folder:
        answer, reasons = ask_model(case, skill_text, Path(folder), args)
    if not reasons:
        reasons = grade(case, answer)
    path = answers / f"{case['id']}-{skill_text['name']}-{number}.txt"
    path.write_text(answer, encoding="utf-8")
    return {
        "time": datetime.datetime.now().isoformat(timespec="seconds"),
        "case": case["id"],
        "run": number,
        "skill_text": skill_text["name"],
        "model": args.model,
        "variant": args.variant,
        "rules_hash": skill_text["hash"],
        "case_hash": case["case_hash"],
        "commit": skill_text["commit"],
        "passed": not reasons,
        "reasons": reasons,
        "seconds": round(time.monotonic() - started, 1),
        "answer": str(path),
    }


def ask_model(
    case: dict, skill_text: dict, scratch: Path, args: argparse.Namespace
) -> tuple[str, list[str]]:
    """Send the skill text's rules and the prompt to ``opencode run``.

    Returns:
        The answer text, and one reason when the run failed.
    """
    model = args.model + (f"#{args.variant}" if args.variant else "")
    command = ["opencode", "run", "--standalone", "-m", model]
    command += ["--format", "json"]
    command.append(f"{skill_text['text']}\n{NO_TOOLS_NOTE}\n{case['prompt']}")
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
