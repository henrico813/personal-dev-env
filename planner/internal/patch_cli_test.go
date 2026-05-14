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

func TestPatchParsesImplementationStepSelector(t *testing.T) {
	parsed, err := parsePatchFieldSelector("implementation[2].summary")
	if err != nil {
		t.Fatalf("parsePatchFieldSelector: %v", err)
	}
	if parsed.Kind != patchFieldSelectorStep || parsed.StepIndex != 2 || parsed.Field != "summary" {
		t.Fatalf("unexpected selector: %#v", parsed)
	}
}

func TestPatchParsesFileChangeSelector(t *testing.T) {
	parsed, err := parsePatchFieldSelector("implementation[1].file_changes[3].explanation")
	if err != nil {
		t.Fatalf("parsePatchFieldSelector: %v", err)
	}
	if parsed.Kind != patchFieldSelectorFileChange || parsed.StepIndex != 1 || parsed.FileChangeIndex != 3 || parsed.Field != "explanation" {
		t.Fatalf("unexpected selector: %#v", parsed)
	}
}

func TestPatchRejectsZeroOrMalformedSelectorIndex(t *testing.T) {
	for _, selector := range []string{
		"implementation[0].summary",
		"implementation[-1].summary",
		"implementation[bogus].summary",
	} {
		if _, err := parsePatchFieldSelector(selector); err == nil {
			t.Fatalf("expected parse failure for %q", selector)
		}
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

func TestPatchUpdatesImplementationStepSummary(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Update Field: implementation[1].summary\n-S\n+Renamed summary\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", sourcePath}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})

	parsed := parseOutputPlan(t, sourcePath)
	if parsed.Implementation[0].Summary != "Renamed summary" {
		t.Fatalf("summary=%q", parsed.Implementation[0].Summary)
	}
}

func TestPatchUpdatesFileChangeFields(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Update Field: implementation[1].file_changes[1].filename\n-f\n+renamed.go\n*** Update Field: implementation[1].file_changes[1].explanation\n-e\n+updated explanation\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", sourcePath}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})

	parsed := parseOutputPlan(t, sourcePath)
	if parsed.Implementation[0].FileChanges[0].Filename != "renamed.go" {
		t.Fatalf("filename=%q", parsed.Implementation[0].FileChanges[0].Filename)
	}
	if parsed.Implementation[0].FileChanges[0].Explanation != "updated explanation" {
		t.Fatalf("explanation=%q", parsed.Implementation[0].FileChanges[0].Explanation)
	}
}

func TestPatchRejectsNestedMismatch(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Update Field: implementation[1].summary\n-wrong\n+Renamed\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", "--json-errors", sourcePath}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})
	code, msg := firstStderrJSON(t, &stderr)
	if code != "VALIDATE_INPUT" {
		t.Fatalf("code=%q want VALIDATE_INPUT", code)
	}
	if !strings.Contains(msg, "patch old value mismatch") {
		t.Fatalf("message %q missing mismatch detail", msg)
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

func TestPatchRejectsMixedFieldOrder(t *testing.T) {
	_, err := parsePlannerPatch("*** Begin Patch\n*** Update Field: title\n+Renamed\n-T\n*** End Patch\n")
	if err == nil || !strings.Contains(err.Error(), "must list - lines before + lines") {
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
	if parsed.Plan.Verification == nil || parsed.Plan.Verification.Summary != "" {
		t.Fatalf("verification summary changed unexpectedly: %#v", parsed.Plan.Verification)
	}
}

func TestPatchRejectsUnsupportedAddItemSelector(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	patch := "*** Begin Patch\n*** Add Item: implementation.steps\n+oops\n*** End Patch\n"

	var stdout, stderr bytes.Buffer
	withStdin(t, []byte(patch), func() {
		if exit := Execute([]string{"patch", "--json-errors", sourcePath}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
		}
	})
	code, msg := firstStderrJSON(t, &stderr)
	if code != "VALIDATE_INPUT" {
		t.Fatalf("code=%q want VALIDATE_INPUT", code)
	}
	if !strings.Contains(msg, "unsupported checklist selector") {
		t.Fatalf("message %q missing unsupported checklist selector", msg)
	}
}

func TestPatchRejectsOutOfRangeNestedSelectors(t *testing.T) {
	for _, patch := range []string{
		"*** Begin Patch\n*** Update Field: implementation[2].title\n-T\n+Renamed\n*** End Patch\n",
		"*** Begin Patch\n*** Update Field: implementation[1].file_changes[2].filename\n-f\n+renamed.go\n*** End Patch\n",
	} {
		dir := t.TempDir()
		sourcePath := writeBehavioralPlan(t, dir)
		var stdout, stderr bytes.Buffer
		withStdin(t, []byte(patch), func() {
			if exit := Execute([]string{"patch", "--json-errors", sourcePath}, &stdout, &stderr); exit != 1 {
				t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
			}
		})
		code, msg := firstStderrJSON(t, &stderr)
		if code != "VALIDATE_INPUT" {
			t.Fatalf("code=%q want VALIDATE_INPUT", code)
		}
		if !strings.Contains(msg, "out of range") {
			t.Fatalf("message %q missing out-of-range detail", msg)
		}
	}
}
