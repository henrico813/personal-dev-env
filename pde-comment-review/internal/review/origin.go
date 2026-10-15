package review

import (
	"path/filepath"
	"strings"
)

// Tells the owner which checkout and agent a review is for.
type Origin struct {
	RepoName string `json:"repo_name"`
	// Or a short commit when no branch is checked out.
	Branch string `json:"branch,omitempty"`
	// Titles change and get cut short, so reviews are grouped by this.
	Session string `json:"session,omitempty"`
	// Herdr's title for the session, if known.
	Agent string `json:"agent,omitempty"`
}

// Drops the branch's prefix, such as "vibe/", to save room on a phone.
func (o Origin) Label() string {
	if o.Branch == "" {
		return o.RepoName
	}
	return o.RepoName + " · " + o.Branch[strings.LastIndex(o.Branch, "/")+1:]
}

// Each round replaces it, so the label shows who asked last.
func (r *Review) SetOrigin(o Origin) error {
	return replaceJSONFile(filepath.Join(r.Dir, "origin.json"), o)
}
