package view

import (
	"encoding/json"
	"strings"
	"testing"
	"time"

	"prs/internal/github"
)

// A pull request as the saved list holds it, with each kind of check result.
const pullRequestJSON = `{
  "number": 12,
  "repository": {"nameWithOwner": "example/repo"},
  "state": "OPEN",
  "author": {"login": "alice"},
  "reviewDecision": "",
  "mergeStateStatus": "BLOCKED",
  "statusCheckRollup": [
    {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS"},
    {"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE"},
    {"__typename": "CheckRun", "name": "e2e", "status": "IN_PROGRESS", "conclusion": ""},
    {"__typename": "StatusContext", "context": "deploy", "state": "PENDING"},
    {"__typename": "StatusContext", "context": "legacy", "state": "SUCCESS"}
  ],
  "updatedAt": "2026-10-31T10:00:00Z"
}`

// The status strip is all you see of a PR while the phone's keyboard is
// up. It must keep the plain review and merge words, the check counts and
// the repo in 3 rows, or a fourth row pushes the repo off the screen.
func TestRenderDetailsStripKeepsStatusRows(t *testing.T) {
	var pr github.PullRequest
	if err := json.Unmarshal(
		[]byte(pullRequestJSON),
		&pr,
	); err != nil {
		t.Fatal(err)
	}
	style := Style{
		Width: 50,
		Now: time.Date(
			2026,
			10,
			31,
			12,
			30,
			0,
			0,
			time.UTC,
		),
	}

	got := RenderDetails(
		pr,
		Strip,
		style,
	)

	want := "#12 OPEN · no review · blocked\n" +
		"checks: 2 pass, 1 fail, 2 pending\n" +
		"example/repo · 2h ago · alice\n"
	if got != want {
		t.Errorf(
			"strip is\n%s\nwant\n%s",
			got,
			want,
		)
	}
}

// The details name each check that did not pass, since the counts alone
// do not say which check to look at. Losing the names sends you to GitHub
// to find the failing one.
func TestRenderDetailsNamesChecksNotPassing(t *testing.T) {
	var pr github.PullRequest
	if err := json.Unmarshal(
		[]byte(pullRequestJSON),
		&pr,
	); err != nil {
		t.Fatal(err)
	}

	got := RenderDetails(
		pr,
		Long,
		Style{Width: 50},
	)

	want := "checks: 2 pass, 1 fail, 2 pending\n" +
		"  fail: lint\n" +
		"  pending: e2e\n" +
		"  pending: deploy\n"
	if !strings.HasSuffix(
		got,
		want,
	) {
		t.Errorf(
			"details are\n%s\nwant them to end with\n%s",
			got,
			want,
		)
	}
}
