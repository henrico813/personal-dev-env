"""Pick each agent's model from its job kind, and log the pick.

``routing.json``, one folder up, maps each job kind to an effort and
an ordered list of models per tool. A pick prints one JSON line with
the first model the caller can use, its effort and a run ID. It adds
one line to ``~/.local/state/pde/route.log``. ``audit`` later pairs
that line with the subagent start that carries the run ID.

Read ``main`` first, then ``pick_model``, ``check_machine`` and
``audit_starts``, one per command. Their helpers follow them.

Examples:
    python3 pde_route.py plan-review --start opencode-subagent
    python3 pde_route.py local-read --start opencode-cli \\
        --check-failed goog/qwen3.8-flash
    python3 pde_route.py check
    python3 pde_route.py audit --since 1d

Exit codes: 1 when no model is left, a ``--model`` name is not on
this machine, ``check`` finds a kind with no model, or ``audit``
flags a start. 2 for an unknown kind, a bad table, an unreadable
model list, a Claude Code start on a kind with no Claude Code list,
or an audit store that yields no start.

Environment:
    PDE_ROUTE_TABLE: path of a table to read instead of routing.json.
    PDE_ROUTE_TIMEOUT: seconds that replace every time limit on
        ``opencode`` and ``claude`` calls, such as the 60-second probe.
"""

import argparse
import contextlib
import json
import os
import re
import secrets
import signal
import sqlite3
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

STARTS = ("opencode-subagent", "opencode-cli", "claude-agent", "claude-cli")
SUBAGENT_STARTS = ("opencode-subagent", "claude-agent")
EFFORTS = ("none", "low", "medium", "high")
KIND_FIELDS = ("effort", "skills", "opencode", "claude")
PROBE_SECONDS = 60.0
REFRESH_SECONDS = 20.0
REFRESH_TRIES = 15
COPY_FRESH_SECONDS = 600
STORE_BUSY_SECONDS = 10.0
PROBE_PROMPT = "reply OK"
RUN_ID = re.compile(r"\brt-[0-9a-f]{6}\b")
OPENCODE_NAME = re.compile(r"[^/\s#]+/[^\s#]+")
CLAUDE_ALIAS = re.compile(r"[^/\s#]+")
NO_EFFORT = ("", "default", "none")

Table = dict[str, dict[str, Any]]


class RouteError(Exception):
    """A failure that ends the command with a message and an exit code."""

    def __init__(self, code: int, message: str) -> None:
        """Keep the exit code beside the message.

        Args:
            code: 1 or 2, as the module docstring lists.
            message: The text printed to stderr.
        """
        super().__init__(message)
        self.code = code


def main(argv: list[str] | None = None) -> int:
    """Run the command that argv names and return its exit code.

    The first word picks the command. ``check`` and ``audit`` are
    commands, and any other word is a job kind to pick for.
    """
    args = sys.argv[1:] if argv is None else argv
    command = args[0] if args else ""
    try:
        if command == "check":
            parse_check_args(args[1:])
            return check_machine(load_table(find_table_path()))
        if command == "audit":
            since = parse_audit_args(args[1:])
            load_table(find_table_path())
            return audit_starts(since)
        pick_args = parse_pick_args(args)
        return pick_model(pick_args, load_table(find_table_path()))
    except RouteError as error:
        print(f"pde_route: {error}", file=sys.stderr)
        return error.code


