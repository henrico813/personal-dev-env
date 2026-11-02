// Package diffsplit splits a pull request's diff into numbered files, one
// per changed file, such as 1.diff and 2.diff. The diff view and the file
// picker's side pane read them from the diff screen's temporary folder.
package diffsplit

import (
	"bytes"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

// Holds one changed file, numbered from 1 in diff order, which names its
// split file. Added and Removed count the lines inside hunks.
type File struct {
	Number  int
	Added   int
	Removed int
	// Any of new, deleted, renamed, copied, binary and mode, in diff order.
	Tags []string
	// The new path, or for a deleted file the path it had, as git prints it.
	Path string
	// The file's lines from the diff, its "diff --git" header first.
	Text []byte
}

// Splits diff and writes each file's lines to N.diff in the folder dir,
// which must exist. Returns one File per changed file.
func WriteFiles(
	dir string,
	diff []byte,
) (
	[]File,
	error,
) {
	files := split(diff)
	for _, file := range files {
		if err := os.WriteFile(
			filepath.Join(
				dir,
				strconv.Itoa(file.Number)+".diff",
			),
			file.Text,
			0o600,
		); err != nil {
			return nil, err
		}
	}
	return files, nil
}

// Returns one File per "diff --git" header in diff. Together their Text
// holds every line from the first header on, in order.
func split(diff []byte) []File {
	var files []File
	inHunk := false
	for _, line := range bytes.SplitAfter(
		diff,
		[]byte("\n"),
	) {
		text := strings.TrimSuffix(
			string(line),
			"\n",
		)
		if rest, ok := strings.CutPrefix(
			text,
			"diff --git ",
		); ok {
			files = append(
				files,
				File{
					Number: len(files) + 1,
					Path:   findSamePath(rest),
				},
			)
			inHunk = false
		}
		if len(files) == 0 {
			continue
		}
		file := &files[len(files)-1]
		file.Text = append(
			file.Text,
			line...,
		)
		// Lines outside hunks are headers, so "+++ b/edit.go" is not an added
		// line.
		switch {
		case !inHunk:
			file.readHeaderLine(text)
			inHunk = strings.HasPrefix(
				text,
				"@@ ",
			)
		case strings.HasPrefix(
			text,
			"+",
		):
			file.Added++
		case strings.HasPrefix(
			text,
			"-",
		):
			file.Removed++
		}
	}
	return files
}

// Notes what one header line, such as "rename from a.go", says about the
// file. Every header line that matters starts with two fixed words.
func (f *File) readHeaderLine(line string) {
	first, rest, _ := strings.Cut(
		line,
		" ",
	)
	second, path, _ := strings.Cut(
		rest,
		" ",
	)
	switch first + " " + second {
	case "new file":
		f.addTag("new")
	case "deleted file":
		f.addTag("deleted")
	case "old mode":
		f.addTag("mode")
	case "Binary files":
		f.addTag("binary")
	case "rename from":
		f.addTag("renamed")
	case "copy from":
		f.addTag("copied")
	case "rename to", "copy to":
		f.Path = path
	}
}

func (f *File) addTag(tag string) {
	f.Tags = append(
		f.Tags,
		tag,
	)
}

// Returns PATH from "a/PATH b/PATH", keeping git's quotes around a path
// with odd characters. A rename's paths differ, and its "rename to" line
// replaces this one.
func findSamePath(rest string) string {
	// Both paths are the same, so the first half of the line is a/PATH.
	half := rest[:len(rest)/2]
	quote := ""
	if strings.HasPrefix(
		half,
		`"`,
	) {
		quote = `"`
		half = strings.TrimSuffix(
			strings.TrimPrefix(
				half,
				`"`,
			),
			`"`,
		)
	}
	path := strings.TrimPrefix(
		half,
		"a/",
	)
	if path == "" {
		return rest
	}
	return quote + path + quote
}
