package card

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Writes text as a note in a project folder under projects, and lists its
// change to newText as rekey would.
func writeRekeyNote(t *testing.T, projects, name, text, newText string) NoteChange {
	t.Helper()
	path := filepath.Join(projects, "Demo", name)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	return NoteChange{Path: path, NewPath: path, NewText: newText, ModTime: info.ModTime()}
}

// A note the user edits after the listing must keep that edit, and rekey
// must not count it as written. Otherwise a stopped rekey loses the
// user's words, or its undo copies the old note over them.
func TestWriteRekeyChangesStopsAtEdit(t *testing.T) {
	projects := Projects{Dir: t.TempDir()}
	backup := t.TempDir()
	first := writeRekeyNote(t, projects.Dir, "A.md", "See [[DEMO-KB-001]].\n", "See [[DEMO-411]].\n")
	second := writeRekeyNote(t, projects.Dir, "B.md", "See [[DEMO-KB-002]].\n", "See [[DEMO-412]].\n")
	saveByHand(t, second.Path, "See [[DEMO-KB-002]].\nedited\n")

	written, err := WriteRekeyChanges(context.Background(), projects, backup, []NoteChange{first, second})

	if err == nil || !strings.Contains(err.Error(), "stopped at B.md") {
		t.Fatalf("error = %v, want a stop at B.md", err)
	}
	if len(written) != 1 || written[0].Path != first.Path {
		t.Fatalf("written = %v, want only A.md", written)
	}
	if got := readNote(t, first.Path); got != "See [[DEMO-411]].\n" {
		t.Errorf("A.md = %q, want the new link", got)
	}
	if got := readNote(t, second.Path); got != "See [[DEMO-KB-002]].\nedited\n" {
		t.Errorf("B.md = %q, want the user's edit kept", got)
	}
}

// After a stop, undo.sh must restore the notes rekey wrote and leave the
// note it stopped at alone, though that note has a backup copy. Otherwise
// the undo copies the old note over the user's edit.
func TestWriteUndoScriptSkipsUnwrittenNotes(t *testing.T) {
	projects := Projects{Dir: t.TempDir()}
	backup := t.TempDir()
	first := writeRekeyNote(t, projects.Dir, "A.md", "See [[DEMO-KB-001]].\n", "See [[DEMO-411]].\n")
	second := writeRekeyNote(t, projects.Dir, "B.md", "See [[DEMO-KB-002]].\n", "See [[DEMO-412]].\n")
	saveByHand(t, second.Path, "See [[DEMO-KB-002]].\nedited\n")
	written, _ := WriteRekeyChanges(context.Background(), projects, backup, []NoteChange{first, second})
	if len(written) != 1 {
		t.Fatalf("written = %v, want a stop after A.md", written)
	}

	undo, err := WriteUndoScript(projects, backup, written)
	if err != nil {
		t.Fatal(err)
	}
	if out, err := exec.Command("sh", undo).CombinedOutput(); err != nil {
		t.Fatalf("sh undo.sh: %v: %s", err, out)
	}

	if got := readNote(t, first.Path); got != "See [[DEMO-KB-001]].\n" {
		t.Errorf("A.md after undo = %q, want the old link", got)
	}
	if got := readNote(t, second.Path); got != "See [[DEMO-KB-002]].\nedited\n" {
		t.Errorf("B.md after undo = %q, want the user's edit kept", got)
	}
}