def pick_model(args: argparse.Namespace, table: Table) -> int:
    """Print and log the model for one agent start.

    Args:
        args: The kind, ``--start``, ``--model``, ``--failed`` and
            ``--check-failed`` values.
        table: The checked routing table.

    Returns:
        0 after printing the pick.

    Raises:
        RouteError: With code 1 when no model is left or a
            ``--model`` name is not listed, and 2 when the kind or
            its Claude Code list is missing or the model list cannot
            be read.
    """
    entry = find_kind(table, args.kind)
    effort = entry["effort"]
    on_opencode = args.start.startswith("opencode-")
    if not on_opencode and "claude" not in entry:
        raise RouteError(
            2,
            f"kind {args.kind} has no Claude Code list; "
            "start it with opencode-cli",
        )
    models = (
        ModelList(force_refresh=bool(args.failed)) if on_opencode else None
    )
    skipped: list[dict[str, str]] = []
    if args.model:
        name, effort = pick_user_model(args.model, effort, models)
        reason = "user"
    else:
        if on_opencode:
            groups = entry["opencode"]
        else:
            groups = [[alias] for alias in entry["claude"]]
        name, skipped = pick_table_model(groups, args, effort, models)
        reason = "table"
    pick = format_pick(name, effort, args.start)
    append_log(
        {
            "time": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "run_id": pick["run_id"],
            "kind": args.kind,
            "start": args.start,
            "model": pick["model"],
            "effort": pick.get("effort"),
            "reason": reason,
            "skipped": skipped,
        }
    )
    print(json.dumps(pick))
    return 0


def check_machine(table: Table) -> int:
    """Print each table name a fresh ``opencode models`` lacks.

    A machine without one provider is normal, so a missing name alone
    does not fail the check.

    Returns:
        0 while each kind keeps a listed name, 1 when some kind has
        none.

    Raises:
        RouteError: With code 2 when the model list cannot be read.
    """
    names, problem = refresh_model_copy()
    if not names:
        raise RouteError(2, f"cannot read the model list: {problem}")
    listed = set(names)
    kinds_by_name: dict[str, list[str]] = {}
    for kind, entry in table.items():
        for group in entry["opencode"]:
            for name in group:
                kinds_by_name.setdefault(name, []).append(kind)
    for name, kinds in kinds_by_name.items():
        if name not in listed:
            print(f"not on this machine: {name} ({', '.join(kinds)})")
    empty = [
        kind
        for kind, entry in table.items()
        if not any(
            name in listed for group in entry["opencode"] for name in group
        )
    ]
    for kind in empty:
        print(f"no model on this machine for kind {kind}")
    return 1 if empty else 0


def audit_starts(since: timedelta) -> int:
    """Pair each subagent start in the window with its logged pick.

    Only ``opencode-subagent`` and ``claude-agent`` picks are paired,
    since a ``-cli`` job has no parent call in a store.

    Args:
        since: How far back from now to read starts and picks.

    Returns:
        0 when every start matches its pick, 1 when some start is
        flagged, 2 when the log has subagent picks for a tool in the
        window and that tool's store yields no start.
    """
    cutoff = datetime.now(timezone.utc) - since
    picks = [p for p in read_log() if p["start"] in SUBAGENT_STARTS]
    picks_by_run_id = {pick["run_id"]: pick for pick in picks}
    tools_in_window = {p["start"] for p in picks if p["time"] >= cutoff}
    opencode = read_opencode_starts(cutoff)
    claude = read_claude_starts(cutoff)
    counts = {
        "opencode-subagent": sum(
            1 for s in opencode if s.resumed_run_id is None
        ),
        "claude-agent": len(claude),
    }
    resumes = len(opencode) - counts["opencode-subagent"]
    print(
        f"read {counts['opencode-subagent']} starts and {resumes} resumes "
        f"from OpenCode, {counts['claude-agent']} starts from Claude Code"
    )
    flagged = 0
    for start in opencode + claude:
        problem = find_start_problem(start, picks_by_run_id)
        if problem:
            flagged += 1
            print(
                f'flagged: {start.store} "{start.description[:60]}" '
                f"({start.label}): {problem}"
            )
    layout_problem = False
    for tool in sorted(tools_in_window):
        if counts[tool] == 0:
            layout_problem = True
            print(
                f"pde_route: the log has {tool} picks since "
                f"{cutoff:%Y-%m-%d %H:%M}, but its store yields no start; "
                "its layout may have changed",
                file=sys.stderr,
            )
    if layout_problem:
        return 2
    return 1 if flagged else 0


