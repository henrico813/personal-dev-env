package github

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

// The search reply nests labels and checks under nodes and the last
// commit, as GitHub's GraphQL API sends them.
const searchReply = `{"data": {"search": {"nodes": [{"number": 12, "title": "Fix login",
  "repository": {"nameWithOwner": "example/repo"},
  "labels": {"nodes": [{"name": "bug"}]},
  "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": [
    {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "FAILURE"},
    {"__typename": "StatusContext", "context": "deploy", "state": "PENDING"}
  ]}}}}]}}]}}}`

// The details read checks and labels from the PR list's one query. If the
// nesting is lost, every pull request shows no checks and looks safe to
// merge.
func TestListPullRequestsReadsNestedFields(t *testing.T) {
	useGHReply(
		t,
		searchReply,
	)

	prs, err := ListPullRequests(context.Background())

	if err != nil {
		t.Fatal(err)
	}
	// Each pull request reads as its repo, labels and checks.
	var got []string
	for _, pr := range prs {
		row := pr.Repo()
		for _, label := range pr.Labels {
			row += " label " + label.Name
		}
		for _, check := range pr.StatusCheckRollup {
			row += ", " + check.Label() + " " + check.Result()
		}
		got = append(
			got,
			row,
		)
	}
	want := "[example/repo label bug, build fail, deploy pending]"
	if fmt.Sprint(got) != want {
		t.Errorf(
			"pull requests read as %v, want %s",
			got,
			want,
		)
	}
}

// The reply for the description, files and comments nests each code
// thread's comments under nodes, as GitHub's GraphQL API sends them.
const extrasReply = `{"data": {"repository": {"pullRequest": {"body": "Fixes login.",
  "reviewThreads": {"nodes": [{"isResolved": false, "path": "a.go", "line": 3,
    "comments": {"nodes": [{"author": {"login": "bob"}, "body": "Why?", "diffHunk": "@@ -1 +1 @@"}]}}]}}}}}`

// A code thread's comments decode through a field that shadows Thread's
// own. If the shadowing breaks, every thread on the comments screen shows
// no comments, and the cache saves them empty.
func TestFetchExtrasReadsThreadComments(t *testing.T) {
	useGHReply(
		t,
		extrasReply,
	)

	extras, err := FetchExtras(
		context.Background(),
		"example/repo",
		12,
	)

	if err != nil {
		t.Fatal(err)
	}
	var got []string
	for _, thread := range extras.Feedback.Threads {
		for _, comment := range thread.Comments {
			got = append(
				got,
				thread.Path+" "+comment.Author.Login+" "+comment.Body,
			)
		}
	}
	if fmt.Sprint(got) != "[a.go bob Why?]" {
		t.Errorf(
			"thread comments are %v, want [a.go bob Why?]",
			got,
		)
	}
}

// Puts a fake gh first on PATH for the rest of the test, which prints
// reply for any call.
func useGHReply(
	t *testing.T,
	reply string,
) {
	t.Helper()
	bin := t.TempDir()
	if err := os.WriteFile(
		filepath.Join(
			bin,
			"gh",
		),
		[]byte("#!/bin/sh\necho '"+reply+"'\n"),
		0o755,
	); err != nil {
		t.Fatal(err)
	}
	t.Setenv(
		"PATH",
		bin+":"+os.Getenv("PATH"),
	)
}
