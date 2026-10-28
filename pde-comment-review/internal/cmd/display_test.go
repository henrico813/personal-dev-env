package cmd

import (
	"bytes"
	"encoding/json"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"pde-comment-review/internal/comment"
	"pde-comment-review/internal/popup"
)

// The screen will draw Go comments from the checker's rows instead of
// WrapForScreen, so the owner must see the same rows at every width.
func TestCheckerRowsMatchWrapForScreen(t *testing.T) {
	buildRepoCheckerOnPath(t)
	checker, err := findCheckerPath()
	if err != nil {
		t.Fatal(err)
	}
	root, err := filepath.Abs(filepath.Join(
		"..",
		"..",
		"..",
	))
	if err != nil {
		t.Fatal(err)
	}
	files, err := exec.Command(
		"git",
		"-C",
		root,
		"ls-files",
		"*.go",
	).Output()
	if err != nil {
		t.Fatal(err)
	}

	// List every Go comment in the repository, then show them all.
	var listed struct {
		Files []struct {
			File     string `json:"file"`
			Comments []struct {
				Text string `json:"text"`
			} `json:"comments"`
		} `json:"files"`
	}
	runChecker(
		t,
		checker,
		root,
		nil,
		&listed,
		"list",
		strings.Fields(string(files))...,
	)
	var entries []map[string]string
	for _, file := range listed.Files {
		for _, c := range file.Comments {
			entries = append(entries, map[string]string{
				"file": file.File,
				"text": c.Text,
			})
		}
	}
	// No comment here has an example line, so these made-up ones hold the
	// example rows to WrapForScreen too.
	for _, text := range []string{
		"\t// Sums the prices.\n\t//\n\t//\tTotal(prices)\n\t//  return 0\n\t// In cents.\n",
		"// Example:\n//\tTotal()\n// After the example.\n",
	} {
		entries = append(entries, map[string]string{
			"file": "made_up.go",
			"text": text,
		})
	}
	request, err := json.Marshal(entries)
	if err != nil {
		t.Fatal(err)
	}
	var shown struct {
		Shown []struct {
			Text   string            `json:"text"`
			Screen []popup.ScreenRow `json:"screen"`
		} `json:"shown"`
	}
	runChecker(
		t,
		checker,
		root,
		request,
		&shown,
		"show",
	)

	if len(entries) < 1000 || len(shown.Shown) != len(entries) {
		t.Fatalf("listed %d comments and showed %d", len(entries), len(shown.Shown))
	}
	for _, s := range shown.Shown {
		for width := 20; width <= 80; width++ {
			want := comment.WrapForScreen(s.Text, width)
			if got := popup.DrawScreenRows(s.Screen, width); !reflect.DeepEqual(got, want) {
				t.Fatalf("at width %d, %q draws\n%q\nwant\n%q", width, s.Text, got, want)
			}
		}
	}
}

// Runs a checker command such as list or show in repo, feeding it stdin, and
// fills reply with the JSON the checker prints.
func runChecker(t *testing.T, checker, repo string, stdin []byte, reply any, command string, files ...string) {
	t.Helper()
	run := exec.Command(
		"python3",
		append([]string{checker, command, "--interface", "1", "--repo", repo}, files...)...,
	)
	run.Dir = repo
	run.Stdin = bytes.NewReader(stdin)
	var stderr bytes.Buffer
	run.Stderr = &stderr
	output, err := run.Output()
	if err != nil {
		t.Fatalf("pde-comment-checker %s: %v: %s", command, err, stderr.String())
	}
	if err := json.Unmarshal(output, reply); err != nil {
		t.Fatal(err)
	}
}