def parse_pick_args(args: list[str]) -> argparse.Namespace:
    """Read a pick's kind and flags, exiting 2 on a usage error."""
    parser = argparse.ArgumentParser(
        prog="pde_route.py",
        description="Pick an agent's model for a job kind.",
    )
    parser.add_argument("kind", help="job kind from routing.json")
    parser.add_argument("--start", required=True, choices=STARTS)
    parser.add_argument("--model", help="the user's model, checked and logged")
    parser.add_argument(
        "--failed",
        action="append",
        default=[],
        help="a model that failed with a not-found, login or limit error",
    )
    parser.add_argument(
        "--check-failed",
        action="append",
        default=[],
        help="a model whose result failed its check; skips its group",
    )
    return parser.parse_args(args)


def parse_check_args(args: list[str]) -> None:
    """Reject any argument to ``check``, exiting 2 on a usage error."""
    parser = argparse.ArgumentParser(
        prog="pde_route.py check",
        description="List table models this machine lacks.",
    )
    parser.parse_args(args)


def parse_audit_args(args: list[str]) -> timedelta:
    """Read ``audit --since``, exiting 2 on a usage error."""
    parser = argparse.ArgumentParser(
        prog="pde_route.py audit",
        description="Pair subagent starts with their logged picks.",
    )
    parser.add_argument(
        "--since", required=True, type=parse_window, help="such as 12h or 1d"
    )
    return parser.parse_args(args).since


def parse_window(text: str) -> timedelta:
    """Turn ``<N>h`` or ``<N>d`` into a time span."""
    match = re.fullmatch(r"(\d+)([hd])", text)
    if not match:
        raise argparse.ArgumentTypeError("use <N>h or <N>d, such as 1d")
    count = int(match.group(1))
    return (
        timedelta(hours=count)
        if match.group(2) == "h"
        else timedelta(days=count)
    )


def find_table_path() -> Path:
    """Return ``PDE_ROUTE_TABLE``, or routing.json beside this folder."""
    override = os.environ.get("PDE_ROUTE_TABLE")
    if override:
        return Path(override)
    return Path(__file__).resolve().parent.parent / "routing.json"


def load_table(path: Path) -> Table:
    """Read the table and check each kind's fields.

    Raises:
        RouteError: With code 2, naming the kind and the field, when
            the table is unreadable or has a bad entry.
    """
    try:
        table = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise RouteError(
            2, f"cannot read the table {path}: {error.strerror}"
        ) from error
    except ValueError as error:
        raise RouteError(2, f"bad table {path}: {error}") from error
    if not isinstance(table, dict) or not table:
        raise RouteError(2, f"bad table {path}: it must map kinds to entries")
    for kind, entry in table.items():
        problem = find_entry_problem(entry)
        if problem:
            field, message = problem
            raise RouteError(
                2, f"bad table {path}: kind {kind}, field {field}: {message}"
            )
    return table


def find_entry_problem(entry: object) -> tuple[str, str] | None:
    """Return the first bad field of one kind and why, or None."""
    if not isinstance(entry, dict):
        return "(entry)", "must be an object"
    for field in entry:
        if field not in KIND_FIELDS:
            return field, "is not a known field"
    for field in ("effort", "skills", "opencode"):
        if field not in entry:
            return field, "is missing"
    if entry["effort"] not in EFFORTS:
        return "effort", f"must be one of {', '.join(EFFORTS)}"
    skills = entry["skills"]
    if not isinstance(skills, list) or not all(
        isinstance(skill, str) and skill for skill in skills
    ):
        return "skills", "must be a list of skill folder names"
    groups = entry["opencode"]
    if not isinstance(groups, list) or not groups:
        return "opencode", "must be a non-empty list of groups"
    for group in groups:
        if not isinstance(group, list) or not group:
            return "opencode", "each group must be a non-empty list"
        for name in group:
            if not isinstance(name, str) or not OPENCODE_NAME.fullmatch(name):
                return "opencode", f"{name!r} is not a provider/model name"
    if "claude" not in entry:
        return None
    aliases = entry["claude"]
    if not isinstance(aliases, list) or not aliases:
        return "claude", "must be a non-empty list of aliases"
    for alias in aliases:
        if not isinstance(alias, str) or not CLAUDE_ALIAS.fullmatch(alias):
            return "claude", f"{alias!r} is not a Claude Code alias"
    return None


