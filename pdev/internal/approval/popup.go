package approval

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

type Popup struct {
	PID       int    `json:"pid"`
	Workspace string `json:"workspace_id"` // Herdr workspace ID, such as "w1G".
	Name      string `json:"name,omitempty"`
}

// The name does not end in .json, so the queue never reads it as a request.
func popupPath() string {
	return filepath.Join(stateDir(), "open-popup")
}

// SavePopup refuses to overwrite another still-live process's record, so a
// second, short-lived --herdr-popup process cannot clobber the marker for a
// popup that is still open under a different PID.
func SavePopup(popup Popup) error {
	if existing, err := LoadPopup(); err == nil && existing.PID != popup.PID && ProcessAlive(existing.PID) {
		return fmt.Errorf("popup already open for pid %d", existing.PID)
	}
	if err := os.MkdirAll(stateDir(), 0o700); err != nil {
		return err
	}
	data, err := json.Marshal(popup)
	if err != nil {
		return err
	}
	return os.WriteFile(popupPath(), data, 0o600)
}

// LoadPopup returns the saved popup. A popup that was killed leaves its file
// behind, so check that its process is running before trusting it.
func LoadPopup() (Popup, error) {
	var popup Popup
	data, err := os.ReadFile(popupPath())
	if err != nil {
		return popup, err
	}
	err = json.Unmarshal(data, &popup)
	return popup, err
}

// RemovePopup clears the marker only when it still names pid. A second,
// short-lived --herdr-popup process must not erase the marker for a popup
// still open under a different PID.
func RemovePopup(pid int) {
	popup, err := LoadPopup()
	if err != nil || popup.PID != pid {
		return
	}
	_ = os.Remove(popupPath())
}
