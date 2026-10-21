package review

import (
	"fmt"
	"path/filepath"
	"strings"
)

// The next round an agent is preparing, saved as draft.json in the
// review's folder.
type Draft struct {
	// The commit the checkout was compared with.
	Base      string    `json:"base"`
	Proposals Proposals `json:"proposals"`
}

// Returns the review name for a Git branch: lower case, with each
// character a name may not use turned into "-". For example,
// "vibe/Add-Tax" becomes "vibe-add-tax".
func NameForBranch(branch string) (string, error) {
	name := strings.Map(
		func(r rune) rune {
			if r >= 'a' && r <= 'z' || r >= '0' && r <= '9' || strings.ContainsRune("._-", r) {
				return r
			}
			return '-'
		},
		strings.ToLower(branch),
	)
	name = strings.TrimLeft(name, "._-")
	if !namePattern.MatchString(name) {
		return "", fmt.Errorf(
			"branch %q has no a-z or 0-9 character to name its review; rename the branch",
			branch,
		)
	}
	return name, nil
}

// Saves d as the draft of the review called name, replacing any
// earlier draft. Saved rounds, notes and decisions are not touched.
func SaveDraft(name string, d Draft) error {
	dir, err := buildReviewDir(name)
	if err != nil {
		return err
	}
	return replaceJSONFile(filepath.Join(dir, "draft.json"), d)
}
