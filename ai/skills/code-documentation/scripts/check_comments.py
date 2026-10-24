#!/usr/bin/env python3
"""Run ``pde-comment-checker``, for agents that still use this name."""

import os
import shutil
import sys

command = shutil.which("pde-comment-checker")
# The old pde-comment-checker script ran this file, so the two would loop.
if command is None or os.environ.get("PDE_COMMENT_CHECKER_WRAPPED"):
    print(
        "pde-comment-checker is missing or out of date; "
        "run pde-installer to install it",
        file=sys.stderr,
    )
    sys.exit(2)
os.environ["PDE_COMMENT_CHECKER_WRAPPED"] = "1"
os.execv(command, [command, *sys.argv[1:]])
