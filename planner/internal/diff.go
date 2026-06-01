package internal

import (
	"io"
	"os"
	"strings"
)

const (
	diffColorReset = "\x1b[0m"
	diffColorRed   = "\x1b[31m"
	diffColorGreen = "\x1b[32m"
)

// diffLines produces a minimal per-line diff between a and b. Identical lines
// are shown with a leading "  ", lines only in a with "- ", lines only in b
// with "+ ". Callers use this to preview planner output before write/dry-run;
// the output is readable by humans and easy to assert against in tests.
func diffLines(a, b string) string {
	return diffLinesColor(a, b, true)
}

func diffLinesForWriter(a, b string, w io.Writer) string {
	return diffLinesColor(a, b, writerSupportsColor(w))
}

func writerSupportsColor(w io.Writer) bool {
	f, ok := w.(*os.File)
	if !ok {
		return false
	}
	info, err := f.Stat()
	if err != nil {
		return false
	}
	return info.Mode()&os.ModeCharDevice != 0
}

func diffLinesColor(a, b string, color bool) string {
	if a == b || strings.TrimRight(a, "\n") == strings.TrimRight(b, "\n") {
		return ""
	}
	aLines := splitLines(a)
	bLines := splitLines(b)

	head := 0
	for head < len(aLines) && head < len(bLines) && aLines[head] == bLines[head] {
		head++
	}
	aTail := len(aLines)
	bTail := len(bLines)
	for aTail > head && bTail > head && aLines[aTail-1] == bLines[bTail-1] {
		aTail--
		bTail--
	}

	var out strings.Builder
	for i := 0; i < head; i++ {
		out.WriteString("  ")
		out.WriteString(aLines[i])
		out.WriteByte('\n')
	}
	for i := head; i < aTail; i++ {
		if color {
			out.WriteString(diffColorRed)
		}
		out.WriteString("- ")
		out.WriteString(aLines[i])
		if color {
			out.WriteString(diffColorReset)
		}
		out.WriteByte('\n')
	}
	for i := head; i < bTail; i++ {
		if color {
			out.WriteString(diffColorGreen)
		}
		out.WriteString("+ ")
		out.WriteString(bLines[i])
		if color {
			out.WriteString(diffColorReset)
		}
		out.WriteByte('\n')
	}
	for i := aTail; i < len(aLines); i++ {
		out.WriteString("  ")
		out.WriteString(aLines[i])
		out.WriteByte('\n')
	}
	return out.String()
}

func splitLines(s string) []string {
	if s == "" {
		return nil
	}
	return strings.Split(strings.TrimSuffix(s, "\n"), "\n")
}
