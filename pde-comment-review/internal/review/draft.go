package review

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// The next round an agent is preparing, saved as draft.json in the
// review's folder.
type Draft struct {
	// The commit the checkout was compared with.
	Base      string    `json:"base"`
	Proposals Proposals `json:"proposals"`
}

// Returns the review name for a branch, such as "vibe-add-tax" for
// "vibe/Add-Tax".
func ReviewNameForBranch(branch string) (string, error) {
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

// Moves the finished review called name aside, such as to name.1, since
// a draft numbers its items from 1. Refuses while a round waits for the
// owner.
func MoveFinishedReviewAside(name string) error {
	r, err := loadReview(name)
	if err != nil || len(r.Rounds) == 0 {
		return err
	}
	if r.AppliedRound != len(r.Rounds) {
		return fmt.Errorf(
			"review %s, for %s, has a round waiting for the owner; send changed items with add, as the comment-review skill says, or tell the owner if that is another checkout",
			name,
			r.Repo(),
		)
	}
	for n := 1; ; n++ {
		aside := r.Dir + "." + strconv.Itoa(n)
		_, err := os.Stat(aside)
		if errors.Is(err, fs.ErrNotExist) {
			return os.Rename(r.Dir, aside)
		}
		if err != nil {
			return err
		}
	}
}

// Saves d as the draft of the review called name, replacing any old one.
func SaveDraft(name string, d Draft) error {
	dir, err := buildReviewDir(name)
	if err != nil {
		return err
	}
	return replaceJSONFile(filepath.Join(dir, "draft.json"), d)
}
