"""List the skills an OpenCode agent read before and after its first edit.

Example:
    python3 ai/evals/skill_audit.py SESSION_ID
"""

import json
import re
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

DB = Path.home() / ".local/share/opencode/opencode.db"
SKILLS = Path.home() / ".agents/skills"
# A successful read prints the skill's unindented frontmatter, maybe after a
# line number from the read tool or cat -n; the skill tool prints a tag.
NAME = re.compile(
    r"^(?:\s*\d+(?::\s|\t))?name: ([\w-]+)\s*$"
    r'|<skill_content name="([\w-]+)"',
    re.M,
)
# A heredoc body is data, so a push or ">" inside it does not count.
HEREDOC = re.compile(
    r"(<<-?\s*['\"]?(\w+)['\"]?[^\n]*)\n.*?^\s*\2\s*$", re.S | re.M
)
QUOTED = re.compile(r"'[^']*'|\"[^\"]*\"")
# Redirects to /dev/null or another descriptor write no file.
WRITES = re.compile(
    r"\bplanner\s+(?:new|patch)\b|\btee\b|\bsed\s+-i"
    r"|(?<![\d&])>>?(?!\s*(?:&|/dev/null))"
)
PUSH = re.compile(r"\bgit\s+(?:-C\s+\S+\s+)?push\b")


def audit(db: sqlite3.Connection, session: str) -> str:
    """Summarize one session's skill reads around its first edit.

    Exits when the session has no records, so a mistyped ID is not
    reported as an agent that read nothing.

    Args:
        db: Read-only connection to the OpenCode database.
        session: OpenCode session ID.

    Returns:
        One line: skills read before the first edit, skills first read
        after it, and whether ``git push`` ran.
    """
    rows = db.execute(
        "select data from session_message where session_id = ?"
        " and type = 'assistant' order by seq",
        (session,),
    ).fetchall()
    if not rows:
        sys.exit(f"{session}: no records")
    before, after, pushed, edited = [], [], False, False
    for (data,) in rows:
        for call in json.loads(data).get("content") or []:
            if call.get("type") != "tool":
                continue
            state = call.get("state") or {}
            output = "".join(
                c.get("text", "") for c in state.get("content") or []
            )
            for match in NAME.finditer(output):
                skill = match[1] or match[2]
                installed = (SKILLS / skill / "SKILL.md").is_file()
                if installed and skill not in before + after:
                    (after if edited else before).append(skill)
            command = str((state.get("input") or {}).get("command", ""))
            command = QUOTED.sub("''", HEREDOC.sub(r"\1", command))
            pushed = pushed or bool(PUSH.search(command))
            edited = edited or call["name"] in ("edit", "write", "patch")
            edited = edited or bool(WRITES.search(command))
    return (
        f"before={','.join(before) or '-'} after={','.join(after) or '-'}"
        f" push={'yes' if pushed else 'no'}"
    )


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    with closing(sqlite3.connect(f"file:{DB}?mode=ro", uri=True)) as conn:
        for session in sys.argv[1:]:
            print(f"{session}: {audit(conn, session)}")