def find_kind(table: Table, kind: str) -> dict[str, Any]:
    """Return the kind's entry, or exit 2 listing the known kinds."""
    if kind not in table:
        raise RouteError(
            2, f"unknown kind {kind}; known kinds: {', '.join(table)}"
        )
    return table[kind]


def pick_table_model(
    groups: list[list[str]],
    args: argparse.Namespace,
    effort: str,
    models: "ModelList | None",
) -> tuple[str, list[dict[str, str]]]:
    """Return the first usable name in the groups and each skipped one.

    Args:
        groups: The kind's groups for the start's tool, in order.
        args: The start and the ``--failed`` and ``--check-failed``
            names, each with any ``#`` suffix.
        effort: The kind's effort, which a probe runs at.
        models: The machine's OpenCode list, or None for Claude Code.

    Raises:
        RouteError: With code 1, listing each name and why it was
            skipped, when no name is left.
    """
    failed = {strip_effort(name) for name in args.failed}
    check_failed = {strip_effort(name) for name in args.check_failed}
    skipped: list[dict[str, str]] = []
    for group in groups:
        group_failed = any(name in check_failed for name in group)
        for name in group:
            if group_failed:
                reason = "its group failed a check"
            elif name in failed:
                reason = "failed"
            elif models is not None and not models.contains(name):
                reason = "not on this machine"
            elif args.start.endswith("-cli") and not probe_model(
                args.start,
                add_effort(name, effort) if models is not None else name,
            ):
                reason = "did not answer"
            else:
                return name, skipped
            skipped.append({"model": name, "reason": reason})
    lines = "".join(f"\n  {s['model']}: {s['reason']}" for s in skipped)
    raise RouteError(1, f"no model left for kind {args.kind}:{lines}")


def pick_user_model(
    model: str, effort: str, models: "ModelList | None"
) -> tuple[str, str]:
    """Return the user's model and effort after checking the name.

    A ``#`` suffix sets the effort, and a bare name gets the kind's
    effort. Claude Code aliases pass, since nothing lists them.

    Raises:
        RouteError: With code 1 when the OpenCode name is not listed.
    """
    name, _, user_effort = model.partition("#")
    if models is not None and not models.contains(name):
        raise RouteError(1, f"model {name} is not on this machine")
    return name, user_effort or effort


def format_pick(name: str, effort: str, start: str) -> dict[str, str]:
    """Build the printed line, with the effort in the form the start takes.

    The ``Agent`` tool takes no effort, so a ``claude-agent`` line has
    none. ``claude --effort`` takes no ``none``, so a ``claude-cli``
    line leaves that effort out.
    """
    run_id = f"rt-{secrets.token_hex(3)}"
    if start.startswith("opencode-"):
        return {
            "model": add_effort(name, effort),
            "effort": effort,
            "run_id": run_id,
        }
    if start == "claude-cli" and effort != "none":
        return {"model": name, "effort": effort, "run_id": run_id}
    return {"model": name, "run_id": run_id}


def add_effort(name: str, effort: str) -> str:
    """Return the OpenCode name with its ``#<effort>`` variant."""
    return name if effort == "none" else f"{name}#{effort}"


def strip_effort(name: str) -> str:
    """Drop a ``#`` suffix, so a printed name matches the table."""
    return name.partition("#")[0]


