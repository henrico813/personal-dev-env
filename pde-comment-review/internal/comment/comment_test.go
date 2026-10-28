package comment

import (
	"slices"
	"testing"
)

// The owner edits a comment as plain text, and that text goes back into the
// Go file. Saving once added a blank "//" line between the lines of a code
// example, because each line became a paragraph of its own.
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
