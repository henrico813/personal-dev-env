"""Pack pde-comment-checker into one file run by ``/usr/bin/python3``.

Example:
    python3 build.py /tmp/pde-comment-checker

Packs the regular ``.py`` files under ``src/comment_checker/`` into a
zip behind a ``#!`` line that runs Python 3.10 or later with ``-I``, so
``PYTHON*`` variables and user site packages cannot change what runs.
``pde-comment-checker python-rules ARGS`` runs the Python rule checker,
and its usage line names both words.
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path

PACKAGE = Path(__file__).parent / "src" / "comment_checker"
# Write __main__ by hand, since the one zipapp generates drops main()'s
# return value and a failed check would exit 0.
MAIN = """\
import sys
if sys.argv[1:2] == ["python-rules"]:
    del sys.argv[1]
    sys.argv[0] += " python-rules"
    from comment_checker.python_rules import main
else:
    from comment_checker.cli import main
sys.exit(main())
"""


def list_sources() -> list[Path]:
    """Return the regular ``.py`` files in ``src/comment_checker/``.

    Files in subfolders, such as ``languages/go.py``, are included.
    Links, such as an editor's ``.#cli.py`` lock, are left out.
    pde-installer hashes the same files to decide when to rebuild.
    """
    return sorted(
        path
        for path in PACKAGE.rglob("*.py")
        if path.is_file() and not path.is_symlink()
    )


def build_packed_file(output: Path) -> None:
    """Write the runnable file to ``output`` and make it executable.

    A link at ``output`` is removed first rather than written through.
    """
    output.unlink(missing_ok=True)
    with output.open("wb") as file:
        file.write(b"#!/usr/bin/python3 -I\n")
        with zipfile.ZipFile(file, "w") as archive:
            archive.writestr("__main__.py", MAIN)
            for path in list_sources():
                name = path.relative_to(PACKAGE).as_posix()
                archive.write(path, f"comment_checker/{name}")
    output.chmod(0o755)


def main(argv: list[str] | None = None) -> int:
    """Build the file named by the only argument and return the exit code."""
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        print("usage: build.py OUTPUT", file=sys.stderr)
        return 2
    build_packed_file(Path(args[0]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
