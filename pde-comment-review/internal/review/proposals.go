// Package review keeps comment reviews: the rounds of proposals an AI writes
// and the owner's notes on them. Reviews are saved under
// $XDG_STATE_HOME/pde/comment-review, so a restart loses nothing.
package review

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// The version of the proposals file layout this program reads; files with
// another version are refused. Raise it when a change to the layout would
// make an older copy of this program misread a newer file.
const Version = 1

// The file an AI writes for one round of a review.
type Proposals struct {
	Version int    `json:"version"`
	Repo    string `json:"repo"`
	Items   []Item `json:"items"`
	// Code the AI renamed, each written "OLD=NEW", such as "Load=LoadConfig",
	// so the owner's approved comments can move to the new names.
	Renames []string `json:"renames,omitempty"`
}

// One comment and what the AI proposes for it. Old is the comment's
// whole lines exactly as in File, or on a draft's item the approved
// text.
type Item struct {
	ID       int      `json:"id"`
	File     string   `json:"file"`
	Old      string   `json:"old"`
	Proposal string   `json:"proposal"`
	New      string   `json:"new,omitempty"`
	Why      string   `json:"why,omitempty"`
	AlsoAt   []string `json:"also_at,omitempty"`
	// The comment checker's name for the comment, such as "Cart.Total",
	// on items a draft found.
	Name string `json:"name,omitempty"`
	// Set when the code under an approved comment was deleted; Old is the
	// approved text, which is no longer in File.
	CodeRemoved bool `json:"code_removed,omitempty"`
}

const (
	ProposeKeep   = "keep"
	ProposeCut    = "cut"
	ProposeReword = "reword"
)

// Reads a proposals file and checks its shape, reporting every bad item
// and rename together rather than stopping at the first.
func ParseProposals(data []byte) (Proposals, error) {
	var p Proposals
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&p); err != nil {
		return p, fmt.Errorf("read proposals: %w", err)
	}
	if _, err := decoder.Token(); err != io.EOF {
		return p, errors.New("read proposals: text after the JSON object")
	}
	if p.Version != Version {
		return p, fmt.Errorf("proposals version is %d; this pde-comment-review reads version %d", p.Version, Version)
	}
	if !filepath.IsAbs(p.Repo) {
		return p, fmt.Errorf("repo %q is not an absolute path", p.Repo)
	}
	if len(p.Items) == 0 {
		return p, errors.New("proposals have no items")
	}
	var problems []error
	for _, pair := range p.Renames {
		if before, after, _ := strings.Cut(pair, "="); before == "" || after == "" {
			problems = append(problems, fmt.Errorf("rename %q must be OLD=NEW", pair))
		}
	}
	seen := map[int]bool{}
	for _, item := range p.Items {
		if seen[item.ID] {
			problems = append(problems, fmt.Errorf("item %d: id is used twice", item.ID))
		}
		seen[item.ID] = true
		if err := findItemProblem(item); err != nil {
			problems = append(problems, fmt.Errorf("item %d: %w", item.ID, err))
		}
	}
	return p, errors.Join(problems...)
}

func findItemProblem(item Item) error {
	// Apply would write "./a.go" and "a.go" separately, losing one's edits.
	if !filepath.IsLocal(item.File) || filepath.Clean(item.File) != item.File {
		return fmt.Errorf("file %q must be a plain relative path inside the repo, such as a/b.go", item.File)
	}
	if item.Old == "" {
		return errors.New("old is empty; it must be the comment's whole lines")
	}
	switch item.Proposal {
	case ProposeKeep, ProposeCut:
		if item.New != "" {
			return fmt.Errorf("new must be empty when the proposal is %s", item.Proposal)
		}
	case ProposeReword:
		if item.New == "" {
			return errors.New("new is empty; a reword needs the new comment lines")
		}
		if item.New == item.Old {
			return errors.New("new is the same as old; propose keep instead")
		}
	default:
		return fmt.Errorf("proposal %q must be keep, cut, or reword", item.Proposal)
	}
	if item.CodeRemoved && (item.Name == "" || item.Proposal != ProposeCut) {
		return errors.New("an item with code_removed needs a name and proposal cut")
	}
	return nil
}

// Holds the first and last line of a comment in its file, counting from 1.
type CommentLines struct {
	First int
	Last  int
	// True for a docstring that is the only statement in its block, so
	// deleting it would break the code.
	NoCut bool
}

// Returns line numbers or an error if the checker found no unique match.
func FindCommentLines(item Item, places map[int]CommentLines) (CommentLines, error) {
	at, ok := places[item.ID]
	if !ok {
		return at, fmt.Errorf(
			"the checker cannot find this comment exactly once in %s",
			item.File,
		)
	}
	return at, nil
}

// Returns up to codeLinesAround lines of code above and below the item's
// comment in repo, to show it in place.
func ReadCodeAround(
	repo string,
	item Item,
	places map[int]CommentLines,
	codeLinesAround int,
) (before, after []string, err error) {
	at, err := FindCommentLines(
		item,
		places,
	)
	if err != nil {
		return nil, nil, err
	}
	data, err := os.ReadFile(filepath.Join(repo, item.File))
	if err != nil {
		return nil, nil, err
	}
	lines := strings.Split(
		strings.TrimSuffix(string(data), "\n"),
		"\n",
	)
	// The file may have changed since the screen read it.
	if at.First < 1 || at.First > at.Last || at.Last > len(lines) {
		return nil, nil, fmt.Errorf(
			"%s changed after the screen read it",
			item.File,
		)
	}
	before = lines[max(0, at.First-1-codeLinesAround) : at.First-1]
	after = lines[at.Last:min(len(lines), at.Last+codeLinesAround)]
	return before, after, nil
}
