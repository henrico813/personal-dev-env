package internal

import "testing"

func TestGenerateUnifiedDiffReturnsEmptyOnEqual(t *testing.T) {
	for _, tc := range []struct {
		name    string
		oldText string
		newText string
	}{
		{name: "same_text", oldText: "a\nb\n", newText: "a\nb\n"},
		{name: "trailing_newline_only", oldText: "a\n", newText: "a"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			if got := generateUnifiedDiff("plan.md", tc.oldText, tc.newText); got != "" {
				t.Fatalf("generateUnifiedDiff() = %q, want empty", got)
			}
		})
	}
}

func TestGenerateUnifiedDiffShowsWholeFileContext(t *testing.T) {
	got := generateUnifiedDiff("plan.md", "a\nold\nc\n", "a\nnew\nc\n")
	want := "--- plan.md\n+++ plan.md\n@@ -1,3 +1,3 @@\n a\n-old\n+new\n c\n"
	if got != want {
		t.Fatalf("generateUnifiedDiff() mismatch:\n%q\nvs\n%q", got, want)
	}
}
