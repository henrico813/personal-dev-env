package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

// prs renders gh's JSON with jq, so these tests fake gh but run the real jq.

var prsColorCodes = regexp.MustCompile(`\x1b\[[0-9;]*m`)

const prsViewJSON = `{
  "number": 12,
  "title": "Fix login timeout",
  "url": "https://github.com/example/repo/pull/12",
  "state": "OPEN",
  "isDraft": false,
  "author": {"login": "alice"},
  "baseRefName": "main",
  "headRefName": "fix/login",
  "reviewDecision": "",
  "mergeStateStatus": "BLOCKED",
  "additions": 3,
  "deletions": 1,
  "changedFiles": 1,
  "labels": [],
  "statusCheckRollup": [
    {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS"},
    {"__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE"},
    {"__typename": "CheckRun", "name": "e2e", "status": "IN_PROGRESS", "conclusion": ""},
    {"__typename": "StatusContext", "context": "deploy", "state": "PENDING"},
    {"__typename": "StatusContext", "context": "legacy", "state": "SUCCESS"}
  ],
  "updatedAt": "2026-10-19T01:36:40Z",
  "files": [{"path": "login.go", "additions": 3, "deletions": 1}],
  "body": "Raise the timeout."
}`

const prsFeedbackJSON = `{"data": {"repository": {"pullRequest": {
  "comments": {"nodes": []},
  "reviews": {"nodes": [
    {"author": {"login": "bob"}, "state": "APPROVED", "submittedAt": "2026-10-18T09:00:00Z", "body": ""},
    {"author": {"login": "alice"}, "state": "PENDING", "submittedAt": null, "body": "unsent"}
  ]},
  "reviewThreads": {"nodes": [
    {"isResolved": true, "isOutdated": false, "path": "a.go", "line": 1, "originalLine": 1,
     "comments": {"nodes": [{"author": {"login": "bob"}, "createdAt": "2026-10-18T08:00:00Z", "body": "fixed now", "diffHunk": "@@ -1 +1 @@\n+a"}]}},
    {"isResolved": false, "isOutdated": false, "path": "b.go", "line": 3, "originalLine": 3,
     "comments": {"nodes": [{"author": {"login": "bob"}, "createdAt": "2026-10-18T08:30:00Z", "body": "why 30s?", "diffHunk": "@@ -3 +3 @@\n+timeout := 30"}]}}
  ]}
}}}}`

// Verifies that prs reads CI checks correctly.
func TestPRsCountsChecksByOutcome(t *testing.T) {
	output := runPRsFullView(t)

	for _, want := range []string{
		"checks: 2 pass, 1 fail, 2 pending\n",
		"  fail: lint\n",
		"  pending: e2e\n",
		"  pending: deploy\n",
	} {
		if !strings.Contains(output, want) {
			t.Errorf(
				"output omits %q:\n%s",
				want,
				output,
			)
		}
	}
}

// Verifies that prs lists unresolved comments on lines of code before
// resolved ones, and shows each resolved one as a single line.
func TestPRsListsUnresolvedLineCommentsFirst(t *testing.T) {
	output := runPRsFullView(t)

	open := strings.Index(output, "## b.go:3 OPEN\n")
	resolved := strings.Index(output, "## a.go:1 resolved\n")
	if open < 0 || resolved < 0 || open > resolved {
		t.Errorf(
			"open thread at %d, resolved at %d; want open first:\n%s",
			open,
			resolved,
			output,
		)
	}
	if strings.Contains(output, "fixed now") {
		t.Errorf("output shows the resolved thread's comment:\n%s", output)
	}
}

// Verifies that prs shows approvals, even without text, and skips your
// unsent draft review, which once broke this view.
func TestPRsShowsApprovalsAndSkipsDrafts(t *testing.T) {
	output := runPRsFullView(t)

	if !strings.Contains(output, "-- bob APPROVED 2026-10-18 09:00\n") {
		t.Errorf("output omits the approval:\n%s", output)
	}
	if strings.Contains(output, "unsent") {
		t.Errorf("output shows the unsent draft review:\n%s", output)
	}
}

func runPRsFullView(t *testing.T) string {
	t.Helper()
	if _, err := exec.LookPath("jq"); err != nil {
		t.Skip("jq is not on PATH; the installer provides it through aqua, so install first")
	}
	home := t.TempDir()
	bin := filepath.Join(
		home,
		".local",
		"bin",
	)
	writeApplyFile(
		t,
		filepath.Join(home, "view.json"),
		prsViewJSON,
	)
	writeApplyFile(
		t,
		filepath.Join(home, "feedback.json"),
		prsFeedbackJSON,
	)
	writeExecutable(
		t,
		filepath.Join(bin, "gh"),
		`#!/bin/sh
if [ "$1" = api ]; then cat "$HOME/feedback.json"; else cat "$HOME/view.json"; fi
`,
	)
	writeExecutable(
		t,
		filepath.Join(bin, "bat"),
		"#!/bin/sh\ncat\n",
	)

	script := filepath.Join(
		repoRoot(t),
		"chezmoi",
		"dot_local",
		"bin",
		"executable_prs",
	)
	command := exec.Command(
		"bash",
		script,
		"example/repo",
		"12",
	)
	command.Env = append(
		os.Environ(),
		"HOME="+home,
		"PATH="+bin+":"+os.Getenv("PATH"),
		"NO_COLOR=",
	)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf(
			"prs failed: %v\n%s",
			err,
			output,
		)
	}
	return prsColorCodes.ReplaceAllString(string(output), "")
}
