"""Run ``hub read``, which takes in one report from a session.

It prints the report, archives it, logs the hub's note and names the
other unread reports, so the hub can read the next report in the step
that acts on this one.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path


def run_read(args: argparse.Namespace) -> int:
    """Print a session's report, archive it and log the hub's note.

    The report is renamed before it is read, so a report the session
    writes after that stays unread for the next ``hub read``.

    Args:
        args: The parsed ``read`` command line.

    Returns:
        0 after printing the report, or 2 when the session has no
        unread report.
    """
    stamp = time.strftime("%H%M")
    report = args.run / f"report-{args.name}.md"
    try:
        archive = archive_report(report, stamp)
    except FileNotFoundError:
        print(f"hub: no unread report from {args.name}", file=sys.stderr)
        return 2
    if args.note is not None:
        with (args.run / "notes.md").open("a") as notes:
            notes.write(f"- {stamp} {args.note}\n")
    text = archive.read_text()
    print(text, end="" if text.endswith("\n") else "\n")
    others = sorted(
        path.stem.removeprefix("report-")
        for path in args.run.glob("report-*.md")
    )
    print(f"unread: {' '.join(others) or '-'}")
    return 0


def archive_report(report: Path, stamp: str) -> Path:
    """Rename the report to an archive name no earlier read used.

    Args:
        report: The unread ``report-NAME.md``.
        stamp: The time as HHMM, such as ``2250``.

    Returns:
        The archive, ``report-NAME.HHMM.read``. When a report from the
        same session was read in the same minute, it is
        ``report-NAME.HHMM-2.read``, then ``-3`` and so on.

    Raises:
        FileNotFoundError: The report does not exist.
    """
    archive = report.with_name(f"{report.stem}.{stamp}.read")
    count = 2
    while archive.exists():
        archive = report.with_name(f"{report.stem}.{stamp}-{count}.read")
        count += 1
    report.rename(archive)
    return archive
