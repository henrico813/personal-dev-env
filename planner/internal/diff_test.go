package internal

import (
	"fmt"
	"strings"
	"testing"
)

func TestDiffReturnsEmptyOnEqual(t *testing.T) {
	if got := diffLines("a\nb\n", "a\nb\n"); got != "" {
		t.Fatalf("want empty, got %q", got)
	}
}

func TestDiffShowsRemovalAndAddition(t *testing.T) {
	got := diffLines("a\nold\nc\n", "a\nnew\nc\n")
	want := "  a\n- old\n+ new\n  c\n"
	if got != want {
		t.Fatalf("diff mismatch:\n%q\nvs\n%q", got, want)
	}
}

func TestDiffHandlesTrailingAddition(t *testing.T) {
	got := diffLines("a\n", "a\nb\n")
	want := "  a\n+ b\n"
	if got != want {
		t.Fatalf("diff mismatch:\n%q\nvs\n%q", got, want)
	}
}

func TestDiffTrailingNewlineOnlyIsEqual(t *testing.T) {
	if got := diffLines("a\n", "a"); got != "" {
		t.Fatalf("trailing-newline-only diff should be empty, got %q", got)
	}
	if got := diffLines("a", "a\n"); got != "" {
		t.Fatalf("trailing-newline-only diff should be empty, got %q", got)
	}
}

func TestDiffPreservesSeparatedCommonLines(t *testing.T) {
	before := "header\nkeep one\nold first\nkeep middle\nold second\nkeep end\n"
	after := "header\nkeep one\nnew first\nkeep middle\nnew second\nkeep end\n"
	want := "  header\n" +
		"  keep one\n" +
		"- old first\n" +
		"+ new first\n" +
		"  keep middle\n" +
		"- old second\n" +
		"+ new second\n" +
		"  keep end\n"

	if got := diffLines(before, after); got != want {
		t.Fatalf("diffLines() mismatch\nwant:\n%s\ngot:\n%s", want, got)
	}
}

func TestDiffDeletesFirstOnTie(t *testing.T) {
	want := "- a\n  b\n+ a\n"

	if got := diffLines("a\nb\n", "b\na\n"); got != want {
		t.Fatalf("diffLines() mismatch\nwant:\n%s\ngot:\n%s", want, got)
	}
}

func TestDiffLimitIncludesSentinelCells(t *testing.T) {
	if !canUseDiffLCS(1999, 1999) {
		t.Fatal("expected 2000 by 2000 table at limit")
	}
	if canUseDiffLCS(1999, 2000) {
		t.Fatal("expected 2000 by 2001 table above limit")
	}
}

func TestDiffLimitAvoidsOverflow(t *testing.T) {
	maxInt := int(^uint(0) >> 1)
	if canUseDiffLCS(maxInt, 1) || canUseDiffLCS(1, maxInt) {
		t.Fatal("overflow-sized dimensions must not use LCS")
	}
}

func TestDiffFallsBackAboveLimit(t *testing.T) {
	before := make([]string, 2000)
	after := make([]string, 2000)
	for i := range before {
		before[i] = fmt.Sprintf("old-%d", i)
		after[i] = fmt.Sprintf("new-%d", i)
	}
	before[1000] = "shared"
	after[1000] = "shared"

	got := diffLines(strings.Join(before, "\n"), strings.Join(after, "\n"))
	if strings.Contains(got, "  shared\n") {
		t.Fatalf("fallback unexpectedly preserved shared line as context")
	}
	removed := strings.Index(got, "- shared\n")
	added := strings.Index(got, "+ shared\n")
	if removed < 0 || added < 0 || removed >= added {
		t.Fatalf("fallback should remove then add shared line")
	}
}
