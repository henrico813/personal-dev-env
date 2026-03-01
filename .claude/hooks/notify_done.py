#!/usr/bin/env python3
"""
Stop hook: ntfy notification on task completion.
Only notifies when user is idle (walked away) or detection unavailable.
"""
import json
import shutil
import subprocess
import sys
import time
from pathlib import Path

# Config
IDLE_THRESHOLD = 60      # seconds
DEBOUNCE_SECONDS = 300   # 5 minutes
SUMMARY_CHARS = 80

CACHE_DIR = Path.home() / ".cache" / "claude-notify"
DEBOUNCE_FILE = CACHE_DIR / "last-notify"
LOG_FILE = CACHE_DIR / "notify.log"

# ntfy config
NTFY_URL = "https://ntfy.googungus.com/claude-code"
TOKEN_FILE = Path.home() / ".config" / "ntfy" / "token"


def log(msg: str):
    """Append to log file for debugging."""
    try:
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        with open(LOG_FILE, "a") as f:
            f.write(f"{time.strftime('%H:%M:%S')} {msg}\n")
    except Exception:
        pass


def get_idle_seconds() -> int | None:
    """Get idle time via xprintidle. Returns None if unavailable."""
    if not shutil.which("xprintidle"):
        return None
    try:
        result = subprocess.run(
            ["xprintidle"], capture_output=True, text=True, timeout=2
        )
        if result.returncode == 0:
            return int(result.stdout.strip()) // 1000
    except Exception:
        pass
    return None


def check_debounce() -> bool:
    """Return True if should skip (recently notified)."""
    try:
        if DEBOUNCE_FILE.exists():
            last = int(DEBOUNCE_FILE.read_text().strip())
            return (int(time.time()) - last) < DEBOUNCE_SECONDS
    except Exception:
        pass
    return False


def update_debounce():
    try:
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        DEBOUNCE_FILE.write_text(str(int(time.time())))
    except Exception:
        pass


def get_summary(transcript_path: str) -> str:
    """Extract last assistant message text."""
    try:
        path = Path(transcript_path).expanduser()
        if not path.exists():
            return "Task completed"

        # Read last 50 lines only
        result = subprocess.run(
            ["tail", "-n", "50", str(path)],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode != 0:
            return "Task completed"

        for line in reversed(result.stdout.strip().split("\n")):
            try:
                entry = json.loads(line)
                if entry.get("role") != "assistant":
                    continue
                content = entry.get("content", "")
                if isinstance(content, list):
                    for block in content:
                        if block.get("type") == "text":
                            content = block.get("text", "")
                            break
                    else:
                        continue
                if content:
                    return " ".join(content.split())[:SUMMARY_CHARS]
            except (json.JSONDecodeError, KeyError):
                continue
        return "Task completed"
    except Exception:
        return "Task completed"


def send_ntfy(title: str, message: str):
    """Send notification via curl."""
    token = ""
    if TOKEN_FILE.exists():
        token = TOKEN_FILE.read_text().strip()

    headers = ["-H", f"Title: {title}", "-H", "Priority: 3"]
    if token:
        headers += ["-H", f"Authorization: Bearer {token}"]

    try:
        subprocess.run(
            ["curl", "-sf", "--max-time", "5", *headers, "-d", message, NTFY_URL],
            capture_output=True, timeout=10
        )
    except Exception as e:
        log(f"send failed: {e}")


def main():
    try:
        data = json.load(sys.stdin)
    except Exception:
        return

    if data.get("stop_hook_active"):
        return

    # Check idle time
    idle = get_idle_seconds()
    if idle is not None and idle < IDLE_THRESHOLD:
        log(f"skip: idle {idle}s < {IDLE_THRESHOLD}s")
        return

    # Check debounce
    if check_debounce():
        log("skip: debounce")
        return

    # Send notification
    cwd = data.get("cwd", "")
    project = Path(cwd).name if cwd else "claude"
    summary = get_summary(data.get("transcript_path", ""))

    log(f"notify: {project}")
    send_ntfy(project, summary)
    update_debounce()


if __name__ == "__main__":
    main()