class ModelList:
    """The OpenCode models this machine lists, from a cached copy.

    The copy at ``~/.cache/pde/opencode-models.txt`` is refreshed at
    most once per pick. A name missing from a copy at least 10 minutes
    old triggers the refresh, and ``--failed`` forces it.
    """

    def __init__(self, force_refresh: bool) -> None:
        """Read the copy, if any, without running ``opencode``."""
        self.path = find_copy_path()
        self.force_refresh = force_refresh
        self.refreshed = False
        self.names: set[str] | None = None
        self.age_seconds = 0.0
        with contextlib.suppress(FileNotFoundError):
            text = self.path.read_text(encoding="utf-8")
            self.names = set(text.split())
            self.age_seconds = time.time() - self.path.stat().st_mtime

    def contains(self, name: str) -> bool:
        """Report whether the machine lists name, refreshing as needed.

        Raises:
            RouteError: With code 2 when there is no copy and the
                refresh fails.
        """
        if self.names is None or (self.force_refresh and not self.refreshed):
            self.refresh()
        assert self.names is not None
        stale = self.age_seconds >= COPY_FRESH_SECONDS
        if name not in self.names and stale and not self.refreshed:
            self.refresh()
        return name in self.names

    def refresh(self) -> None:
        """Replace the names with a fresh list, or keep the old copy.

        Raises:
            RouteError: With code 2 when the refresh fails and there
                is no copy to fall back to.
        """
        self.refreshed = True
        names, problem = refresh_model_copy()
        if names:
            self.names = set(names)
            self.age_seconds = 0.0
            return
        if self.names is None:
            raise RouteError(2, f"cannot read the model list: {problem}")
        print(
            f"pde_route: warning: cannot refresh the model list ({problem}); "
            f"using the old copy at {self.path}",
            file=sys.stderr,
        )


def refresh_model_copy() -> tuple[list[str], str]:
    """Run ``opencode models`` and save a non-empty answer as the copy.

    OpenCode lists nothing until the folder finishes loading, so an
    empty answer is retried once a second, up to 15 tries within the
    20-second limit.

    Returns:
        The listed names and an empty string, or no names and why.
    """
    limit = read_time_limit(REFRESH_SECONDS)
    deadline = time.monotonic() + limit
    for _ in range(REFRESH_TRIES):
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            code, output, error = run_bounded(
                ["opencode", "models"], remaining
            )
        except OSError as failure:
            return [], f"cannot run opencode: {failure.strerror}"
        if code is None:
            return [], f"opencode models timed out after {limit:g} s"
        if code != 0:
            return [], f"opencode models {describe_exit(code, error)}"
        names = output.split()
        if names:
            save_copy(find_copy_path(), names)
            return names, ""
        time.sleep(max(0.0, min(1.0, deadline - time.monotonic())))
    return [], f"opencode models listed no models within {limit:g} s"


