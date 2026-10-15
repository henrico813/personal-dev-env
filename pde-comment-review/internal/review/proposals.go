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
	"strconv"
	"strings"

	"pde-comment-review/internal/comment"
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

// One comment and what the AI proposes for it. Old is the comment copied
// exactly from File: all of its lines, each with its indentation and
// newline.
type Item struct {
	ID       int      `json:"id"`
	File     string   `json:"file"`
	Old      string   `json:"old"`
	Proposal string   `json:"proposal"`
	New      string   `json:"new,omitempty"`
	Why      string   `json:"why,omitempty"`
	AlsoAt   []string `json:"also_at,omitempty"`
}

const (
	ProposeKeep   = "keep"
	ProposeCut    = "cut"
	ProposeReword = "reword"
)

// Reads a proposals file and checks it against the repository. Each old must
// be whole "//" lines found exactly once in its file, starting a line, with
// no comment line just above or below; a Go directive just above or below
// is allowed. Reports a problem for every bad item and rename together,
// rather than stopping at the first.
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
		if err := findItemProblem(p.Repo, item); err != nil {
			problems = append(problems, fmt.Errorf("item %d: %w", item.ID, err))
		}
	}
	return p, errors.Join(problems...)
}

func findItemProblem(repo string, item Item) error {
	// Apply would write "./a.go" and "a.go" separately, losing one's edits.
	if !filepath.IsLocal(item.File) || filepath.Clean(item.File) != item.File {
		return fmt.Errorf("file %q must be a plain relative path inside the repo, such as a/b.go", item.File)
	}
	if !strings.HasSuffix(item.File, ".go") {
		return fmt.Errorf("file %q must be a Go file", item.File)
	}
	if err := comment.CheckWholeLines(item.Old); err != nil {
		return fmt.Errorf("old: %w", err)
	}
	switch item.Proposal {
	case ProposeKeep, ProposeCut:
		if item.New != "" {
			return fmt.Errorf("new must be empty when the proposal is %s", item.Proposal)
		}
	case ProposeReword:
		if err := comment.CheckWholeLines(item.New); err != nil {
			return fmt.Errorf("new: %w", err)
		}
		if item.New == item.Old {
			return errors.New("new is the same as old; propose keep instead")
		}
	default:
		return fmt.Errorf("proposal %q must be keep, cut, or reword", item.Proposal)
	}
	data, err := os.ReadFile(filepath.Join(repo, item.File))
	if err != nil {
		return err
	}
	if _, err := findOnce(string(data), item.Old); err != nil {
		return fmt.Errorf("%s: %w", item.File, err)
	}
	return nil
}

// Returns where old starts in text. Refuses old unless it appears exactly
// once, starts a line, is not inside a string or /* */ comment, and has
// no comment line just above or below it, not counting a Go directive.
func findOnce(text, old string) (int, error) {
	switch n := strings.Count(text, old); n {
	case 0:
		return 0, errors.New("comment found 0 times, want once")
	case 1:
	default:
		return 0, fmt.Errorf("comment found %d times, at lines %s; pde-comment-review cannot tell the copies apart", n, listCopyLines(text, old))
	}
	at := strings.Index(text, old)
	if at > 0 && text[at-1] != '\n' {
		return 0, errors.New("comment does not start a line")
	}
	if err := comment.RefusePartialComment(text, at, at+len(old)); err != nil {
		return 0, fmt.Errorf("old %w", err)
	}
	if err := comment.RefuseTextOutsideComments(text, at, at+len(old)); err != nil {
		return 0, fmt.Errorf("old %w", err)
	}
	return at, nil
}

// Returns the line where each copy of old starts in text, such as "3, 9".
func listCopyLines(text, old string) string {
	var found []string
	for at := strings.Index(text, old); at >= 0; {
		found = append(found, strconv.Itoa(strings.Count(text[:at], "\n")+1))
		next := strings.Index(text[at+len(old):], old)
		if next < 0 {
			break
		}
		at += len(old) + next
	}
	return strings.Join(found, ", ")
}
