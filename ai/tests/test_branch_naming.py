"""Run the ``planner key`` lines that agents copy from the skills.

Plan ``ABC-12 Approvals file.md`` must give branch
``feat/abc-12-approvals-file``, so one search for ``abc-12`` finds the
plan's branch, worktree and run folder.
"""

import os
import re
import subprocess
import textwrap
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
PLAN = "Main Vault/ABC-12 Approvals file.md"
SH_BLOCK = re.compile(r"^( *)```sh\n(.*?)^\1```", re.MULTILINE | re.DOTALL)


@pytest.fixture(scope="session")
def planner_dir(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Build planner from this checkout, since its ``key`` may be new.

    Args:
        tmp_path_factory: Factory for the build folder.

    Returns:
        The folder holding the built ``planner``.
    """
    folder = tmp_path_factory.mktemp("bin")
    planner = str(folder / "planner")
    subprocess.run(
        ["go", "build", "-C", str(ROOT / "planner"), "-o", planner, "./main"],
        check=True,
    )
    return folder


def run_key_lines(
    path: Path, folder: Path, planner_dir: Path, after: str = ""
) -> list[str]:
    """Run a file's one ``sh`` block that calls ``planner key``, in zsh.

    The block runs on plan ``PLAN`` with type ``feat`` and words
    ``approvals-file``, and a stub ``wt`` prints its arguments.

    Args:
        path: Skill file holding the block.
        folder: Folder for the plan, the stub and ``XDG_STATE_HOME``.
        planner_dir: Folder holding the built ``planner``.
        after: Lines to run after the block.

    Returns:
        The lines the block printed.
    """
    blocks = [
        match.group(2)
        for match in SH_BLOCK.finditer(path.read_text())
        if "planner key" in match.group(2)
    ]
    assert len(blocks) == 1, f"{path} needs one sh block with planner key"
    plan = folder / PLAN
    plan.parent.mkdir()
    plan.write_text("# ABC-12 Approvals file\n")
    wt = folder / "wt"
    wt.write_text('#!/bin/sh\nprintf "%s\\n" "$@"\n')
    wt.chmod(0o755)
    script = textwrap.dedent(blocks[0]) + after
    for placeholder, value in [
        ("<plan file>", str(plan)),
        ("<type>", "feat"),
        ("<words>", "approvals-file"),
    ]:
        script = script.replace(placeholder, value)
    path_dirs = [str(planner_dir), str(folder), os.environ["PATH"]]
    env = {
        **os.environ,
        "PATH": os.pathsep.join(path_dirs),
        "XDG_STATE_HOME": str(folder),
    }
    result = subprocess.run(
        ["zsh", "-c", script],
        cwd=folder,
        env=env,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"zsh failed:\n{script}\n{result.stderr}"
    return result.stdout.splitlines()


def test_agents_md_lines_make_key_branch(
    tmp_path: Path, planner_dir: Path
) -> None:
    """Ask ``wt`` for ``feat/abc-12-approvals-file``, the key and words.

    Every planned branch and worktree starts from these lines. A slip, such
    as ``$K`` in single quotes, drops the key from all of them, and
    ``wt list | rg abc-12`` no longer finds the work.
    """
    wt_args = run_key_lines(ROOT / "ai/AGENTS.md", tmp_path, planner_dir)

    assert wt_args[:3] == ["switch", "--create", "feat/abc-12-approvals-file"]


def test_roadmap_setup_keeps_key_in_names(
    tmp_path: Path, planner_dir: Path
) -> None:
    """Name the run folder and the saved ``BR`` after plan ABC-12.

    A roadmap run builds many pull requests unattended, and each later step
    reads ``BR`` back from the run folder. A slip in this long block would
    name every branch and folder of the run without the key.
    """
    build = ROOT / "ai/skills/roadmap-run/references/build.md"
    # A new shell reads the saved values, as each later tool call does.
    reread = '\nzsh -c \'. "$1/env"; print -r -- "$BR"\' _ "$D"\n'

    *_, folder_line, saved = run_key_lines(
        build, tmp_path, planner_dir, reread
    )
    folder = Path(folder_line.removeprefix("run folder: "))

    assert folder.name.startswith("roadmap-run.abc-12.")
    assert saved == "feat/abc-12-approvals-file"