def save_copy(path: Path, names: list[str]) -> None:
    """Replace the copy through a temporary file and a rename."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}")
    temporary.write_text(
        "".join(f"{name}\n" for name in names), encoding="utf-8"
    )
    os.replace(temporary, path)


def probe_model(start: str, name: str) -> bool:
    """Send "reply OK" to the model and report whether it answered.

    Args:
        start: ``opencode-cli`` or ``claude-cli``.
        name: The OpenCode name with its effort, or a Claude Code
            alias.
    """
    if start == "opencode-cli":
        command = ["opencode", "run", "--standalone", "--format", "json"]
        command += ["-m", name, PROBE_PROMPT]
    else:
        command = ["claude", "-p", "--model", name]
        command += ["--output-format", "json", PROBE_PROMPT]
    try:
        code, output, _ = run_bounded(command, read_time_limit(PROBE_SECONDS))
    except OSError:
        return False
    if code != 0:
        return False
    if start == "opencode-cli":
        return has_opencode_reply(output)
    return has_claude_result(output)


def has_opencode_reply(output: str) -> bool:
    """Report whether ``opencode run`` events hold reply text and no error."""
    replied = False
    for line in output.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        if event.get("type") == "error":
            return False
        part = event.get("part")
        if event.get("type") == "text" and isinstance(part, dict):
            replied = replied or bool(str(part.get("text") or "").strip())
    return replied


def has_claude_result(output: str) -> bool:
    """Report whether ``claude -p`` JSON output is a non-empty success."""
    try:
        answer = json.loads(output)
    except ValueError:
        return False
    if not isinstance(answer, dict) or answer.get("is_error") is not False:
        return False
    return bool(str(answer.get("result") or "").strip())


def run_bounded(
    command: list[str], limit: float
) -> tuple[int | None, str, str]:
    """Run a command with no input, killing it after limit seconds.

    The command gets its own process group, so the kill also stops a
    child that still holds its output open.

    Returns:
        The exit code, or None after a timeout, then stdout and stderr.
    """
    with subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    ) as process:
        try:
            output, error = process.communicate(timeout=limit)
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


def describe_exit(code: int, error: str) -> str:
    """Say how a command exited, with its last line of stderr."""
    last = error.strip().splitlines()[-1:] or [""]
    return f"exited {code} {last[0]}".rstrip()


def read_time_limit(default: float) -> float:
    """Return ``PDE_ROUTE_TIMEOUT`` in seconds, or the default limit."""
    override = os.environ.get("PDE_ROUTE_TIMEOUT")
    if not override:
        return default
    try:
        return float(override)
    except ValueError:
        raise RouteError(
            2, "PDE_ROUTE_TIMEOUT must be a number of seconds"
        ) from None


def find_copy_path() -> Path:
    """Return the path of the cached ``opencode models`` answer."""
    return (
        find_xdg_dir("XDG_CACHE_HOME", ".cache")
        / "pde"
        / "opencode-models.txt"
    )


def find_log_path() -> Path:
    """Return the path of the pick log."""
    return find_xdg_dir("XDG_STATE_HOME", ".local/state") / "pde" / "route.log"


def find_xdg_dir(variable: str, default: str) -> Path:
    """Return the folder an XDG variable names, or its default in home."""
    value = os.environ.get(variable)
    return Path(value) if value else Path.home() / default


def append_log(record: dict[str, Any]) -> None:
    """Append one JSON line to the pick log in a single write.

    Several sessions may pick at once. One write to a file opened for
    appending keeps their lines whole.
    """
    path = find_log_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    line = (json.dumps(record) + "\n").encode()
    descriptor = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o644)
    try:
        os.write(descriptor, line)
    finally:
        os.close(descriptor)


def read_log() -> list[dict[str, Any]]:
    """Return every pick in the log, with ``time`` read as a datetime.

    A line that is not a pick, such as one cut short, is skipped.
    """
    try:
        text = find_log_path().read_text(encoding="utf-8")
    except FileNotFoundError:
        return []
    picks = []
    for line in text.splitlines():
        try:
            pick = json.loads(line)
            pick["time"] = datetime.fromisoformat(pick["time"])
        except (ValueError, TypeError, KeyError):
            continue
        fields = (pick.get(name) for name in ("run_id", "start", "model"))
        if pick["time"].tzinfo and all(isinstance(f, str) for f in fields):
            picks.append(pick)
    return picks


@dataclass
class Start:
    """One subagent start or resume read from a harness's store.

    Attributes:
        store: ``OpenCode`` or ``Claude Code``.
        label: The child session ID or the meta file name, so a reader
            can find the start.
        description: The description the caller gave the subagent.
        model: The model the call set, or None.
        child_models: Each model the child's replies name.
        resumed_run_id: For a resume, the run ID in the description of
            the child's first start, or an empty string when it has
            none. None for a new start.
    """

    store: str
    label: str
    description: str
    model: str | None
    child_models: list[str]
    resumed_run_id: str | None = None


def find_start_problem(
    start: Start, picks_by_run_id: dict[str, dict[str, Any]]
) -> str | None:
    """Return why a start is flagged, or None when it matches its pick."""
    if start.resumed_run_id is not None:
        # A resume runs no pick, so only a model that differs from the
        # first start's pick is wrong.
        first_pick = picks_by_run_id.get(start.resumed_run_id)
        if (
            start.model
            and first_pick
            and not is_same_model(first_pick["model"], start.model)
        ):
            return (
                f"resumed on {start.model}, but its first start logged "
                f"{first_pick['model']}"
            )
        return None
    match = RUN_ID.search(start.description)
    if not match:
        return "no run ID in its description"
    pick = picks_by_run_id.get(match.group())
    if pick is None:
        return f"run ID {match.group()} is not in the log"
    if not start.model:
        return "the call set no model, so it ran on the session's model"
    if not is_same_model(pick["model"], start.model):
        return f"started on {start.model}, but the log gives {pick['model']}"
    for child_model in start.child_models:
        if not is_same_model(pick["model"], child_model):
            return (
                f"the child ran on {child_model}, "
                f"but the log gives {pick['model']}"
            )
    return None


def is_same_model(logged: str, stored: str) -> bool:
    """Compare a logged name with a name a store holds.

    An OpenCode name compares provider, model ID and effort, where a
    variant of ``default`` or ``none`` counts as no effort. A Claude
    Code alias such as ``opus`` matches an ID such as
    ``claude-opus-5-5``.
    """
    if "/" not in logged:
        return stored == logged or stored.startswith(f"claude-{logged}-")
    return split_opencode_name(logged) == split_opencode_name(stored)


def split_opencode_name(name: str) -> tuple[str, str]:
    """Split ``provider/model#variant`` into the name and its effort."""
    base, _, variant = name.partition("#")
    return base, "" if variant in NO_EFFORT else variant


