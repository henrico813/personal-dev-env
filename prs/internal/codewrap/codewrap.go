// Package codewrap rewraps long lines of diffs and other text so each
// wrapped row starts 2 columns past its code's indentation, breaking where
// nvim breaks. fzf's own wrap starts wrapped rows at the left edge, under
// the line numbers, which is hard to read on a phone.
package codewrap

import (
	"regexp"
	"strings"
	"unicode/utf8"
)

// The characters nvim breaks a line after by default (breakat).
const breakAt = " !@*-+;:,./?"

var (
	escapeSequence = regexp.MustCompile(`^\x1b\[[0-9;]*[A-Za-z]`)
	// delta fills the rest of an added or removed row with its background
	// color, followed by an erase-to-end sequence.
	backgroundFill = regexp.MustCompile(`\x1b\[[0-9;]*m\x1b\[0?K`)
	// A diff line has a line number or blanks before its +/- mark.
	numberMargin = regexp.MustCompile(`^ *[0-9]* *$`)
)

// Returns text with every line longer than width columns rewrapped.
// Margin is the columns before the code, 6 for delta's narrow layout,
// 1 for a plain diff and 0 for other text.
func Wrap(
	text string,
	width,
	margin int,
) string {
	var out strings.Builder
	for _, line := range strings.SplitAfter(
		text,
		"\n",
	) {
		if line == "" {
			continue
		}
		// A carriage return would send the cursor back to the row's start.
		line = strings.TrimSuffix(
			strings.TrimSuffix(
				line,
				"\n",
			),
			"\r",
		)
		rows := wrapLine(
			line,
			width,
			margin,
		)
		out.WriteString(strings.Join(
			rows,
			"\n",
		) + "\n")
	}
	return out.String()
}

// Returns line as rows of at most width columns, with each tab filled to
// the next 2-column stop.
// Wrapped rows repeat the +/- mark, start 2 columns past the code's
// indentation, and keep the line's colors and background.
func wrapLine(
	line string,
	width,
	margin int,
) []string {
	tokens := splitTokens(line)
	var plain strings.Builder
	var whole strings.Builder
	for _, t := range tokens {
		whole.WriteString(t.text)
		if !t.isEscape {
			plain.WriteString(t.text)
		}
	}
	if utf8.RuneCountInString(plain.String()) <= width {
		return []string{whole.String()}
	}
	w := newWrapper(
		line,
		[]rune(plain.String()),
		width,
		margin,
	)
	for len(tokens) > 0 {
		count, pieceWidth := measurePiece(tokens)
		if w.col+pieceWidth > width && w.canBreak {
			w.startRow()
		}
		w.addPiece(tokens[:count])
		endsInBreak := tokens[count-1].canBreakAfter()
		tokens = tokens[count:]
		// Like nvim, a row may break after a break character past the
		// indentation, never inside it.
		if endsInBreak && w.col > w.codeCol {
			w.canBreak = true
		}
	}
	return append(
		w.rows,
		w.row.String(),
	)
}

// Holds a color escape sequence or one character, with tabs already
// turned to spaces.
type token struct {
	text     string
	isEscape bool
}

func (t token) canBreakAfter() bool {
	return !t.isEscape && len(t.text) == 1 && strings.Contains(
		breakAt,
		t.text,
	)
}

// Holds the line being wrapped and the row being built.
type wrapper struct {
	width int
	// Columns of the margin, or 0 when the line has none.
	lead int
	// The +/- mark, repeated on each wrapped row.
	mark string
	// The column where the line's code starts.
	codeCol int
	// The column where wrapped rows start their code.
	indent int
	// The background color that fills the rest of a changed row, or "".
	fill  string
	reset string
	// The color escape sequences in force, carried onto the next row.
	state    string
	rows     []string
	row      strings.Builder
	col      int
	canBreak bool
}

func newWrapper(
	line string,
	plain []rune,
	width,
	margin int,
) *wrapper {
	w := &wrapper{width: width}
	if margin > 0 && len(plain) >= margin &&
		numberMargin.MatchString(string(plain[:margin-1])) &&
		strings.ContainsRune(
			"-+ ",
			plain[margin-1],
		) {
		w.lead = margin
		w.mark = string(plain[margin-1])
	}
	w.codeCol = w.lead
	for w.codeCol < len(plain) && plain[w.codeCol] == ' ' {
		w.codeCol++
	}
	// Like nvim, keep at least 20 columns for the text.
	w.indent = max(
		min(
			w.codeCol+2,
			width-20,
		),
		w.lead,
	)
	if fill := backgroundFill.FindString(line); fill != "" {
		w.fill = fill[:strings.LastIndex(
			fill,
			"\x1b",
		)]
	}
	if strings.Contains(
		line,
		"\x1b",
	) {
		w.reset = "\x1b[0m"
	}
	return w
}

// Ends the row and starts the next one with the mark under the old mark,
// then blanks up to the hanging indent, in the line's background.
func (w *wrapper) startRow() {
	if w.fill != "" {
		w.row.WriteString(w.fill + "\x1b[0K")
	}
	w.rows = append(
		w.rows,
		w.row.String()+w.reset,
	)
	w.row.Reset()
	w.row.WriteString(strings.Repeat(
		" ",
		max(
			w.lead-1,
			0,
		),
	))
	w.row.WriteString(w.fill + w.mark + strings.Repeat(
		" ",
		w.indent-w.lead,
	))
	w.row.WriteString(w.reset + w.state)
	w.col = w.indent
	w.canBreak = false
}

// Adds a piece's tokens to the row, dropping a space at the edge. A color
// sequence is kept for the next row too.
func (w *wrapper) addPiece(tokens []token) {
	for _, t := range tokens {
		switch {
		case t.isEscape:
			w.row.WriteString(t.text)
			w.keepColor(t.text)
		case t.text == " " && w.col >= w.width:
		default:
			// A word longer than the row is broken here.
			if w.col >= w.width {
				w.startRow()
			}
			w.row.WriteString(t.text)
			w.col++
		}
	}
}

func (w *wrapper) keepColor(sequence string) {
	switch {
	case sequence == "\x1b[m" || sequence == "\x1b[0m":
		w.state = ""
	case strings.HasSuffix(
		sequence,
		"m",
	):
		w.state += sequence
	}
}

// Returns how many tokens the next piece has, up to and including the next
// character nvim may break after, and its width. A space at the end of a
// row may pass the edge, as in nvim.
func measurePiece(tokens []token) (
	count,
	width int,
) {
	for _, t := range tokens {
		count++
		if t.isEscape {
			continue
		}
		if t.canBreakAfter() {
			if t.text != " " {
				width++
			}
			break
		}
		width++
	}
	return count, width
}

// Returns line as color escape sequences and single characters, with each
// tab turned into spaces up to the next 2-column stop.
func splitTokens(line string) []token {
	var tokens []token
	col := 0
	for len(line) > 0 {
		if sequence := escapeSequence.FindString(line); sequence != "" {
			tokens = append(
				tokens,
				token{
					text:     sequence,
					isEscape: true,
				},
			)
			line = line[len(sequence):]
			continue
		}
		_, size := utf8.DecodeRuneInString(line)
		char := line[:size]
		line = line[size:]
		if char != "\t" {
			tokens = append(
				tokens,
				token{text: char},
			)
			col++
			continue
		}
		for stop := col + 2 - col%2; col < stop; col++ {
			tokens = append(
				tokens,
				token{text: " "},
			)
		}
	}
	return tokens
}
