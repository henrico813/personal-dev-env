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

// Runs in Herdr's popup. Without a review name in its environment, it first
// lists the waiting reviews to pick from.
func runReviewPopup(in, out *os.File) error {
	if saveOpenPopup("Comment review") == nil {
		defer removeOpenPopup()
	}
	name := os.Getenv(reviewNameEnv)
	if name == "" {
		var err error
		name, err = pickWaitingReview(in, out)
		if err != nil || name == "" {
			return err
		}
	}
	r, origin, err := openReviewWithOrigin(name)
	if err != nil {
		return err
	}
	_ = saveOpenPopup(popup.BuildRecordName(origin))
	return runReviewScreen(r, origin, in, out, screenOptions{settle: popup.Settle, inPopup: true})
}

// Runs in Herdr's review tab, where the e and d keys open nvim and less in
// the same tab. It closes the tab when the owner quits.
func runReviewTab(in, out *os.File) error {
	if os.Getenv("HERDR_PLUGIN_ENTRYPOINT_ID") != tabPaneID {
		return errors.New("only Herdr's review tab runs tab; press Ctrl+b E")
	}
	name := os.Getenv(reviewNameEnv)
	var err error
	if name == "" {
		name, err = pickWaitingReview(in, out)
	}
	if err == nil && name != "" {
		err = answerReview(name, in, out, popup.Settle)
	}
	if err != nil {
		waitForEnter(err)
	}
	return closeOwnTab()
}

// Returns the review the owner picks, or "" if they quit or none wait.
func pickWaitingReview(in, out *os.File) (string, error) {
	choices, err := listWaitingChoices()
	if err != nil || len(choices) == 0 {
		return "", err
	}
	picker := popup.NewPicker(choices)
	term, err := popup.Start(in, out)
	if err != nil {
		return "", err
	}
	defer term.Restore()
	for {
		key, err := term.DrawAndReadKey(func(cols, rows int) []string {
			picker.Cols, picker.Rows = cols, rows
			return picker.BuildScreen()
		})
		if err != nil {
			return "", err
		}
		switch picker.PressKey(key) {
		case popup.Quit:
			return "", nil
		case popup.Open:
			return picker.Chosen(), nil
		}
	}
}

func listWaitingChoices() ([]popup.Choice, error) {
	waiting, err := review.ListWaitingReviews()
	if err != nil {
		return nil, err
	}
	var choices []popup.Choice
	for _, r := range waiting {
		origin, err := readReviewOrigin(r)
		if err != nil {
			return nil, err
		}
		entries := r.ListEntries()
		answered := 0
		for _, e := range entries {
			if e.Decision != nil {
				answered++
			}
		}
		choices = append(choices, popup.Choice{
			Name:     r.Name,
			Origin:   origin,
			Items:    len(entries),
			Answered: answered,
		})
	}
	return choices, nil
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
