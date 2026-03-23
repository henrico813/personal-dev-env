#!/usr/bin/env python3
"""
SessionStart hook: Sync Claude Code docs if stale or missing.

Triggers background sync if docs are >24h old or missing. Non-blocking.
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

CLAUDE_DIR = Path(os.path.expanduser("~/.claude"))
DOCS_DIR = CLAUDE_DIR / "docs"
LAST_SYNC_FILE = DOCS_DIR / ".last_sync"
SYNC_SCRIPT = CLAUDE_DIR / "sync-docs.py"
STALE_THRESHOLD = 24 * 60 * 60  # 24 hours


def needs_sync() -> bool:
    """Check if docs need syncing."""
    if not DOCS_DIR.exists():
        return True

    if not list(DOCS_DIR.glob("*.md")):
        return True

    if not LAST_SYNC_FILE.exists():
        return True

    try:
        last_sync = LAST_SYNC_FILE.stat().st_mtime
        return (time.time() - last_sync) > STALE_THRESHOLD
    except Exception:
        return True


def run_sync():
    """Run the sync script in background and update timestamp."""
    if not SYNC_SCRIPT.exists():
        return

    try:
        # Touch timestamp immediately to prevent re-triggering
        DOCS_DIR.mkdir(exist_ok=True)
        LAST_SYNC_FILE.touch()

        # Run sync in background, don't block session start
        subprocess.Popen(
            ["python3", str(SYNC_SCRIPT)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True
        )
    except Exception:
        pass


def main():
    try:
        # Read stdin (required for hooks)
        json.load(sys.stdin)

        if needs_sync():
            run_sync()

    except Exception:
        # Silent fail
        pass

    sys.exit(0)


if __name__ == "__main__":
    main()
