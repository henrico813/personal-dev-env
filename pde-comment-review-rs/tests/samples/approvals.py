# Approvals live in the clone's shared Git folder.
APPROVALS_PATH = ".git/pde/code-comments.json"

def load_approvals():
    """Return every approval entry, oldest first."""
