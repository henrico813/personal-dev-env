package card

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

// A card note as card writes it, cut to the frontmatter and the Log.
const sampleCard = "---\ntype: card\nstage: active\n---\n\n# DEMO-001 Sample card\n\n## Log\n---\n\n- 2026-11-01 draft: by tester\n"

// Writes text as DEMO-001's note in a new folder and loads it.
func writeTestCard(t *testing.T, text string) Card {
	t.Helper()
	path := filepath.Join(t.TempDir(), "DEMO-001 Sample card.md")
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	c, err := Load(path, "DEMO-001")
	if err != nil {
		t.Fatal(err)
	}
	return c
}

// Saves text over the note as Obsidian would, one second later, so the
// note's modified time changes on any file system.
func saveByHand(t *testing.T, path, text string) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
	later := info.ModTime().Add(time.Second)
	if err := os.Chtimes(path, later, later); err != nil {
		t.Fatal(err)
	}
}

// Reads the note's text.
func readNote(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

// A hand edit saved while card writes must stay in the note, with card's
// change on top. Losing it is the worst bug card can have.
func TestUpdateKeepsSaveMadeDuringWrite(t *testing.T) {
	c := writeTestCard(t, sampleCard)
	handEdit := sampleCard + "- a line saved in Obsidian\n"
	calls := 0

	written, err := Update(c, func(text string) (string, error) {
		calls++
		if calls == 1 {
			saveByHand(t, c.Path, handEdit)
		}
		return text + "- 2026-11-02 note: by tester\n", nil
	})

	if err != nil || !written {
		t.Fatalf("Update = %v, %v; want true, nil", written, err)
	}
	if got, want := readNote(t, c.Path), handEdit+"- 2026-11-02 note: by tester\n"; got != want {
		t.Errorf("note =\n%s\nwant\n%s", got, want)
	}
}

// When the note keeps changing, card must stop with the Python card's
// words, which the hub shows, and leave the last save as it was.
func TestUpdateFailsAfterTwoHandEdits(t *testing.T) {
	c := writeTestCard(t, sampleCard)
	calls := 0
	lastSave := ""

	written, err := Update(c, func(text string) (string, error) {
		calls++
		lastSave = sampleCard + "- save " + string(rune('0'+calls)) + "\n"
		saveByHand(t, c.Path, lastSave)
		return text + "- 2026-11-02 note: by tester\n", nil
	})

	if written || err == nil || err.Error() != "DEMO-001 changed twice while writing; try again" {
		t.Fatalf("Update = %v, %v; want the changed twice error", written, err)
	}
	if got := readNote(t, c.Path); got != lastSave {
		t.Errorf("note =\n%s\nwant the last save\n%s", got, lastSave)
	}
}
