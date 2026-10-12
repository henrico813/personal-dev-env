"""List the skills an agent read before and after its first edit.

Examples:
    python3 ai/evals/skill_audit.py ses_ecea4232bffeZhbnmQ0LmVBFZn
    python3 ai/evals/skill_audit.py --vibe RUN_DIR
"""

import argparse
import json
import re
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

DB = Path.home() / ".local/share/opencode/opencode.db"
# A read of a skill missing here failed, so it does not count. Vibe workers
# read the same skills from /vibe-home/.agents/skills in their container.
INSTALLED = Path.home() / ".agents/skills"
# Installed skills only; repository copies such as ai/skills/x/SKILL.md
# are review material, not loads.
SKILL = re.compile(r"\.(?:agents|claude)/skills/([\w${},-]+)/SKILL\.md")
# Only a command that reads a file counts, so a path quoted in a document
# the agent writes is not a load.
READER = re.compile(r"\s*(?:(?:do|then)\s+)*(?:cat|head|tail|sed|nl|less)\b")
LOOP = re.compile(r"\bfor\s+(\w+)\s+in\s+([^;]+?)\s*;\s*do\b")
# A heredoc body is data the command writes or runs elsewhere.
HEREDOC = re.compile(
    r"<<-?\s*['\"]?(\w+)['\"]?[^\n]*\n.*?^\s*\1$", re.M | re.S
)
QUOTED = re.compile(r"'[^']*'|\"[^\"]*\"")
START = r"(?:^|[;&|(\n]\s*|\b(?:do|then)\s+)"
WRITES = re.compile(
    START + r"(?:planner\s+(?:new|patch)|git\s+commit"
    r"|git\s+apply(?![^;&|\n]*--(?:check|stat))"
    r"|vibe\s+run(?![^;&|\n]*--help))\b"
)
PUSH = re.compile(START + r"git\s+(?:-C\s+\S+\s+)?push\b")


def skills_read(name: str, args: dict) -> list[str]:
    """Return the installed skills one tool call reads.

    Expands ``for s in a b; do cat ~/.agents/skills/$s/SKILL.md; done``
    and ``~/.agents/skills/{a,b}/SKILL.md`` to ``a`` and ``b``.

    Args:
        name: Tool name, such as ``skill``, ``read``, or ``shell``.
        args: The tool call's input.

    Returns:
        Skill names in the order the call reads them.
    """
    if name == "skill":
        return [str(args.get("id") or args.get("name"))]
    if name == "read":
        return SKILL.findall(str(args.get("path") or args.get("filePath")))
    command = HEREDOC.sub("", str(args.get("command", "")))
    loops = {var: words.split() for var, words in LOOP.findall(command)}
    names = []
    for part in re.split(r"[;&|\n]", command):
        for raw in SKILL.findall(part) if READER.match(part) else []:
            if raw.startswith("$"):
                names += [
                    w.strip("'\"") for w in loops.get(raw.strip("${}"), [])
                ]
            else:
                names += raw.strip("{}").split(",")
    return names


def audit(calls: list[tuple[str, dict]]) -> str:
    """Summarize skill reads around the first edit and any ``git push``.

    The first edit is an edit, write, or patch tool call, or a shell
    command such as ``planner new``, ``git commit``, or ``vibe run``.
    Skills not installed on this machine are left out.

    Args:
        calls: Tool name and input pairs in call order.

    Returns:
        One line: skills read before the first edit, skills first read
        after it, and whether ``git push`` ran.
    """
    before, after, pushed, edited = [], [], False, False
    for name, args in calls:
        command = HEREDOC.sub("", str(args.get("command", "")))
        command = QUOTED.sub("''", command)
        for skill in skills_read(name, args):
            installed = (INSTALLED / skill / "SKILL.md").is_file()
            if installed and skill not in before + after:
                (after if edited else before).append(skill)
        pushed = pushed or bool(PUSH.search(command))
        edited = edited or name in ("edit", "write", "patch")
        edited = edited or bool(WRITES.search(command))
    return (
        f"before={','.join(before) or '-'} after={','.join(after) or '-'}"
        f" push={'yes' if pushed else 'no'}"
    )


def session_calls(db: Path, session: str) -> list[tuple[str, dict]]:
    """Read one OpenCode session's tool calls in order; never writes.

    Exits with an error when the session has no records, so a mistyped ID
    is not reported as an agent that read nothing.
    """
    with closing(sqlite3.connect(f"file:{db}?mode=ro", uri=True)) as conn:
        rows = conn.execute(
            "select data from session_message where session_id = ?"
            " and type = 'assistant' order by seq",
            (session,),
        ).fetchall()
    if not rows:
        sys.exit(f"{session}: no records in {db}")
    parts = [p for (d,) in rows for p in json.loads(d).get("content") or []]
    return [
        (p["name"], p.get("state", {}).get("input") or {})
        for p in parts
        if p.get("type") == "tool"
    ]


def vibe_calls(run_dir: Path) -> list[tuple[str, dict]]:
    """Read a Vibe worker's tool calls from the run's events.jsonl."""
    lines = (run_dir / "events.jsonl").read_text().splitlines()
    events = [json.loads(line) for line in lines]
    return [
        (p["name"], p.get("arguments") or {})
        for e in events
        if e.get("type") == "message_end"
        and e["message"].get("role") == "assistant"
        for p in e["message"].get("content") or []
        if p.get("type") == "toolCall"
    ]


def main() -> None:
    """Print one line per OpenCode session and Vibe run."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("sessions", nargs="*", help="OpenCode session IDs")
    parser.add_argument("--vibe", type=Path, action="append", default=[])
    parser.add_argument("--db", type=Path, default=DB)
    args = parser.parse_args()
    for session in args.sessions:
        print(f"{session}: {audit(session_calls(args.db, session))}")
    for run_dir in args.vibe:
        print(f"{run_dir.name}: {audit(vibe_calls(run_dir))}")


if __name__ == "__main__":
    main()
