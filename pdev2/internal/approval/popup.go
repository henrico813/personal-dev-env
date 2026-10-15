package approval

import (
	"encoding/json"
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

func SavePopup(popup Popup) error {
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

func RemovePopup() {
	_ = os.Remove(popupPath())
}
