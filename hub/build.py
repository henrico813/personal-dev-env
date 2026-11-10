"""Pack hub into one file that runs with ``/usr/bin/python3``.

Example:
    python3 build.py /tmp/hub

Packs the regular ``.py`` files in ``src/hub/``, as skill-lessons'
build.py does, so a machine needs only Python 3.10 or later.
"""

from __future__ import annotations

import sys
import zipfile
from pathlib import Path

PACKAGE = Path(__file__).parent / "src" / "hub"
# Written by hand because zipapp's own __main__ drops main()'s return
# value, so a failed command would exit 0.
MAIN = "import sys\nfrom hub.cli import main\nsys.exit(main())\n"


def build_packed_file(output: Path) -> None:
    """Write the runnable file to ``output`` and make it executable.

    Links and folders in ``src/hub/`` are left out. pde-installer
    hashes the same files to decide when to rebuild. A link at
    ``output`` is removed first rather than written through.

    Args:
        output: The file to write.
    """
    sources = sorted(
        path
        for path in PACKAGE.iterdir()
        if path.name.endswith(".py")
        and path.is_file()
        and not path.is_symlink()
    )
    output.unlink(missing_ok=True)
    with output.open("wb") as file:
        file.write(b"#!/usr/bin/python3 -I\n")
        with zipfile.ZipFile(file, "w") as archive:
            archive.writestr("__main__.py", MAIN)
            for path in sources:
                archive.write(path, f"hub/{path.name}")
    output.chmod(0o755)


def main(argv: list[str] | None = None) -> int:
    """Build the file named by the only argument.

    Args:
        argv: The arguments after ``build.py``, or None for ``sys.argv``.

    Returns:
        0 once built, or 2 without exactly one argument.
    """
    args = sys.argv[1:] if argv is None else argv
    if len(args) != 1:
        print("usage: build.py OUTPUT", file=sys.stderr)
        return 2
    build_packed_file(Path(args[0]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
