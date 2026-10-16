package cmd

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// The record the PR approval tool reads when Herdr refuses its popup, so it
// can say which popup to close and where.
type popupRecord struct {
	PID       int    `json:"pid"`
	Workspace string `json:"workspace_id"`
	Name      string `json:"name"`
}

// Runs in Herdr's popup, on the review named in its environment.
func runReviewPopup(in, out *os.File) error {
	r, origin, err := openReviewWithOrigin(os.Getenv(reviewNameEnv))
	if err != nil {
		return err
	}
	if saveOpenPopup(popup.BuildRecordName(origin)) == nil {
		defer removeOpenPopup()
	}
	return runReviewScreen(r, origin, in, out, popup.Settle)
}

func findOpenPopupPath() string {
	return filepath.Join(filepath.Dir(review.FindStateRoot()), "pr-write", "open-popup")
}

// Saves this popup's process, workspace and name for the PR approval tool.
// Herdr gives a popup no HERDR_WORKSPACE_ID, so the workspace comes from
// HERDR_PLUGIN_CONTEXT_JSON.
func saveOpenPopup(name string) error {
	var context struct {
		Workspace string `json:"workspace_id"`
	}
	if err := json.Unmarshal([]byte(os.Getenv("HERDR_PLUGIN_CONTEXT_JSON")), &context); err != nil {
		return err
	}
	if context.Workspace == "" {
		return errors.New("herdr gave no workspace")
	}
	data, err := json.Marshal(popupRecord{PID: os.Getpid(), Workspace: context.Workspace, Name: name})
	if err != nil {
		return err
	}
	path := findOpenPopupPath()
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o600)
}

func removeOpenPopup() {
	_ = os.Remove(findOpenPopupPath())
}
