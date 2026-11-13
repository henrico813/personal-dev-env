"""Run a terminal program on a hidden screen and print what it shows.

Agents use this to look at a TUI or CLI change at a phone size
without asking the user to test it. Each screen is its own tmux
server on the socket ``pde-screen-<name>``, so no command here can
reach the user's tmux or Herdr panes. Read ``main`` first.

Example:
    python3 screen.py start demo --size 40x16 -- glimmer help
    python3 screen.py wait demo Usage
    python3 screen.py stop demo
"""

import argparse
import json
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

SOCKET_PREFIX = "pde-screen-"
SESSION = "screen"
PHONE_SIZE = "40x16"
NAME_PATTERN = re.compile(r"[a-z0-9][a-z0-9-]*")
SIZE_PATTERN = re.compile(r"([1-9][0-9]*)x([1-9][0-9]*)")
# tmux 3.7 blanks a dead pane's screen, so the pane stays open after
# the program exits. $0 is the tmux path and "$@" the program.
HOLD_OPEN = (
    '"$@"; "$0" set-option -g @exit_code "$?"; while :; do sleep 3600; done'
)
SCREEN_FORMAT = (
    "#{pane_width} #{pane_height} #{cursor_x} #{cursor_y} #{@exit_code}"
)
POLL_SECONDS = 0.1


class ScreenError(Exception):
    """A screen command that cannot run, with a message for the agent."""


