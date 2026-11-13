package internal

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Agents pick what to read from these line ranges, so a wrong range, or a
// heading taken from inside a code block, sends them to the wrong lines.
func TestOutlineListsPartsWithTheirLines(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	exit := Execute(
		[]string{
			"outline",
			"testdata/outline_plan.md",
		},
		&stdout,
		&stderr,
	)

	want := "8-26      ## Overview\n" +
		"23-26       ### Limits\n" +
		"27-32     ## Definition of Done\n" +
		"30-32       ### Goals\n" +
		"33-63     ## Implementation\n" +
		"36-49       ### 1. Add the greeting\n" +
		"40-49         `hello.go`\n" +
		"50-63       ### 2. Test the greeting\n" +
		"54-63         `hello_test.go`\n" +
		"64-69     ## Verification\n" +
		"67-69       ### Automated Verification\n" +
		"70-79     ## Notes for agents\n" +
		"73-76       ### Limits\n" +
		"77-79       ### 3. Old step\n"
	if exit != 0 || stdout.String() != want {
		t.Errorf(
			"exit %d, stderr %q, stdout\n%s\nwant\n%s",
			exit,
			stderr.String(),
			stdout.String(),
			want,
		)
	}
}

// Agents read one step or section instead of the whole plan, so the text must
// start at its heading and stop before the next part.
func TestShowPrintsOnePlanPart(t *testing.T) {
	tests := []struct {
		name    string
		args    []string
		wantOut string
	}{
		{
			name: "step",
			args: []string{
				"--step",
				"2",
			},
			wantOut: "### 2. Test the greeting\n\nAdds a test.\n\n" +
				"`hello_test.go`\n> Tests Hello.\n\n```diff\n" +
				"--- /dev/null\n+++ b/hello_test.go\n@@ -0,0 +1 @@\n" +
				"+package hello\n```\n\n",
		},
		{
			name: "section in any letter case",
			args: []string{
				"--section",
				"verification",
			},
			wantOut: "## Verification\n---\n\n" +
				"### Automated Verification\n- [ ] go test ./...\n\n",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			exit, stdout, stderr := runShowOnSamplePlan(tc.args)

			if exit != 0 || stdout != tc.wantOut {
				t.Errorf(
					"exit %d, stderr %q, stdout %q; want exit 0, stdout %q",
					exit,
					stderr,
					stdout,
					tc.wantOut,
				)
			}
		})
	}
}

// Agents guess step numbers and section names, so a guess that matches no
// part or two parts must fail and print nothing rather than the wrong text.
func TestShowFailsWithoutOneMatchingPart(t *testing.T) {
	tests := []struct {
		name     string
		args     []string
		wantExit int
		wantErr  string
	}{
		{
			name: "step only outside Implementation",
			args: []string{
				"--step",
				"3",
			},
			wantExit: 1,
			wantErr:  "no step 3",
		},
		{
			name: "two sections with one name",
			args: []string{
				"--section",
				"Limits",
			},
			wantExit: 1,
			wantErr:  "2 sections are named",
		},
		{
			name: "both step and section",
			args: []string{
				"--step",
				"2",
				"--section",
				"Goals",
			},
			wantExit: 2,
			wantErr:  "exactly one",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			exit, stdout, stderr := runShowOnSamplePlan(tc.args)

			if exit != tc.wantExit || stdout != "" || !strings.Contains(stderr, tc.wantErr) {
				t.Errorf(
					"exit %d, stdout %q, stderr %q; want exit %d, no stdout, stderr containing %q",
					exit,
					stdout,
					stderr,
					tc.wantExit,
					tc.wantErr,
				)
			}
		})
	}
}

// Runs planner show on the sample plan with the given flags.
func runShowOnSamplePlan(args []string) (int, string, string) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	exit := Execute(
		append(
			[]string{
				"show",
				"testdata/outline_plan.md",
			},
			args...,
		),
		&stdout,
		&stderr,
	)
	return exit, stdout.String(), stderr.String()
}

// A draft plan can leave a code block open, which hides every later heading.
// Outline and show must fail and name the line, not print a short outline or
// the wrong part with exit 0.
func TestOutlineFailsOnUnclosedCodeBlock(t *testing.T) {
	path := filepath.Join(
		t.TempDir(),
		"plan.md",
	)
	plan := "## Overview\n\n```text\n## Implementation\n\n### 1. One\n"
	if err := os.WriteFile(
		path,
		[]byte(plan),
		0o644,
	); err != nil {
		t.Fatal(err)
	}

	for _, args := range [][]string{
		{
			"outline",
			path,
		},
		{
			"show",
			path,
			"--section",
			"Overview",
		},
	} {
		var stdout bytes.Buffer
		var stderr bytes.Buffer

		exit := Execute(
			args,
			&stdout,
			&stderr,
		)

		if exit != 1 || stdout.String() != "" || strings.Count(stderr.String(), "line 3 never closes") != 1 {
			t.Errorf(
				"%s: exit %d, stdout %q, stderr %q; want exit 1, no stdout, stderr naming line 3 once",
				args[0],
				exit,
				stdout.String(),
				stderr.String(),
			)
		}
	}
}
