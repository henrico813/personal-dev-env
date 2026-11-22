package approval

import "testing"

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
