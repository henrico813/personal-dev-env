package internal

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
)

// DecodeStrict decodes a single JSON value with unknown-field rejection and
// shared linted diagnostics for syntax and type errors.
func DecodeStrict(raw []byte, target any) error {
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.DisallowUnknownFields()
	if err := dec.Decode(target); err != nil {
		return fmt.Errorf("%s: %w", Lint(raw, err), err)
	}
	if _, err := dec.Token(); err != io.EOF {
		if err == nil {
			return fmt.Errorf("trailing data after JSON value")
		}
		return fmt.Errorf("%s: %w", Lint(raw, err), err)
	}
	return nil
}

// Lint converts raw encoding/json decode errors into human-readable diagnostics
// with line/column and field context where available.
func Lint(data []byte, err error) string {
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
