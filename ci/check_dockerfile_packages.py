"""Fail when a Dockerfile package install puts two packages on one line.

One package per line keeps each added or removed package on its own line
in a diff. With no arguments, checks every Dockerfile that git tracks.

Example finding::

    vibe/docker/Dockerfile:4: bash, git, ripgrep share one line
"""

import subprocess
import sys

# Each install command, as the command word and its subcommand.
INSTALL_COMMANDS = {
    "apt-get": "install",
    "apt": "install",
    "apk": "add",
    "pip": "install",
    "pip3": "install",
    "npm": "install",
}
SEPARATORS = {"&&", "||", "|", ";"}


def main(argv):
    """Print each crowded install line and return 1 if any was found."""
    paths = argv or list_tracked_dockerfiles()
    findings = []
    for path in paths:
        findings.extend(check_dockerfile(path))
    for finding in findings:
        print(finding)
    return 1 if findings else 0


def list_tracked_dockerfiles():
    """Return the paths of tracked files named Dockerfile or Dockerfile.*."""
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "--",
            ":(glob)**/Dockerfile",
            ":(glob)**/Dockerfile.*",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.splitlines()


def check_dockerfile(path):
    """Return a finding for each line that installs more than one package."""
    with open(path, encoding="utf-8") as dockerfile:
        lines = dockerfile.read().splitlines()
    findings = []
    for line_number, packages in find_crowded_install_lines(lines):
        names = ", ".join(packages)
        findings.append(f"{path}:{line_number}: {names} share one line")
    return findings


def find_crowded_install_lines(lines):
    """Yield the line number and packages of each crowded install line."""
    command = None
    installing = False
    in_run = False
    for line_number, line in enumerate(lines, start=1):
        words = line.split()
        # Docker drops comment lines inside a RUN without ending it.
        if in_run and words and words[0].startswith("#"):
            continue
        if not in_run:
            if not words or words[0].upper() != "RUN":
                continue
            words = words[1:]
            command = None
            installing = False
        in_run = line.rstrip().endswith("\\")
        packages = []
        for word in split_separators(words):
            if word in SEPARATORS:
                command = None
                installing = False
            elif installing:
                if not word.startswith("-") and word != "\\":
                    packages.append(word)
            elif word in INSTALL_COMMANDS:
                command = word
            elif command and not word.startswith("-"):
                installing = word == INSTALL_COMMANDS[command]
                command = None
        if len(packages) > 1:
            yield line_number, packages


def split_separators(words):
    """Yield each word, splitting a trailing semicolon off as its own word."""
    for word in words:
        if word.endswith(";") and word != ";":
            yield word[:-1]
            yield ";"
        else:
            yield word


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
