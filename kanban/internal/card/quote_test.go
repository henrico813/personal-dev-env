package card

import "testing"

// Log lines and frontmatter list items hold these quotes, and the parity
// check compares them byte for byte with the Python card's. Each want is
// what Python's json.dumps printed for the text.
func TestQuoteASCIIMatchesPythonJSON(t *testing.T) {
	tests := []struct {
		name string
		text string
		want string
	}{
		{
			name: "umlaut",
			text: "Größe",
			want: `"Gr\u00f6\u00dfe"`,
		},
		{
			name: "past U+FFFF",
			text: "a\U0001F600",
			want: `"a\ud83d\ude00"`,
		},
		{
			name: "quote and backslash",
			text: `say "a\b"`,
			want: `"say \"a\\b\""`,
		},
		{
			name: "tab",
			text: "a\tb",
			want: `"a\tb"`,
		},
		{
			name: "delete character",
			text: "a\x7f",
			want: `"a\u007f"`,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := QuoteASCII(tt.text); got != tt.want {
				t.Errorf("QuoteASCII(%q) = %s, want %s", tt.text, got, tt.want)
			}
		})
	}
}
