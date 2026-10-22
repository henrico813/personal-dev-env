package review

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-comment-review/internal/comment"
)

// Returns each file's new text without writing it, so
// writeAndApproveDecisions can refuse before changing any file.
func (r *Review) BuildChangedFiles(foundByName map[int]string) (map[string]string, error) {
	entries := r.ListEntries()
	if err := refuseUndecided(entries); err != nil {
		return nil, err
	}
	texts := map[string]string{}
	changed := map[string]string{}
	var problems []error
	for _, e := range entries {
		// Removed code left no comment to change.
		if e.Item.CodeRemoved {
			continue
		}
		file := e.Item.File
		text, ok := texts[file]
		if !ok {
			data, err := os.ReadFile(filepath.Join(r.Repo(), file))
			if err != nil {
				problems = append(problems, fmt.Errorf("item %d: %w", e.Item.ID, err))
				continue
			}
			text = string(data)
		}
		// Each item is found in the text with earlier answers already in
		// it, so two items in one file both land.
		current := findCurrentComment(e.Item, foundByName)
		at, err := findOnce(text, current)
		if err != nil {
			problems = append(
				problems,
				fmt.Errorf(
					"%s: item %d changed in the file after it was added; ask the AI to send it again",
					file,
					e.Item.ID,
				),
			)
			continue
		}
		replacement, err := chooseReplacement(e)
		if err != nil {
			problems = append(problems, fmt.Errorf("item %d: %w", e.Item.ID, err))
			continue
		}
		if replacement != current {
			text = text[:at] + replacement + text[at+len(current):]
			changed[file] = text
		}
		texts[file] = text
	}
	if err := errors.Join(problems...); err != nil {
		return nil, err
	}
	return changed, nil
}

// Returns the text that takes the place of the item's comment: its old text
// for a keep, "" for a cut.
func chooseReplacement(e Entry) (string, error) {
	d, item := e.Decision, e.Item
	switch {
	case d.Action == ActionEdited:
		if err := comment.CheckWholeLines(d.Text); err != nil {
			return "", fmt.Errorf("the owner's text: %w", err)
		}
		return d.Text, nil
	case d.Action == ActionAccept && item.Proposal == ProposeReword:
		return item.New, nil
	case d.Action == ActionCut || d.Action == ActionAccept && item.Proposal == ProposeCut:
		return "", nil
	case d.Action == ActionKeep || d.Action == ActionAccept && item.Proposal == ProposeKeep:
		return item.Old, nil
	}
	return "", fmt.Errorf("decision %q must be keep, cut, accept or edited", d.Action)
}

func refuseUndecided(entries []Entry) error {
	var ids []string
	for _, e := range entries {
		if e.Decision == nil {
			ids = append(ids, fmt.Sprint(e.Item.ID))
		}
	}
	if len(ids) > 0 {
		return fmt.Errorf("not decided: items %s", strings.Join(ids, ", "))
	}
	return nil
}
