#!/usr/bin/env python3
"""
UserPromptSubmit hook: Auto-inject Claude Code docs context on help queries.

Detects help queries, searches local docs + user config, returns additionalContext.
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Optional

CLAUDE_DIR = Path(os.path.expanduser("~/.claude"))
DOCS_DIR = CLAUDE_DIR / "docs"
LAST_SYNC_FILE = DOCS_DIR / ".last_sync"
STALE_THRESHOLD = 24 * 60 * 60  # 24 hours in seconds

# Keywords that suggest a Claude Code help query
CONCEPT_KEYWORDS = [
    "hook", "hooks", "command", "commands", "agent", "agents",
    "mcp", "skill", "skills", "settings", "config", "configure",
    "claude code", "claudecode", "prompt", "permission", "permissions",
    "tool", "tools", "model", "context", "memory", "session",
    "bash", "edit", "write", "read", "glob", "grep"
]

QUESTION_PATTERNS = [
    r"\bhow\s+(?:do|does|can|to)\b",
    r"\bwhat\s+(?:is|are|does)\b",
    r"\bwhere\s+(?:is|are|do)\b",
    r"\bwhy\s+(?:does|is|do)\b",
    r"\bcan\s+(?:i|you|claude)\b",
    r"\bexplain\b",
    r"\btell\s+me\s+about\b",
    r"\bshow\s+me\b",
]

MAX_CONTEXT_CHARS = 6000
MAX_OFFICIAL_EXCERPTS = 3
MAX_USER_EXAMPLES = 2


def is_help_query(prompt: str) -> bool:
    """Detect if the prompt is asking for help about Claude Code."""
    prompt_lower = prompt.lower()

    # Check for concept keywords
    has_keyword = any(kw in prompt_lower for kw in CONCEPT_KEYWORDS)
    if not has_keyword:
        return False

    # Check for question patterns or ending with ?
    if prompt.rstrip().endswith("?"):
        return True

    for pattern in QUESTION_PATTERNS:
        if re.search(pattern, prompt_lower):
            return True

    return False


def is_docs_stale() -> bool:
    """Check if docs are older than 24 hours or missing."""
    if not DOCS_DIR.exists() or not list(DOCS_DIR.glob("*.md")):
        return True

    if not LAST_SYNC_FILE.exists():
        return True

    try:
        import time
        last_sync = LAST_SYNC_FILE.stat().st_mtime
        return (time.time() - last_sync) > STALE_THRESHOLD
    except Exception:
        return True


def trigger_background_sync():
    """Start background sync if docs are stale."""
    if not is_docs_stale():
        return

    sync_script = CLAUDE_DIR / "sync-docs.py"
    if not sync_script.exists():
        return

    try:
        # Run sync in background, don't wait
        subprocess.Popen(
            ["python3", str(sync_script)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True
        )
    except Exception:
        pass  # Silent fail


def search_file(path: Path, query_words: list[str]) -> Optional[tuple[str, int]]:
    """Search a file for query words. Returns (excerpt, score) or None."""
    try:
        content = path.read_text(encoding="utf-8")
    except Exception:
        return None

    content_lower = content.lower()
    score = sum(content_lower.count(word) for word in query_words)

    if score == 0:
        return None

    # Extract relevant excerpt around first match
    for word in query_words:
        idx = content_lower.find(word)
        if idx >= 0:
            # Get surrounding context (up to 800 chars)
            start = max(0, idx - 200)
            end = min(len(content), idx + 600)

            # Try to start/end at paragraph boundaries
            excerpt = content[start:end]
            if start > 0:
                newline = excerpt.find("\n")
                if newline > 0 and newline < 50:
                    excerpt = excerpt[newline + 1:]
            if end < len(content):
                newline = excerpt.rfind("\n")
                if newline > len(excerpt) - 50:
                    excerpt = excerpt[:newline]

            return (excerpt.strip(), score)

    return None


def search_docs(prompt: str) -> list[tuple[str, str, int]]:
    """Search official docs. Returns [(filename, excerpt, score)]."""
    results = []
    query_words = [w.lower() for w in prompt.split() if len(w) > 2]

    if not DOCS_DIR.exists():
        return results

    for path in DOCS_DIR.glob("*.md"):
        result = search_file(path, query_words)
        if result:
            excerpt, score = result
            results.append((path.stem, excerpt, score))

    return sorted(results, key=lambda x: -x[2])


def search_user_config(prompt: str) -> list[tuple[str, str, int]]:
    """Search user's commands, agents, hooks. Returns [(type/name, excerpt, score)]."""
    results = []
    query_words = [w.lower() for w in prompt.split() if len(w) > 2]

    search_paths = [
        ("commands", CLAUDE_DIR / "commands", "*.md"),
        ("agents", CLAUDE_DIR / "agents", "*.md"),
        ("hooks", CLAUDE_DIR / "hooks", "*.py"),
    ]

    for config_type, dir_path, pattern in search_paths:
        if not dir_path.exists():
            continue
        for path in dir_path.glob(pattern):
            result = search_file(path, query_words)
            if result:
                excerpt, score = result
                results.append((f"{config_type}/{path.stem}", excerpt, score))

    return sorted(results, key=lambda x: -x[2])


def format_context(docs: list, user_config: list) -> str:
    """Format search results into context string."""
    parts = []
    chars = 0

    # Add official docs first
    for name, excerpt, _ in docs[:MAX_OFFICIAL_EXCERPTS]:
        section = f"## Claude Code Docs: {name}\n\n{excerpt}\n"
        if chars + len(section) > MAX_CONTEXT_CHARS:
            break
        parts.append(section)
        chars += len(section)

    # Add user config examples
    for name, excerpt, _ in user_config[:MAX_USER_EXAMPLES]:
        section = f"## User Config Example: {name}\n\n{excerpt}\n"
        if chars + len(section) > MAX_CONTEXT_CHARS:
            break
        parts.append(section)
        chars += len(section)

    return "\n".join(parts)


def main():
    try:
        input_data = json.load(sys.stdin)
        prompt = input_data.get("prompt", "")

        if not is_help_query(prompt):
            sys.exit(0)

        # Trigger background sync if needed
        trigger_background_sync()

        # Search for relevant content
        docs = search_docs(prompt)
        user_config = search_user_config(prompt)

        if not docs and not user_config:
            sys.exit(0)

        # Format and output context
        context = format_context(docs, user_config)
        if context:
            result = {
                "additionalContext": f"<claude-code-docs>\n{context}</claude-code-docs>"
            }
            print(json.dumps(result))

    except Exception:
        # Silent fail - don't break Claude's workflow
        pass

    sys.exit(0)


if __name__ == "__main__":
    main()
