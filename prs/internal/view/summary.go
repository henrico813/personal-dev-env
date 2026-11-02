package view

import (
	"strconv"
	"strings"
	"unicode/utf8"

	"prs/internal/github"
)

// Returns the PR summary screen's list of files under its heading. Long
// paths are cut from the left so the file name shows.
func RenderFileSummary(
	pr github.PullRequest,
	extras github.Extras,
	s Style,
) string {
	var out strings.Builder
	out.WriteString("\n" + s.FormatHeading("Files") + "\n")
	for _, file := range extras.Files {
		added := "+" + strconv.Itoa(file.Additions)
		removed := "-" + strconv.Itoa(file.Deletions)
		room := s.Width - utf8.RuneCountInString(added+" "+removed+" ")
		out.WriteString(s.paintGreen(added) + " " + s.paintRed(removed) + " " + cutLeft(
			file.Path,
			room,
		) + "\n")
	}
	// GitHub sends at most 100 files, and the diff screen shows them all.
	if more := pr.ChangedFiles - len(extras.Files); more > 0 {
		out.WriteString(s.paintGray("and "+strconv.Itoa(more)+" more files") + "\n")
	}
	return out.String()
}

// Returns a section heading of the PR summary screen, such as "== Files ==".
func (s Style) FormatHeading(title string) string {
	return s.paint(
		blue,
		"== "+title+" ==",
	)
}
