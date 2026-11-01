// Package view turns GitHub data into the text prs shows, in plain words
// with relative times. Each screen's text is in the file named for it, such
// as RenderDetails in prlist.go and CollectItems in comments.go, and view.go
// holds the colors and wrapping they share.
package view

import (
	"regexp"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"
)

// The 16 basic colors, so the terminal's color profile picks the shades.
const (
	red     = "31"
	green   = "32"
	yellow  = "33"
	magenta = "35"
	cyan    = "36"
	gray    = "90"
	bold    = "1"
	blue    = "1;34"
)

// Holds plain words and the color they show in.
type coloredWords struct {
	words string
	color string
}

// GitHub's review decisions and single review states in plain words. The
// details read decisions, and comment bylines read states such as
// COMMENTED and DISMISSED.
var reviewWords = map[string]coloredWords{
	"APPROVED": {
		words: "approved",
		color: green,
	},
	"CHANGES_REQUESTED": {
		words: "changes asked",
		color: red,
	},
	"REVIEW_REQUIRED": {
		words: "needs review",
		color: yellow,
	},
	"COMMENTED": {
		words: "commented",
		color: gray,
	},
	"DISMISSED": {
		words: "dismissed",
		color: gray,
	},
}

// Holds what every view needs besides its data. Width is the columns that
// prose wraps to, and Now is the time that relative times count from.
type Style struct {
	Width int
	Now   time.Time
	Color bool
}

// Returns text wrapped at spaces to width columns, keeping lines inside
// ``` or ~~~ fences as they are. A list item wraps under its text, and
// other lines with leading spaces are code and are kept.
func WrapProse(
	text string,
	width int,
) string {
	lines := strings.Split(
		text,
		"\n",
	)
	inCode := false
	for i, line := range lines {
		if fencePattern.MatchString(line) {
			inCode = !inCode
		} else if !inCode {
			lines[i] = wrapProseLine(
				line,
				width,
			)
		}
	}
	return strings.Join(
		lines,
		"\n",
	)
}

var (
	fencePattern      = regexp.MustCompile("^\\s*(```|~~~)")
	listMarkerPattern = regexp.MustCompile(`^\s*(?:[-*+]|[0-9]+[.)]) `)
)

func wrapProseLine(
	line string,
	width int,
) string {
	marker := listMarkerPattern.FindString(line)
	hang := len(marker)
	isCode := hang == 0 && line != "" && (line[0] == ' ' || line[0] == '\t')
	if utf8.RuneCountInString(line) <= width || isCode {
		return line
	}
	pad := strings.Repeat(
		" ",
		hang,
	)
	rows := []string{marker}
	for _, word := range strings.Fields(line[hang:]) {
		last := len(rows) - 1
		lastWidth := utf8.RuneCountInString(rows[last])
		switch {
		case lastWidth <= hang:
			rows[last] += word
		case lastWidth+1+utf8.RuneCountInString(word) <= width:
			rows[last] += " " + word
		default:
			rows = append(
				rows,
				pad+word,
			)
		}
	}
	return strings.Join(
		rows,
		"\n",
	)
}

// Returns a relative time such as 8h ago.
func (s Style) formatAge(then time.Time) string {
	seconds := int(s.Now.Sub(then).Seconds())
	switch {
	case seconds < 3600:
		return strconv.Itoa(seconds/60) + "m ago"
	case seconds < 86400:
		return strconv.Itoa(seconds/3600) + "h ago"
	}
	return strconv.Itoa(seconds/86400) + "d ago"
}

func (s Style) paint(
	code string,
	text string,
) string {
	if !s.Color {
		return text
	}
	return "\x1b[" + code + "m" + text + "\x1b[0m"
}

func (s Style) paintRed(text string) string {
	return s.paint(
		red,
		text,
	)
}

func (s Style) paintGreen(text string) string {
	return s.paint(
		green,
		text,
	)
}

func (s Style) paintYellow(text string) string {
	return s.paint(
		yellow,
		text,
	)
}

func (s Style) paintCyan(text string) string {
	return s.paint(
		cyan,
		text,
	)
}

func (s Style) paintGray(text string) string {
	return s.paint(
		gray,
		text,
	)
}

func (s Style) paintBold(text string) string {
	return s.paint(
		bold,
		text,
	)
}

// Returns a review decision or a single review's state in plain words and
// color, or no review for any other value.
func (s Style) formatReview(decision string) string {
	found, ok := reviewWords[decision]
	if !ok {
		found = coloredWords{
			words: "no review",
			color: gray,
		}
	}
	return s.paintWords(found)
}

func (s Style) paintWords(w coloredWords) string {
	return s.paint(
		w.color,
		w.words,
	)
}

// Returns path cut from the left to fit width columns, with an ellipsis.
func cutLeft(
	path string,
	width int,
) string {
	runes := []rune(path)
	if len(runes) <= width || width < 1 {
		return path
	}
	return "…" + string(runes[len(runes)-width+1:])
}
