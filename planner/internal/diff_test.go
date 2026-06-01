package internal

import (
	"strings"
	"testing"
)

func stripANSI(s string) string {
	replacer := strings.NewReplacer(diffColorRed, "", diffColorGreen, "", diffColorReset, "")
	return replacer.Replace(s)
}

func TestDiffLines(t *testing.T) {
	tests := []struct {
		name       string
		a          string
		b          string
		want       string
		wantEmpty  bool
		wantColors bool
	}{
		{
			name:      "equal",
			a:         "a\nb\n",
			b:         "a\nb\n",
			wantEmpty: true,
		},
		{
			name: "replacement",
			a:    "a\nold\nc\n",
			b:    "a\nnew\nc\n",
			want: "  a\n- old\n+ new\n  c\n",
		},
		{
			name: "trailing_addition",
			a:    "a\n",
			b:    "a\nb\n",
			want: "  a\n+ b\n",
		},
		{
			name:       "changed_lines_are_colored",
			a:          "old\n",
			b:          "new\n",
			want:       "- old\n+ new\n",
			wantColors: true,
		},
		{
			name:      "newline_only_removed",
			a:         "a\n",
			b:         "a",
			wantEmpty: true,
		},
		{
			name:      "newline_only_added",
			a:         "a",
			b:         "a\n",
			wantEmpty: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := diffLines(tc.a, tc.b)
			if tc.wantEmpty {
				if got != "" {
					t.Fatalf("want empty, got %q", got)
				}
				return
			}
			if stripANSI(got) != tc.want {
				t.Fatalf("diff mismatch:\n%q\nvs\n%q", got, tc.want)
			}
			if tc.wantColors {
				if !strings.Contains(got, diffColorRed) {
					t.Fatalf("diff missing red color: %q", got)
				}
				if !strings.Contains(got, diffColorGreen) {
					t.Fatalf("diff missing green color: %q", got)
				}
				if !strings.Contains(got, diffColorReset) {
					t.Fatalf("diff missing reset color: %q", got)
				}
			}
		})
	}
}
