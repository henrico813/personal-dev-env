package internal

import "testing"

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
