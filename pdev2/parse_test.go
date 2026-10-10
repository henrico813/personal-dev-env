package main

import (
	"bytes"
	"os"
	"path/filepath"
	"slices"
	"testing"
)

// The approver must see the text gh will post. A body file is read once and
// sent to gh on standard input, so later edits to the file cannot change what
// was approved.
func TestParseReadsEveryFlagForm(t *testing.T) {
	bodyPath := filepath.Join(t.TempDir(), "body.md")
	if err := os.WriteFile(bodyPath, []byte("file body"), 0o600); err != nil {
		t.Fatal(err)
	}
	stdin := bytes.NewBufferString("stdin body")
	cases := []struct {
		name                      string
		args                      []string
		title, body, repo, target string
		ghArgs                    []string
	}{
		{name: "title", args: []string{"pr", "create", "--title", "T"}, title: "T"},
		{name: "title equals", args: []string{"pr", "create", "--title=T"}, title: "T"},
		{name: "merge subject", args: []string{"pr", "merge", "12", "--subject", "S"}, title: "S", target: "12"},
		{name: "body file", args: []string{"pr", "comment", "12", "--body-file", bodyPath},
			body: "file body", target: "12", ghArgs: []string{"pr", "comment", "12", "--body-file", "-"}},
		{name: "body file equals before target", args: []string{"pr", "comment", "--body-file=" + bodyPath, "12"},
			body: "file body", target: "12", ghArgs: []string{"pr", "comment", "--body-file=-", "12"}},
		{name: "body from stdin", args: []string{"pr", "comment", "12", "-F", "-"},
			body: "stdin body", target: "12", ghArgs: []string{"pr", "comment", "12", "--body-file", "-"}},
		{name: "repo", args: []string{"pr", "edit", "12", "--repo", "o/r"}, repo: "o/r", target: "12"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			req, err := parse(tc.args, stdin)

			if err != nil {
				t.Fatal(err)
			}
			ghArgs := tc.ghArgs
			if ghArgs == nil {
				ghArgs = tc.args
			}
			if req.Title != tc.title || req.Body != tc.body || req.Repo != tc.repo || req.Target != tc.target {
				t.Errorf("got title %q body %q repo %q target %q", req.Title, req.Body, req.Repo, req.Target)
			}
			if !slices.Equal(req.Args, ghArgs) {
				t.Errorf("gh args = %q, want %q", req.Args, ghArgs)
			}
		})
	}
}

// Browser writes, unknown operations, and incomplete flags must never reach approval.
func TestParseRejectsUnsafeRequests(t *testing.T) {
	for _, args := range [][]string{
		{"pr", "create", "--web"},
		{"pr", "create", "-w"},
		{"pr", "publish", "12"},
		{"pr", "create", "--title"},
	} {
		if _, err := parse(args, nil); err == nil {
			t.Errorf("parse(%q) succeeded, want an error", args)
		}
	}
}

// An answer covers one exact request. Rewriting a body file and rerunning the
// same command line must need a new answer, not reuse the old approval.
func TestRequestIDChangesWithPostedText(t *testing.T) {
	bodyPath := filepath.Join(t.TempDir(), "body.md")
	args := []string{"pr", "comment", "12", "--body-file", bodyPath}
	var ids []string
	for _, body := range []string{"approved text", "rewritten text"} {
		if err := os.WriteFile(bodyPath, []byte(body), 0o600); err != nil {
			t.Fatal(err)
		}
		req, err := parse(args, nil)
		if err != nil {
			t.Fatal(err)
		}
		ids = append(ids, req.ID)
	}

	if ids[0] == ids[1] {
		t.Errorf("both body file contents got request ID %s", ids[0])
	}
}

// The approval screen shows a PR's old title and body next to the new ones,
// so the program must find the PR number even when options come first, as in
// gh pr edit --base main 12. Taking main as the PR would show the wrong PR's
// text or none. An option it does not know might take the number as its value,
// so then it reports the number as unknown instead of guessing.
func TestEditFindsPRNumberAfterOptions(t *testing.T) {
	cases := []struct {
		name    string
		args    []string
		target  string
		unknown bool
	}{
		{name: "number first", args: []string{"12", "--base", "main"}, target: "12"},
		{name: "option with value before number", args: []string{"--base", "main", "12"}, target: "12"},
		{name: "option=value before number", args: []string{"--add-label=bug", "12"}, target: "12"},
		{name: "option without value before number", args: []string{"--remove-milestone", "12"}, target: "12"},
		{name: "unlisted option before number", args: []string{"--new-flag", "x", "12"}, unknown: true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			req, err := parse(append([]string{"pr", "edit"}, tc.args...), nil)

			if err != nil {
				t.Fatal(err)
			}
			if req.Target != tc.target || req.TargetUnknown != tc.unknown {
				t.Errorf("target = %q, unknown = %t; want %q, %t", req.Target, req.TargetUnknown, tc.target, tc.unknown)
			}
		})
	}
}
