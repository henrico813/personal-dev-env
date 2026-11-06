"""Keeps the notes the owner writes for the agent."""

import json
from pathlib import Path


class NoteBook:
    """Holds one review's notes, one per comment."""

    def __init__(self, folder):
        self.folder = Path(folder)
        self.notes = {}

    def add(self, comment_id, note):
        # Stores the note so the agent reads it on its next turn.
        self.notes[comment_id] = note.strip()
        self.save()

    def save(self):
        """Writes every note to notes.json."""
        path = self.folder / "notes.json"
        path.write_text(json.dumps(self.notes, indent=2))

    def clear(self):
        """Forgets every note."""
