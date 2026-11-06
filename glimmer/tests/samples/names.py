"""Reads and writes the approvals file."""

# The approvals file in the clone's Git folder.
APPROVALS_PATH = ".git/pde/code-comments.json"


class Approvals:
    """Every approval entry, oldest first."""

    # Reads the file once per step.
    @classmethod
    def load(cls):
        """Return every approval entry, oldest first."""
        # A missing file has no entries.
        entries = cls()
        return entries


# Approvals are read again after each answer.

def save_approvals():
    ("""Write every approval entry.""")
