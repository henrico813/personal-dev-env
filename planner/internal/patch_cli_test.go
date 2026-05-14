package internal

import (
	"bytes"
	"os"
	"strings"
	"testing"
)

func TestPatchParsesTrailingNewline(t *testing.T) {
	ops, err := parsePlannerPatch("*** Begin Patch\n*** Update Field: title\n-T\n+Renamed\n*** End Patch\n")
	if err != nil {
		t.Fatalf("parsePlannerPatch: %v", err)
	}
	if len(ops) != 1 || ops[0].Selector != "title" {
		t.Fatalf("unexpected ops: %+v", ops)
	}
}

func TestPatchParsesMultiOp(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Update Field: title\n-T\n+Renamed\n*** Add Item: verification.manual\n+Added manual check\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", sourcePath}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})

	parsed := parseOutputPlan(t, sourcePath)
	if parsed.Title != "Renamed" {
		t.Fatalf("title=%q", parsed.Title)
	}
	if got := parsed.Verification.Manual[len(parsed.Verification.Manual)-1].Text; got != "Added manual check" {
		t.Fatalf("manual checklist not appended: %q", got)
	}
}

func TestPatchRejectsChecklistNewline(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Add Item: verification.manual\n+line one\n+line two\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", sourcePath}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})
	if !strings.Contains(stderr.String(), "single-line") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestPatchRejectsFlagArgs(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"patch", sourcePath, "--diff"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
	}
	if !strings.Contains(stderr.String(), "usage: planner patch <plan.md> [<out.md>]") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestPatchRejectsUnknownHeader(t *testing.T) {
	_, err := parsePlannerPatch("*** Begin Patch\n*** Wrong Header: title\n+oops\n*** End Patch\n")
	if err == nil || !strings.Contains(err.Error(), "unknown patch header") {
		t.Fatalf("unexpected err: %v", err)
	}
}

func TestPatchAllowsEmptyVerificationSummary(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Update Field: verification.summary\n-\n+\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", sourcePath}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})

	raw, err := os.ReadFile(sourcePath)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	if parsed.Verification == nil || parsed.Verification.Summary != "" {
		t.Fatalf("verification summary changed unexpectedly: %#v", parsed.Verification)
	}
}
