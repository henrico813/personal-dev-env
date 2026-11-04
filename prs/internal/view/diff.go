package view

import (
	"strconv"
	"strings"

	"prs/internal/diffsplit"
)

// Returns one row per changed file for fzf, with the file's number hidden
// first and the counts and tags in color.
func RenderFileRows(
	files []diffsplit.File,
	s Style,
) string {
	var out strings.Builder
	for _, file := range files {
		counts := s.paintGreen("+"+strconv.Itoa(file.Added)) + " " + s.paintRed("-"+strconv.Itoa(file.Removed))
		if len(file.Tags) > 0 {
			counts += " " + s.paintYellow(strings.Join(
				file.Tags,
				" ",
			))
		}
		out.WriteString(strconv.Itoa(file.Number) + "\t" + counts + "\t" + file.Path + "\n")
	}
	return out.String()
}
