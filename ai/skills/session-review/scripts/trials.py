"""Run stored trials of a model against current rules and grade them.

Each case is the situation before a mistake, with the corrected
behavior as its answer key. A case in ``<trials>/cases/*.json`` lists
its rules files. A skill's case, in ``ai/skills/<skill>/evals/*.json``
of the current Git repository, gets that skill's SKILL.md instead.
The script sends the rules and prompt to ``opencode run`` N times and
grades each answer with the case's patterns. A case with ``judge``
questions also needs a yes to each from a blind scorer, the
``--scorer`` model, which sees only the prompt and the answer.
``--at`` and ``--without`` add runs on the skill's old text and on no
rules, to compare.

``--changed BASE`` runs the cases of each skill whose files differ
from BASE. ``--gate`` runs only cases with ``correction`` and exits 1
when one fails more than half its runs on the current text; the merge
check uses both. ``--check-cases`` and ``--last-validated`` run
nothing: one checks the cases and their known answers, the other
reports each skill's last call from the saved results.
``--check-scorer`` runs only the scorer, on each judged case's good
and bad answers. Read ``main`` first.

Example:
    python3 ~/.agents/skills/session-review/scripts/trials.py -n 5
    python3 trials.py --case stale-approval --variant high
    python3 trials.py --skill skill-development --at main --without
    python3 trials.py --all-skills --without
    python3 trials.py --changed origin/main --gate
    python3 trials.py --all-skills --check-cases
    python3 trials.py --all-skills --check-scorer --scorer goog/qwen3.8
    python3 trials.py --last-validated
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
REQUIRED = {"id", "source", "prompt"}
OPTIONAL = {"pass", "fail", "judge", "correction", "good", "bad"}
# Keep the first Review model in model-routing's Models list, at medium.
SCORER = "claude-code/claude-sonnet-5#medium"
# The scorer gets no rules, skill text or model name, so it can judge
# only what the answer says.
SCORER_NOTE = (
    "You check an answer to a task. Read the task and the answer, then"
    " answer each numbered question with yes or no. Reply with one line"
    " per question, such as '1: yes', and nothing else."
)
VERDICT = re.compile(r"^\W*(\d+)\W+(yes|no)\b", re.I | re.M)
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
        1 when a case cannot be used, ``--check-scorer`` gets cases
        but none with judge questions, or ``--gate`` finds a case whose
        "new" skill text failed more than half its runs, else 0,
        whatever the pass rate. No chosen case at all, such as from an
        empty ``--changed``, gives 0.
    """
    args = parse_args(argv)
    if args.last_validated:
        saved = read_saved_runs(args.trials / "results.jsonl")
        for line in list_last_validated(saved):
            print(line)
        return 0
    try:
        cases = load_cases(args)
    except CaseError as error:
        print(f"trials: {error}", file=sys.stderr)
        return 1
    if args.check_cases:
        print(f"{len(cases)} cases ok")
        return 0
    if not cases:
        print("trials: no cases to run")
        return 0
    if args.check_scorer:
        return check_scorer(cases, args)
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
    # A killed run leaves a half line; a new line would glue onto it.
    if log.is_file() and log.stat().st_size:
        with log.open("rb") as handle:
            handle.seek(-1, os.SEEK_END)
            needs_newline = handle.read(1) != b"\n"
        if needs_newline:
            with log.open("a", encoding="utf-8") as handle:
                handle.write("\n")
    blocked = []
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
            # The first skill text is always "new", the text being merged.
            _, runs, _ = graded[0]
            passed, n = sum(run["passed"] for run in runs), len(runs)
            if args.gate and (n - passed) * 2 > n:
                blocked.append(f"blocked: {case['id']} new {passed}/{n}")
    model = args.model + (f"#{args.variant}" if args.variant else "")
    print(f"total {total}/{count} on {model}, answers in {answers}")
    for line in blocked:
        print(line)
    return 1 if blocked else 0


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
        "--all-skills",
        action="store_true",
        help="run the cases of every skill in the checkout that has evals/",
    )
    parser.add_argument(
        "--changed",
        metavar="BASE",
        help="run the cases of each skill whose files differ from BASE",
    )
    parser.add_argument(
        "--gate",
        action="store_true",
        help="run only correction cases; exit 1 when one fails over half",
    )
    parser.add_argument(
        "--at", help="also run each skill case on its SKILL.md at this commit"
    )
    parser.add_argument(
        "--without",
        action="store_true",
        help="also run each case with no rules, reusing saved runs",
    )
    parser.add_argument(
        "--last-validated",
        action="store_true",
        help="print each skill's last call from results.jsonl, run nothing",
    )
    parser.add_argument(
        "--scorer",
        default=SCORER,
        help="model that answers each case's judge questions",
    )
    parser.add_argument(
        "--check-cases",
        action="store_true",
        help="check the chosen cases and their known answers, run nothing",
    )
    parser.add_argument(
        "--check-scorer",
        action="store_true",
        help="score each judged case's good and bad answers N times",
    )
    args = parser.parse_args(argv)
    if args.at and not (args.skill or args.all_skills or args.changed):
        parser.error("--at needs --skill, --all-skills or --changed")
    if args.skill and args.all_skills:
        parser.error("--skill and --all-skills cannot be used together")
    return args


