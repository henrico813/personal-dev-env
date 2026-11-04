package codewrap

import (
	"strings"
	"testing"
)

// A wrapped row must start 2 columns past its code's indentation and keep
// the +/- mark, breaking after the characters nvim breaks after. Otherwise
// code lands under the line numbers or splits mid-name, which is hard to
// read on a phone.
func TestWrapLineHangsRowsUnderCode(t *testing.T) {
	tests := []struct {
		name string
		line string
		want string
	}{
		{
			name: "added line with a tab",
			line: "  49 +\tneeded := max(fenceLen, len(ChooseCodeFence(string(diff))))",
			want: "  49 +  needed := max(fenceLen, \n" +
				"     +    len(ChooseCodeFence(string(diff))))",
		},
		{
			name: "unchanged line breaks after a dot",
			line: "  46    if fenceLen < 3 || strings.TrimSpace(closing) != strings.Repeat(\"`\", fenceLen) {",
			want: "  46    if fenceLen < 3 || strings.\n" +
				"          TrimSpace(closing) != strings.\n" +
				"          Repeat(\"`\", fenceLen) {",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := wrapLine(
				tt.line,
				50,
				6,
			)

			if joined := strings.Join(
				got,
				"\n",
			); joined != tt.want {
				t.Errorf(
					"wrapped to\n%s\nwant\n%s",
					joined,
					tt.want,
				)
			}
		})
	}
}

// The code on a wrapped row must keep the added line's background, so the
// whole change still reads as added.
func TestWrapLineKeepsLineBackground(t *testing.T) {
	background := "\x1b[48;2;0;40;0m"
	line := "\x1b[34m  49 \x1b[0m" + background + "+\x1b[38;2;222;222;222m" +
		"\tneeded := max(fenceLen, len(ChooseCodeFence(string(diff))))" +
		"\x1b[0m" + background + "\x1b[0K\x1b[0m"

	rows := wrapLine(
		line,
		50,
		6,
	)

	codeStart := strings.Index(
		rows[1],
		"len(",
	)
	lastReset := strings.LastIndex(
		rows[1][:codeStart],
		"\x1b[0m",
	)
	if !strings.Contains(
		rows[1][lastReset:codeStart],
		background,
	) {
		t.Errorf(
			"wrapped row %q shows its code without the added background",
			rows[1],
		)
	}
}