def read_opencode_starts(cutoff: datetime) -> list[Start]:
    """Read the ``subagent`` calls in OpenCode's database since cutoff.

    The database is opened read-only with a busy timeout, since
    OpenCode writes to it while the audit reads. A resume is paired
    with the child's first start, found in the whole database.

    Raises:
        RouteError: With code 2 when the database cannot be read.
    """
    path = (
        find_xdg_dir("XDG_DATA_HOME", ".local/share")
        / "opencode"
        / "opencode.db"
    )
    if not path.exists():
        return []
    cutoff_ms = int(cutoff.timestamp() * 1000)
    try:
        with contextlib.closing(
            sqlite3.connect(
                f"{path.as_uri()}?mode=ro",
                uri=True,
                timeout=STORE_BUSY_SECONDS,
            )
        ) as db:
            rows = db.execute(
                "SELECT time_created, data FROM session_message"
                " WHERE type = 'assistant' AND data LIKE '%\"subagent\"%'"
                " ORDER BY time_created, seq"
            ).fetchall()
            first_descriptions: dict[str, str] = {}
            starts = []
            for created, data in rows:
                for call in list_subagent_calls(data):
                    start = read_opencode_call(call, first_descriptions)
                    if start and created >= cutoff_ms:
                        starts.append(start)
            for start in starts:
                if start.resumed_run_id is None:
                    start.child_models = read_child_models(db, start.label)
                else:
                    first = RUN_ID.search(
                        first_descriptions.get(start.label, "")
                    )
                    start.resumed_run_id = first.group() if first else ""
    except sqlite3.Error as error:
        raise RouteError(
            2, f"cannot read the OpenCode store {path}: {error}"
        ) from error
    return starts


