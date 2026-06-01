package internal

import (
	"fmt"
	"strings"
)

// generateUnifiedDiff renders a single-file unified diff from the exact file
// name plus the old and new text bodies. Callers use this to preview patch
// output without having to guess at the output format.
func generateUnifiedDiff(filename, oldText, newText string) string {
	if oldText == newText || strings.TrimRight(oldText, "\n") == strings.TrimRight(newText, "\n") {
		return ""
	}

	oldLines := splitLines(oldText)
	newLines := splitLines(newText)

	head := 0
	for head < len(oldLines) && head < len(newLines) && oldLines[head] == newLines[head] {
		head++
	}
	oldTail := len(oldLines)
	newTail := len(newLines)
	for oldTail > head && newTail > head && oldLines[oldTail-1] == newLines[newTail-1] {
		oldTail--
		newTail--
	}

	var out strings.Builder
	oldStart := 1
	if len(oldLines) == 0 {
		oldStart = 0
	}
	newStart := 1
	if len(newLines) == 0 {
		newStart = 0
	}
	fmt.Fprintf(&out, "--- %s\n+++ %s\n@@ -%d,%d +%d,%d @@\n", filename, filename, oldStart, len(oldLines), newStart, len(newLines))

	for i := 0; i < head; i++ {
		out.WriteByte(' ')
		out.WriteString(oldLines[i])
		out.WriteByte('\n')
	}
	for i := head; i < oldTail; i++ {
		out.WriteByte('-')
		out.WriteString(oldLines[i])
		out.WriteByte('\n')
	}
	for i := head; i < newTail; i++ {
		out.WriteByte('+')
		out.WriteString(newLines[i])
		out.WriteByte('\n')
	}
	for i := oldTail; i < len(oldLines); i++ {
		out.WriteByte(' ')
		out.WriteString(oldLines[i])
		out.WriteByte('\n')
	}

	return out.String()
}
