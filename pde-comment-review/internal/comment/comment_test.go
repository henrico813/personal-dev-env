package comment

import (
	"slices"
	"testing"
)

// The owner edits a comment as plain text, and that text goes back into the
// Go file. The Python prototype put each line of a code example in a
// paragraph of its own, so saving added a blank "//" line between them. This
// checks that an edit keeps the example's lines together and unchanged,
// rewraps the prose, and keeps the old comment's indentation.
func TestEditedTextBecomesGoComment(t *testing.T) {
	old := "\t// ParseConfig reads the config file at path and returns its\n" +
		"\t// settings.\n" +
		"\t//\n" +
		"\t//\tcfg, err := ParseConfig(\"app.conf\")\n" +
		"\t//\tfmt.Println(cfg.Port, err)\n"
	text := ToPlainText(old)
	if text != "ParseConfig reads the config file at path and returns its settings.\n\n"+
		"\tcfg, err := ParseConfig(\"app.conf\")\n"+
		"\tfmt.Println(cfg.Port, err)\n" {
		t.Fatalf("ToPlainText() = %q", text)
	}
	edited := "ParseConfig reads the config file at path and fills in its settings with a default for each key that the file leaves out.\n\n" +
		"\tcfg, err := ParseConfig(\"app.conf\")\n" +
		"\tfmt.Println(cfg.Port, err)\n"

	got := ToCommentLines(edited, old)

	// The first line exactly fills the 79-column limit, so the next word,
	// "a", must start the second line.
	want := "\t// ParseConfig reads the config file at path and fills in its settings with\n" +
		"\t// a default for each key that the file leaves out.\n" +
		"\t//\n" +
		"\t//\tcfg, err := ParseConfig(\"app.conf\")\n" +
		"\t//\tfmt.Println(cfg.Port, err)\n"
	if got != want {
		t.Fatalf("ToCommentLines() =\n%s\nwant\n%s", got, want)
	}
}

// The tool changes only comments, never code. A review replaces a comment's
// text in a Go file, so text that is not whole comment lines would change code
// or join a comment to the next line.
func TestCheckWholeLinesRefusesNonComments(t *testing.T) {
	tests := []struct {
		name    string
		text    string
		wantErr bool
	}{
		{name: "indented comment", text: "\t// One.\n\t//\n\t// Two.\n"},
		{name: "no final newline", text: "// One.", wantErr: true},
		{name: "code between comments", text: "// One.\nx := 1\n// Two.\n", wantErr: true},
		{name: "code example", text: "// One:\n//\tx := 1\n"},
		{name: "go directive", text: "// One.\n//go:generate stringer -type=Code\n", wantErr: true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := CheckWholeLines(tt.text)

			if (err != nil) != tt.wantErr {
				t.Fatalf("CheckWholeLines(%q) = %v, want error %v", tt.text, err, tt.wantErr)
			}
		})
	}
}

// The owner reads comments on a phone in a 40x16 Herdr popup. Live testing of
// the PR approval popup found 37 columns inside its border. A wider row wraps
// in the terminal and pushes the header and the key hint off the phone screen.
func TestScreenRowsFitPopupWidth(t *testing.T) {
	text := "\t// Save writes the answer to disk before the popup moves to the next item.\n" +
		"\t//\n" +
		"\t//\tsave(answer)\n"

	got := WrapForScreen(text, 37)

	// The second row is exactly 37 columns.
	want := []string{
		"// Save writes the answer to disk",
		"// before the popup moves to the next",
		"// item.",
		"//",
		"//\tsave(answer)",
	}
	if !slices.Equal(got, want) {
		t.Fatalf("WrapForScreen() = %q, want %q", got, want)
	}
}
