"""Pack skill-lessons into one file that runs with ``/usr/bin/python3``.

Example:
    python3 build.py /tmp/skill-lessons

Packs the regular ``.py`` files in ``src/skill_lessons/``; links and
folders are skipped. The result is a zip behind a ``#!`` line, so a
machine needs only Python 3.10 or later. That line runs Python with
``-I``, so ``PYTHON*`` variables and user site packages cannot change
what runs.
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path

PACKAGE = Path(__file__).parent / "src" / "skill_lessons"
# Write __main__ by hand; the one zipapp generates drops main()'s return
# value, so a refused command would exit 0.
MAIN = "import sys\nfrom skill_lessons.cli import main\nsys.exit(main())\n"


def list_sources() -> list[Path]:
    """Return the regular ``.py`` files in ``src/skill_lessons/``.

    Links, such as an editor's ``.#cli.py`` lock, and folders are left out.
    pde-installer hashes the same files to decide when to rebuild.
    """
    return sorted(
        path
        for path in PACKAGE.iterdir()
        if path.name.endswith(".py")
        and path.is_file()
        and not path.is_symlink()
    )


def build_packed_file(output: Path) -> None:
    """Write the runnable file to ``output`` and make it executable.

    A link at ``output``, such as one uv left, is removed first rather
    than written through.
    """
    output.unlink(missing_ok=True)
    with output.open("wb") as file:
        file.write(b"#!/usr/bin/python3 -I\n")
        with zipfile.ZipFile(file, "w") as archive:
            archive.writestr("__main__.py", MAIN)
            for path in list_sources():
                archive.write(path, f"skill_lessons/{path.name}")
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
