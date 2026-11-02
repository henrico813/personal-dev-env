package view

import "testing"

// Descriptions and comments wrap to the screen's width in less, here 30
// columns. A list item must wrap under its own text, and code in a fence
// must stay as written, or wrapped code reads as different code.
func TestWrapProseHangsItemsKeepsCode(t *testing.T) {
	tests := []struct {
		name string
		text string
		want string
	}{
		{
			name: "list item",
			text: "- keep the checks in view when the keyboard is up",
			want: "- keep the checks in view when\n  the keyboard is up",
		},
		{
			name: "fenced code",
			text: "```\nif x { return a very long line that must not wrap }\n```",
			want: "```\nif x { return a very long line that must not wrap }\n```",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := WrapProse(
				tt.text,
				30,
			)

			if got != tt.want {
				t.Errorf(
					"wrapped to\n%s\nwant\n%s",
					got,
					tt.want,
				)
			}
		})
	}
}
