// Package comment reads and writes Go line comments. It is the only code in
// pde-comment-review that knows comment syntax.
package comment

import "unicode/utf8"

// Joins words with single spaces into lines of at most width characters.
// A word longer than width goes on a line of its own, so that line is
// wider than width.
func WrapWords(words []string, width int) []string {
	var lines []string
	line := ""
	for _, word := range words {
		if line != "" && utf8.RuneCountInString(line)+1+utf8.RuneCountInString(word) > width {
			lines = append(lines, line)
			line = ""
		}
		if line == "" {
			line = word
		} else {
			line += " " + word
		}
	}
	if line != "" {
		lines = append(lines, line)
	}
	return lines
}
