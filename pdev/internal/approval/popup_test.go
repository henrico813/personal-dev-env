package approval

import (
	"os"
	"testing"
)

// A short-lived second --herdr-popup process must not overwrite the marker
// for a popup still open under a different, live PID, or the first popup's
// record is lost before it ever defers its own removal.
func TestSavePopupKeepsAnotherProcessesRecord(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	if err := SavePopup(Popup{PID: os.Getpid(), Workspace: "w1"}); err != nil {
		t.Fatal(err)
	}

	if err := SavePopup(Popup{PID: os.Getpid() + 100000, Workspace: "w2"}); err == nil {
		t.Fatal("save overwrote a still-live process's popup record")
	}

	got, err := LoadPopup()
	if err != nil || got.Workspace != "w1" {
		t.Fatalf("popup = %#v, %v; want the original record kept", got, err)
	}
}

// A short-lived second --herdr-popup process must not erase the marker for
// a popup still open under a different PID, or nothing is left to tell the
// inbox or ctrl+b a where the real popup is.
func TestRemovePopupKeepsAnotherProcessesRecord(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	if err := SavePopup(Popup{PID: 111, Workspace: "w1"}); err != nil {
		t.Fatal(err)
	}

	RemovePopup(222)

	got, err := LoadPopup()
	if err != nil || got.PID != 111 {
		t.Fatalf("popup = %#v, %v; want pid 111 kept", got, err)
	}
}

func TestRemovePopupClearsItsOwnRecord(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	if err := SavePopup(Popup{PID: 111, Workspace: "w1"}); err != nil {
		t.Fatal(err)
	}

	RemovePopup(111)

	if _, err := LoadPopup(); err == nil {
		t.Fatal("popup record kept after its own process removed it")
	}
}
