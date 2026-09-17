package internal

import "strings"

// A 4 million-cell table uses about 32 MiB on 64-bit systems.
const maxDiffLCSCells = 4_000_000

// diffLines produces a bounded per-line diff between a and b. It preserves
// common lines with LCS when the unmatched region fits the memory limit and
// falls back to remove-then-add output above the limit.
func diffLines(a, b string) string {
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
		writeDiffLine(&out, "  ", aLines[i])
	}

	aMiddle := aLines[head:aTail]
	bMiddle := bLines[head:bTail]
	switch {
	case len(aMiddle) == 0:
		for _, line := range bMiddle {
			writeDiffLine(&out, "+ ", line)
		}
	case len(bMiddle) == 0:
		for _, line := range aMiddle {
			writeDiffLine(&out, "- ", line)
		}
	case canUseDiffLCS(len(aMiddle), len(bMiddle)):
		writeLCSDiff(&out, aMiddle, bMiddle)
	default:
		for _, line := range aMiddle {
			writeDiffLine(&out, "- ", line)
		}
		for _, line := range bMiddle {
			writeDiffLine(&out, "+ ", line)
		}
	}

	for i := aTail; i < len(aLines); i++ {
		writeDiffLine(&out, "  ", aLines[i])
	}
	return out.String()
}

func canUseDiffLCS(aLen, bLen int) bool {
	maxInt := int(^uint(0) >> 1)
	if aLen < 0 || bLen < 0 || aLen == maxInt || bLen == maxInt {
		return false
	}
	rows := aLen + 1
	cols := bLen + 1
	return rows <= maxDiffLCSCells/cols
}

func writeLCSDiff(out *strings.Builder, aLines, bLines []string) {
	cols := len(bLines) + 1
	lcs := make([]int, (len(aLines)+1)*cols)
	for i := len(aLines) - 1; i >= 0; i-- {
		for j := len(bLines) - 1; j >= 0; j-- {
			idx := i*cols + j
			if aLines[i] == bLines[j] {
				lcs[idx] = lcs[(i+1)*cols+j+1] + 1
				continue
			}
			if lcs[(i+1)*cols+j] >= lcs[i*cols+j+1] {
				lcs[idx] = lcs[(i+1)*cols+j]
			} else {
				lcs[idx] = lcs[i*cols+j+1]
			}
		}
	}

	i, j := 0, 0
	for i < len(aLines) && j < len(bLines) {
		switch {
		case aLines[i] == bLines[j]:
			writeDiffLine(out, "  ", aLines[i])
			i++
			j++
		case lcs[(i+1)*cols+j] >= lcs[i*cols+j+1]:
			writeDiffLine(out, "- ", aLines[i])
			i++
		default:
			writeDiffLine(out, "+ ", bLines[j])
			j++
		}
	}
	for ; i < len(aLines); i++ {
		writeDiffLine(out, "- ", aLines[i])
	}
	for ; j < len(bLines); j++ {
		writeDiffLine(out, "+ ", bLines[j])
	}
}

func writeDiffLine(out *strings.Builder, prefix, line string) {
	out.WriteString(prefix)
	out.WriteString(line)
	out.WriteByte('\n')
}

func splitLines(s string) []string {
	if s == "" {
		return nil
	}
	return strings.Split(strings.TrimSuffix(s, "\n"), "\n")
}
