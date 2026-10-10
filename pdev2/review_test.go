package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func writeScript(t *testing.T, dir, name, body string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte("#!/bin/sh\n"+body+"\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	return path
}

// The Moshi phone-approval change deleted the bash diff helper while two calls
// remained, so every `pr edit` failed and no test noticed.
func TestEditReviewShowsCurrentAndRequested(t *testing.T) {
	gh := writeScript(t, t.TempDir(), "gh", `echo '{"title":"old","body":"same"}'`)
	cases := []struct {
		name  string
		title string
		want  []string
	}{
		{name: "changed title", title: "new", want: []string{"-old", "+new", "No changes (body not in this command)"}},
		{name: "unchanged title", title: "old", want: []string{"Title:\nNo changes\n"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			req := request{Operation: "edit", Target: "12", Title: tc.title, HasTitle: true}

			text, err := approvalText(gh, req)

			if err != nil {
				t.Fatal(err)
			}
			for _, want := range tc.want {
				if !strings.Contains(text, want) {
					t.Errorf("review is missing %q:\n%s", want, text)
				}
			}
		})
	}
}

// A fetch failure must be shown to the approver instead of blocking the write.
func TestEditReviewShowsFetchFailure(t *testing.T) {
	gh := writeScript(t, t.TempDir(), "gh", "echo boom >&2; exit 1")
	req := request{Operation: "edit", Target: "12", Title: "new", HasTitle: true}

	text, err := approvalText(gh, req)

	if err != nil || !strings.Contains(text, "Fetch failed: boom\n") {
		t.Fatalf("review = %q, %v; want the fetch error in the text", text, err)
	}
}

// The fetch runs under the request lock; a hung gh would block every rerun.
func TestEditReviewStopsHungFetch(t *testing.T) {
	// The fake gh never answers; the shortened timeout keeps the test fast.
	gh := writeScript(t, t.TempDir(), "gh", "exec sleep 10")
	oldTimeout := fetchTimeout
	fetchTimeout = 100 * time.Millisecond
	t.Cleanup(func() { fetchTimeout = oldTimeout })
	req := request{Operation: "edit", Target: "12", Title: "new", HasTitle: true}

	text, err := approvalText(gh, req)

	if err != nil || !strings.Contains(text, "Fetch failed: gh pr view timed out") {
		t.Fatalf("review = %q, %v; want the timeout in the text", text, err)
	}
}