def list_subagent_calls(data: str) -> list[dict[str, Any]]:
    """Return the ``state`` of each ``subagent`` part in one message."""
    try:
        message = json.loads(data)
    except ValueError:
        return []
    content = message.get("content") if isinstance(message, dict) else None
    if not isinstance(content, list):
        return []
    return [
        part["state"]
        for part in content
        if isinstance(part, dict)
        and part.get("name") == "subagent"
        and isinstance(part.get("state"), dict)
    ]


def read_opencode_call(
    state: dict[str, Any], first_descriptions: dict[str, str]
) -> Start | None:
    """Turn one call's state into a start, noting each child's first start.

    Args:
        state: The ``subagent`` part's ``state``.
        first_descriptions: Child session ID to the description of its
            first start. A new start adds its child here.

    Returns:
        The start, or None when the call has no input.
    """
    call_input = state.get("input")
    if not isinstance(call_input, dict):
        return None
    metadata = state.get("metadata")
    if not isinstance(metadata, dict):
        metadata = {}
    description = str(call_input.get("description") or "")
    model = call_input.get("model")
    if not isinstance(model, str):
        model = None
    resumed = call_input.get("sessionID")
    if isinstance(resumed, str) and resumed:
        return Start(
            "OpenCode", resumed, description, model, [], resumed_run_id=""
        )
    child = str(metadata.get("sessionID") or "")
    if child:
        first_descriptions.setdefault(child, description)
    return Start(
        "OpenCode", child or "no child session", description, model, []
    )


def read_child_models(db: sqlite3.Connection, session_id: str) -> list[str]:
    """Return each distinct model a child session's replies name."""
    models: list[str] = []
    rows = db.execute(
        "SELECT data FROM session_message"
        " WHERE session_id = ? AND type = 'assistant'",
        (session_id,),
    )
    for (data,) in rows:
        try:
            model = json.loads(data).get("model")
        except (ValueError, AttributeError):
            continue
        if not isinstance(model, dict) or not model.get("id"):
            continue
        name = f"{model.get('providerID')}/{model['id']}"
        if model.get("variant"):
            name += f"#{model['variant']}"
        if name not in models:
            models.append(name)
    return models


def read_claude_starts(cutoff: datetime) -> list[Start]:
    """Read Claude Code subagent starts whose transcript began since cutoff.

    Each start is a ``*.meta.json`` file under
    ``~/.claude/projects/*/*/subagents/``, with the transcript beside
    it. The transcript's first ``timestamp`` gives the start's time.
    """
    starts = []
    root = Path.home() / ".claude" / "projects"
    for meta_path in sorted(root.glob("*/*/subagents/*.meta.json")):
        stem = meta_path.name.removesuffix(".meta.json")
        began, child_models = read_claude_transcript(
            meta_path.with_name(f"{stem}.jsonl")
        )
        if began is None or began < cutoff:
            continue
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if not isinstance(meta, dict):
            continue
        model = (
            meta.get("model") if isinstance(meta.get("model"), str) else None
        )
        description = str(meta.get("description") or "")
        starts.append(
            Start(
                "Claude Code", meta_path.name, description, model, child_models
            )
        )
    return starts


def read_claude_transcript(path: Path) -> tuple[datetime | None, list[str]]:
    """Return a transcript's first timestamp and each reply model it names."""
    began = None
    models: list[str] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return None, []
    for line in lines:
        try:
            entry = json.loads(line)
        except ValueError:
            continue
        if not isinstance(entry, dict):
            continue
        if began is None and isinstance(entry.get("timestamp"), str):
            with contextlib.suppress(ValueError):
                began = datetime.fromisoformat(
                    entry["timestamp"].replace("Z", "+00:00")
                )
        message = entry.get("message")
        model = message.get("model") if isinstance(message, dict) else None
        # Claude Code names its own notices "<synthetic>", which no
        # model wrote.
        if (
            isinstance(model, str)
            and not model.startswith("<")
            and model not in models
        ):
            models.append(model)
    return began, models


if __name__ == "__main__":
    sys.exit(main())