def load_cases(args: argparse.Namespace) -> list[dict]:
    """Read and check every chosen case, so a bad one stops the script early.

    Args:
        args: The command-line options. With ``--skill``,
            ``--all-skills`` or ``--changed``, the cases come from each
            skill's ``evals/`` in the checkout instead of the trials
            folder's ``cases/``.

    Returns:
        The chosen cases, each with its case hash and skill texts
        added. The list is empty only with ``--changed`` or ``--gate``.

    Raises:
        CaseError: A case is broken, its rules are missing, or a
            wanted ID names no case.
    """
    if args.skill or args.all_skills or args.changed:
        top = Path(run_git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
        names = args.skill
        if args.all_skills:
            found = (top / "ai/skills").glob("*/evals")
            names = sorted(folder.parent.name for folder in found)
        elif args.changed:
            names = list_changed_skills(top, args.changed)
        folders = [
            (top / "ai/skills" / name / "evals", name) for name in names
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
    if args.gate:
        cases = [case for case in cases if case.get("correction")]
    if not cases and not (args.changed or args.gate):
        names = ", ".join(str(folder) for folder, _ in folders)
        if not names:
            raise CaseError("no skill has an evals/ folder")
        raise CaseError(f"no cases in {names}")
    for case in cases:
        case["skill_texts"] = list_skill_texts(case, args, top)
    return cases


def list_changed_skills(top: Path, base: str) -> list[str]:
    """Return each skill with an ``evals/`` folder that differs from BASE.

    Uncommitted edits count, but new files git does not track do not,
    since ``git diff`` does not list them.
    """
    out = run_git(top, "diff", "--name-only", "-z", base, "--", "ai/skills")
    names = {Path(path).parts[2] for path in out.split("\0") if path}
    return sorted(
        name for name in names if (top / "ai/skills" / name / "evals").is_dir()
    )


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
    for field in ("rules", "pass", "fail", "judge"):
        value = case.get(field, [])
        if not isinstance(value, list) or not all(
            isinstance(item, str) for item in value
        ):
            raise CaseError(f"{path}: {field} must be a list of strings")
    if not case.get("pass") and not case.get("judge"):
        raise CaseError(f"{path}: needs a pass pattern or a judge question")
    try:
        for pattern in case.get("pass", []) + case.get("fail", []):
            re.compile(pattern)
    except re.error as error:
        raise CaseError(f"{path}: bad pattern {pattern!r}: {error}") from error
    check_known_answers(path, case)
    case["skill"] = skill
    case["case_hash"] = hashlib.sha256(path.read_bytes()).hexdigest()[:12]
    return case


def check_known_answers(path: Path, case: dict) -> None:
    """Check a case's ``good`` and ``bad`` answers against its patterns.

    The good answer must pass. The bad one must fail, unless judge
    questions can still fail it. A correction case blocks merges, and
    a judged one is how ``--check-scorer`` tests a scorer, so both need
    the two answers.

    Raises:
        CaseError: An answer is missing or not text, or the patterns
            grade it wrongly.
    """
    if not isinstance(case.get("correction", False), bool):
        raise CaseError(f"{path}: correction must be true or false")
    needs = case.get("correction") or case.get("judge")
    if needs and not {"good", "bad"} <= set(case):
        raise CaseError(f"{path}: needs good and bad answers")
    for field in ("good", "bad"):
        if not isinstance(case.get(field, ""), str):
            raise CaseError(f"{path}: {field} must be a string")
    if "good" in case and grade(case, case["good"]):
        reason = grade(case, case["good"])[0]
        raise CaseError(f"{path}: good answer fails, {reason}")
    if "bad" in case and not case.get("judge"):
        if not grade(case, case["bad"]):
            raise CaseError(f"{path}: bad answer passes every pattern")


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
    """Return each valid JSON line, oldest first, skipping cut-short ones."""
    if not log.exists():
        return []
    runs = []
    for line in log.read_text(encoding="utf-8").splitlines():
        try:
            runs.append(json.loads(line))
        except ValueError:
            continue
    return runs


def find_saved_runs(
    saved: list[dict], case: dict, skill_text: dict, args: argparse.Namespace
) -> list[dict]:
    """Return N saved runs of the "none" skill text to use instead of new ones.

    A run with no rules depends only on the case, the model and, for a
    judged case, the scorer, so runs that match on all three give the
    same evidence. A run whose ``opencode`` call failed, such as by a
    timeout, holds no answer, so it is never reused.

    Returns:
        The last N matching runs, or an empty list when fewer are
        saved or the skill text has rules.
    """
    if skill_text["name"] != "none":
        return []
    scorer = args.scorer if case.get("judge") else None
    found = [
        run
        for run in saved
        if run["case_hash"] == case["case_hash"]
        and run["rules_hash"] == skill_text["hash"]
        and (run["model"], run["variant"]) == (args.model, args.variant)
        and run.get("scorer") == scorer
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


def list_last_validated(saved: list[dict]) -> list[str]:
    """Return one line per skill on the last call that ran its cases.

    A call is the runs that share one answers folder. The line counts
    only that call's runs of the skill's own text, the "new" skill text,
    so a reused or no-skill run cannot make the skill look better. Lines
    of cases in the trials folder name no skill and are skipped.

    Args:
        saved: The lines of ``results.jsonl``, oldest first.

    Returns:
        Lines sorted by skill, such as ``skill-development 2026-11-20
        goog/qwen3.8-flash 13/15 rules 1a2b3c4d5e6f``.
    """
    runs = [
        run for run in saved if run.get("skill") and run["skill_text"] == "new"
    ]
    last = {run["skill"]: run for run in runs}
    lines = []
    for skill, newest in sorted(last.items()):
        call = Path(newest["answer"]).parent
        same = [
            run
            for run in runs
            if run["skill"] == skill and Path(run["answer"]).parent == call
        ]
        passed = sum(run["passed"] for run in same)
        variant = newest["variant"]
        model = newest["model"] + (f"#{variant}" if variant else "")
        day = newest["time"][:10]
        lines.append(
            f"{skill} {day} {model} {passed}/{len(same)}"
            f" rules {newest['rules_hash']}"
        )
    return lines


def check_scorer(cases: list[dict], args: argparse.Namespace) -> int:
    """Score each judged case's good and bad answers N times and print it.

    The scorer agrees on a run when it passes the good answer, or fails
    the bad one by a verdict rather than a failed call. Run it once per
    scorer and compare the totals before switching scorers.

    Returns:
        1 when no chosen case has judge questions, else 0.
    """
    judged = [case for case in cases if case.get("judge")]
    if not judged:
        print("trials: no chosen case has judge questions", file=sys.stderr)
        return 1
    total = count = 0
    for case in judged:
        parts = [case["id"]]
        for field in ("good", "bad"):
            agreed = 0
            for _ in range(args.n):
                reasons = ask_scorer(case, case[field], args)
                if field == "good":
                    agreed += not reasons
                else:
                    agreed += bool(reasons) and not reasons[0].startswith(
                        "opencode "
                    )
            parts.append(f"{field} {agreed}/{args.n}")
            total += agreed
            count += args.n
        print(" ".join(parts))
    print(f"scorer {args.scorer} agreed {total}/{count}")
    return 0


def run_trial(
    case: dict,
    skill_text: dict,
    number: int,
    args: argparse.Namespace,
    answers: Path,
) -> dict:
    """Run one trial, grade it, ask the scorer and save its answer.

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
    model = args.model + (f"#{args.variant}" if args.variant else "")
    message = f"{skill_text['text']}\n{NO_TOOLS_NOTE}\n{case['prompt']}"
    answer, failure = ask_opencode(model, message, args.timeout)
    reasons = [f"opencode {failure}"] if failure else grade(case, answer)
    # Each scorer call costs a run, so it judges only passing answers.
    if not reasons and case.get("judge"):
        reasons = ask_scorer(case, answer, args)
    path = answers / f"{case['id']}-{skill_text['name']}-{number}.txt"
    path.write_text(answer, encoding="utf-8")
    return {
        "time": datetime.datetime.now().isoformat(timespec="seconds"),
        "case": case["id"],
        "skill": case["skill"],
        "run": number,
        "skill_text": skill_text["name"],
        "model": args.model,
        "variant": args.variant,
        "scorer": args.scorer if case.get("judge") else None,
        "rules_hash": skill_text["hash"],
        "case_hash": case["case_hash"],
        "commit": skill_text["commit"],
        "passed": not reasons,
        "reasons": reasons,
        "seconds": round(time.monotonic() - started, 1),
        "answer": str(path),
    }


def ask_scorer(case: dict, answer: str, args: argparse.Namespace) -> list[str]:
    """Ask the scorer the case's judge questions about one answer.

    The scorer sees only the prompt, the answer and the questions,
    never the skill text, the rules or the model, so it cannot favor
    one skill text.

    Returns:
        Each question it did not answer yes, or how its call failed.
        Empty when the answer passes.
    """
    questions = [
        f"{number}. {question}"
        for number, question in enumerate(case["judge"], start=1)
    ]
    message = "\n".join(
        [
            SCORER_NOTE,
            "===== task =====",
            case["prompt"],
            "===== answer =====",
            answer,
            "===== questions =====",
            *questions,
        ]
    )
    reply, failure = ask_opencode(args.scorer, message, args.timeout)
    if failure:
        return [f"opencode scorer {failure}"]
    verdicts: dict[int, str] = {}
    for number, verdict in VERDICT.findall(reply):
        verdicts.setdefault(int(number), verdict.lower())
    return [
        f"scorer said {verdicts.get(number, 'nothing')} to question {number}"
        for number in range(1, len(case["judge"]) + 1)
        if verdicts.get(number) != "yes"
    ]


def ask_opencode(
    model: str, message: str, timeout: int
) -> tuple[str, str | None]:
    """Send one message to ``opencode run`` in a new empty folder.

    Returns:
        The answer text, and how the call failed, or None.
    """
    command = ["opencode", "run", "--standalone", "-m", model]
    command += ["--format", "json", message]
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
    with tempfile.TemporaryDirectory(prefix="trial-") as folder:
        code, output, error = run_bounded(command, Path(folder), env, timeout)
    if code != 0:
        return "", describe_failure(code, error, timeout)
    return read_answer(output), None


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
        for pattern in case.get("pass", [])
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
