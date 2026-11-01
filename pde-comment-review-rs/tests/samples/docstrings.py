# Reads the approvals file for the review tool.
"""Reads the approvals file."""


def load_approvals():
    # Approvals are read once per step.
    """Return every approval entry, oldest first.

    A missing file has no entries.
    """
    "Only the first string is a docstring."


def save_approvals():
    ("""Write every approval entry, newest last.""")


def shout():
    f"{print('side effect')}"
    return 1


def read_bytes():
    b"""Not a docstring, since Python skips bytes."""


def template():
    t"""Not a docstring, since Python skips template strings."""
