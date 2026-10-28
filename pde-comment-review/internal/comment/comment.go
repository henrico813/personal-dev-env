// Package comment reads and writes Go line comments. It is the only code in
// pde-comment-review that knows comment syntax.
package comment

import (
	"errors"
	"go/scanner"
	"go/token"
	"strings"
	"unicode/utf8"
)

const lineWidth = 79

// Returns an error if src[start:end] is only some lines of a longer
// comment: a "//" line sits just above or just below it. A Go directive
// such as //go:generate there is fine, as it is not part of the comment.
func RefusePartialComment(src string, start, end int) error {
	above := strings.TrimSuffix(src[:start], "\n")
	above = above[strings.LastIndex(above, "\n")+1:]
	below, _, _ := strings.Cut(src[end:], "\n")
	if isCommentLine(above) && !isDirectiveLine(above) || isCommentLine(below) && !isDirectiveLine(below) {
		return errors.New("is only part of a comment: a // line is just above or below it")
	}
	return nil
}

// Returns an error unless each line of src[start:end] starts a comment when
// Go reads src. A "//" line inside a string, such as a test fixture, or
// inside a /* */ comment is refused.
func RefuseTextOutsideComments(src string, start, end int) error {
	fset := token.NewFileSet()
	file := fset.AddFile("", fset.Base(), len(src))
	var s scanner.Scanner
	s.Init(file, []byte(src), nil, scanner.ScanComments)
	commentStarts := map[int]bool{}
	for {
		pos, tok, _ := s.Scan()
		if tok == token.EOF {
			break
		}
		if tok == token.COMMENT {
			commentStarts[file.Offset(pos)] = true
		}
	}
	for at := start; at < end; {
		line, _, _ := strings.Cut(src[at:end], "\n")
		if !commentStarts[at+len(line)-len(strings.TrimLeft(line, " \t"))] {
			return errors.New("is not a comment when Go reads the file: it is inside a string or a /* */ comment")
		}
		at += len(line) + 1
	}
	return nil
}

func isCommentLine(line string) bool {
	return strings.HasPrefix(strings.TrimLeft(line, " \t"), "//")
}

// Reports whether "//" is followed by something other than a space, a tab
// or the line's end, as in //go:generate.
func isDirectiveLine(line string) bool {
	body, ok := strings.CutPrefix(strings.TrimLeft(line, " \t"), "//")
	return ok && body != "" && body[0] != ' ' && body[0] != '\t'
}

// One block of a comment: sentences, or a code example when Code is true. An
// example's lines are kept as written and never rewrapped.
type paragraph struct {
	Code bool
	Text string
}

func parseParagraphs(text string) []paragraph {
	var bodies []string
	for _, line := range strings.Split(strings.TrimSuffix(text, "\n"), "\n") {
		bodies = append(bodies, strings.TrimPrefix(strings.TrimLeft(line, " \t"), "//"))
	}
	return groupParagraphs(bodies)
}

// Splits lines, with their "//" removed, into blocks. A blank line, or a
// switch between sentences and example lines, starts a new block. A line
// that starts with a tab or two spaces is an example line.
func groupParagraphs(lines []string) []paragraph {
	var out []paragraph
	var words, code []string
	flush := func() {
		if len(words) > 0 {
			out = append(out, paragraph{Text: strings.Join(words, " ")})
		}
		if len(code) > 0 {
			out = append(out, paragraph{Code: true, Text: strings.Join(code, "\n")})
		}
		words, code = nil, nil
	}
	for _, line := range lines {
		switch {
		case strings.TrimSpace(line) == "":
			flush()
		case isCodeLine(line):
			if len(words) > 0 {
				flush()
			}
			code = append(code, strings.TrimRight(line, " \t"))
		default:
			if len(code) > 0 {
				flush()
			}
			words = append(words, strings.Fields(line)...)
		}
	}
	flush()
	return out
}

func isCodeLine(line string) bool {
	return strings.HasPrefix(line, "\t") || strings.HasPrefix(line, "  ")
}

// Turns a Go comment into the text a reviewer edits. The "//" markers are
// removed, each block of sentences becomes one line, and a blank line
// separates blocks. Example lines keep their leading tab or spaces, which
// is how ToCommentLines tells them apart from sentences.
func ToPlainText(text string) string {
	var parts []string
	for _, p := range parseParagraphs(text) {
		parts = append(parts, p.Text)
	}
	return strings.Join(parts, "\n\n") + "\n"
}

// Turns text edited from ToPlainText back into "//" lines, indented like
// old, the comment it replaces. Sentences are rewrapped to lineWidth; lines
// that start with a tab or two spaces are kept unchanged as code examples.
// Text with no words returns "", so writing the result deletes the
// comment.
func ToCommentLines(text, old string) string {
	indent := findIndent(old)
	width := lineWidth - len(strings.ReplaceAll(indent, "\t", "    ")) - len("// ")
	var lines []string
	for _, p := range groupParagraphs(strings.Split(text, "\n")) {
		if len(lines) > 0 {
			lines = append(lines, indent+"//")
		}
		if p.Code {
			for _, line := range strings.Split(p.Text, "\n") {
				lines = append(lines, indent+"//"+line)
			}
			continue
		}
		for _, line := range WrapWords(strings.Fields(p.Text), width) {
			lines = append(lines, indent+"// "+line)
		}
	}
	if len(lines) == 0 {
		return ""
	}
	return strings.Join(lines, "\n") + "\n"
}

// Splits a Go comment into rows for a narrow screen, such as the
// 37-column phone popup. Drops the indentation and rewraps sentences to
// width. Example lines are not wrapped, so the caller must handle any that
// are wider than width.
func WrapForScreen(text string, width int) []string {
	var rows []string
	for i, p := range parseParagraphs(text) {
		if i > 0 {
			rows = append(rows, "//")
		}
		if p.Code {
			for _, line := range strings.Split(p.Text, "\n") {
				rows = append(rows, "//"+line)
			}
			continue
		}
		for _, line := range WrapWords(strings.Fields(p.Text), width-len("// ")) {
			rows = append(rows, "// "+line)
		}
	}
	return rows
}

func findIndent(text string) string {
	line, _, _ := strings.Cut(text, "\n")
	return line[:len(line)-len(strings.TrimLeft(line, " \t"))]
}

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