def main(argv: list[str] | None = None) -> int:
    """Run one screen command and print its result.

    Args:
        argv: Command-line arguments without the program name, or None
            for ``sys.argv``.

    Returns:
        0 on success, 1 when ``wait`` did not find its text, and 2
        when the command could not run.
    """
    args = parse_args(argv)
    try:
        if args.command == "start":
            start_screen(args.name, args.size, args.cwd, args.program)
            print(json.dumps({"name": args.name, "size": args.size}))
        elif args.command == "keys":
            run_tmux(args.name, "send-keys", "-t", SESSION, *args.keys)
        elif args.command == "type":
            run_tmux(args.name, "send-keys", "-l", "-t", SESSION, args.text)
        elif args.command == "show":
            print(json.dumps(read_screen(args.name, args.ansi), indent=2))
        elif args.command == "wait":
            return wait_for_text(args.name, args.text, args.timeout)
        elif args.command == "stop":
            stop_screen(args.name)
    except ScreenError as error:
        print(f"screen.py: {error}", file=sys.stderr)
        return 2
    return 0


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse the subcommand and its arguments.

    Args:
        argv: Command-line arguments without the program name, or None
            for ``sys.argv``.

    Returns:
        The parsed arguments, with the subcommand in ``command``.
    """
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)

    start = commands.add_parser("start", help="run a program on a screen")
    start.add_argument("name", type=check_screen_name)
    start.add_argument(
        "--size",
        type=check_screen_size,
        default=PHONE_SIZE,
        help=f"columns x rows (default {PHONE_SIZE}, a phone)",
    )
    start.add_argument("--cwd", help="folder to run the program in")
    start.add_argument("program", nargs="+", help="program and arguments")

    keys = commands.add_parser("keys", help="press tmux keys, such as Down")
    keys.add_argument("name", type=check_screen_name)
    keys.add_argument("keys", nargs="+")

    typed = commands.add_parser("type", help="type text as written")
    typed.add_argument("name", type=check_screen_name)
    typed.add_argument("text")

    show = commands.add_parser("show", help="print the screen as JSON")
    show.add_argument("name", type=check_screen_name)
    show.add_argument(
        "--ansi", action="store_true", help="keep colors as escape codes"
    )

    wait = commands.add_parser("wait", help="wait until text is on screen")
    wait.add_argument("name", type=check_screen_name)
    wait.add_argument("text")
    wait.add_argument("--timeout", type=float, default=10.0, help="seconds")

    stop = commands.add_parser("stop", help="end the program and screen")
    stop.add_argument("name", type=check_screen_name)
    return parser.parse_args(argv)


def check_screen_name(value: str) -> str:
    """Return value when it is a usable screen name.

    Args:
        value: The name given on the command line.

    Returns:
        value, unchanged.

    Raises:
        argparse.ArgumentTypeError: When value has anything but
            lowercase letters, digits and hyphens.
    """
    if not NAME_PATTERN.fullmatch(value):
        raise argparse.ArgumentTypeError(
            f"{value!r} must be lowercase letters, digits and hyphens"
        )
    return value


def check_screen_size(value: str) -> str:
    """Return value when it reads as columns x rows, such as 40x16.

    Args:
        value: The size given on the command line.

    Returns:
        value, unchanged.

    Raises:
        argparse.ArgumentTypeError: When value is not two positive
            numbers joined by an x.
    """
    if not SIZE_PATTERN.fullmatch(value):
        raise argparse.ArgumentTypeError(
            f"{value!r} must be columns x rows, such as {PHONE_SIZE}"
        )
    return value


def start_screen(
    name: str, size: str, cwd: str | None, program: list[str]
) -> None:
    """Start program on a new screen of size columns x rows.

    Args:
        name: The screen's name.
        size: Columns x rows, such as 40x16.
        cwd: Folder to run program in, or None for this one.
        program: The program and its arguments.

    Raises:
        ScreenError: When a screen with this name is already running,
            or tmux cannot start the program.
    """
    if is_screen_running(name):
        raise ScreenError(
            f"screen {name} is already running; stop it or pick a new name"
        )
    columns, rows = SIZE_PATTERN.fullmatch(size).groups()
    folder = ["-c", cwd] if cwd else []
    run_tmux(
        name,
        "new-session",
        "-d",
        "-s",
        SESSION,
        "-x",
        columns,
        "-y",
        rows,
        *folder,
        "--",
        "sh",
        "-c",
        HOLD_OPEN,
        find_tmux(),
        *program,
    )


def read_screen(name: str, ansi: bool = False) -> dict:
    """Return the screen's size, cursor, exit code and visible lines.

    Args:
        name: The screen's name.
        ansi: Keep colors and styles in the lines as escape codes.

    Returns:
        A dict with ``width``, ``height``, ``cursor``, ``exited``,
        ``exit_code`` and one string per row in ``lines``. tmux drops
        spaces at the end of each line.

    Raises:
        ScreenError: When no screen has this name.
    """
    fields = run_tmux(
        name, "display-message", "-p", "-t", SESSION, SCREEN_FORMAT
    )
    width, height, cursor_x, cursor_y, exit_code = fields.rstrip("\n").split(
        " "
    )
    capture = ["capture-pane", "-p", "-t", SESSION] + (["-e"] if ansi else [])
    lines = run_tmux(name, *capture).split("\n")[: int(height)]
    return {
        "name": name,
        "width": int(width),
        "height": int(height),
        "cursor": {"x": int(cursor_x), "y": int(cursor_y)},
        "exited": exit_code != "",
        "exit_code": int(exit_code) if exit_code else None,
        "lines": lines,
    }


def wait_for_text(name: str, text: str, timeout: float) -> int:
    """Print the screen once text shows in one line, or the wait ends.

    The wait ends early when the program has exited without showing
    text, since its screen will not change again.

    Args:
        name: The screen's name.
        text: Text to find within one line of the screen.
        timeout: Seconds to wait.

    Returns:
        0 when text appeared and 1 when it did not.

    Raises:
        ScreenError: When no screen has this name.
    """
    deadline = time.monotonic() + timeout
    while True:
        screen = read_screen(name)
        if any(text in line for line in screen["lines"]):
            print(json.dumps(screen, indent=2))
            return 0
        if screen["exited"] or time.monotonic() >= deadline:
            break
        time.sleep(POLL_SECONDS)
    print(json.dumps(screen, indent=2))
    reason = "the program exited" if screen["exited"] else f"{timeout:g}s"
    print(f"screen.py: {text!r} not on screen after {reason}", file=sys.stderr)
    return 1


def stop_screen(name: str) -> None:
    """End the screen's program and tmux server, and remove its socket.

    Args:
        name: The screen's name.

    Raises:
        ScreenError: When no screen has this name.
    """
    socket = run_tmux(name, "display-message", "-p", "#{socket_path}")
    run_tmux(name, "kill-server")
    # tmux 3.7 leaves the socket file behind after kill-server.
    Path(socket.strip()).unlink(missing_ok=True)


def is_screen_running(name: str) -> bool:
    """Return whether a screen with this name has a tmux server.

    Args:
        name: The screen's name.

    Returns:
        True when the screen's tmux server answers.
    """
    command = build_tmux_command(name, "has-session", "-t", SESSION)
    return subprocess.run(command, capture_output=True).returncode == 0


def run_tmux(name: str, *args: str) -> str:
    """Run a tmux command on the screen's own server and return stdout.

    Args:
        name: The screen's name.
        args: The tmux command and its arguments.

    Returns:
        What tmux printed.

    Raises:
        ScreenError: When tmux fails, with its message, or when no
            screen has this name.
    """
    result = subprocess.run(
        build_tmux_command(name, *args),
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if result.returncode == 0:
        return result.stdout
    if "no server running" in result.stderr or "error connecting" in (
        result.stderr
    ):
        raise ScreenError(
            f"no screen named {name}; start one with "
            f"screen.py start {name} -- PROGRAM"
        )
    raise ScreenError(f"tmux failed: {result.stderr.strip()}")


def build_tmux_command(name: str, *args: str) -> list[str]:
    """Build a tmux command for the screen's own server.

    ``-f /dev/null`` skips the user's tmux config, so every machine
    draws the same plain screen. ``-u`` keeps box-drawing and other
    non-ASCII characters when the locale is not UTF-8.

    Args:
        name: The screen's name.
        args: The tmux command and its arguments.

    Returns:
        The command as a list of arguments.
    """
    socket = SOCKET_PREFIX + name
    return [find_tmux(), "-u", "-L", socket, "-f", "/dev/null", *args]


def find_tmux() -> str:
    """Return the path of tmux on PATH.

    Returns:
        The path.

    Raises:
        ScreenError: When tmux is not installed.
    """
    path = shutil.which("tmux")
    if path is None:
        raise ScreenError("tmux is not installed; install it and try again")
    return path


if __name__ == "__main__":
    sys.exit(main())
