package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"strings"
)

// lintJSON converts a raw encoding/json decode error into a human-readable
// diagnostic. SyntaxError gets line/col context; UnmarshalTypeError gets field
// info. Other errors pass through unchanged.
func lintJSON(data []byte, err error) string {
	var synErr *json.SyntaxError
	var typeErr *json.UnmarshalTypeError
	switch {
	case errors.As(err, &synErr):
		line, col := offsetToLineCol(data, synErr.Offset)
		snippet := contextSnippet(data, synErr.Offset)
		return fmt.Sprintf("line %d col %d: %s\n  %s", line, col, synErr.Error(), snippet)
	case errors.As(err, &typeErr):
		return fmt.Sprintf("field %s: expected %s, got %s", typeErr.Field, typeErr.Type, typeErr.Value)
	default:
		return err.Error()
	}
}

func offsetToLineCol(data []byte, offset int64) (line, col int) {
	if offset > int64(len(data)) {
		offset = int64(len(data))
	}
	line = 1
	lastNL := -1
	for i, b := range data[:offset] {
		if b == '\n' {
			line++
			lastNL = i
		}
	}
	return line, int(offset) - lastNL
}

func contextSnippet(data []byte, offset int64) string {
	start := int(offset) - 20
	if start < 0 {
		start = 0
	}
	end := int(offset) + 20
	if end > len(data) {
		end = len(data)
	}
	return strings.TrimSpace(string(data[start:end]))
}
