package internal

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/kaptinlin/jsonrepair"
)

// Read loads JSON from a path or stdin, then applies the shared repair policy.
// Callers decide how to surface repair notices; this helper only reports
// whether repaired bytes were produced.
func Read(path string, useStdin, allowAutoDetect bool, stdin io.Reader, stdinPiped func() bool) ([]byte, bool, error) {
	var data []byte
	var err error
	switch {
	case useStdin || (allowAutoDetect && path == "" && stdinPiped()):
		data, err = io.ReadAll(stdin)
	case path == "":
		return nil, false, fmt.Errorf("no JSON source: pass a path, pipe stdin, or use --stdin")
	default:
		data, err = os.ReadFile(path)
	}
	if err != nil {
		return nil, false, err
	}

	repaired, didRepair, err := MaybeRepair(data)
	if err != nil {
		return nil, false, err
	}
	return repaired, didRepair, nil
}

// MaybeRepair normalizes common JSON repair cases while keeping already-valid
// payloads unchanged.
func MaybeRepair(data []byte) ([]byte, bool, error) {
	if json.Valid(data) {
		return data, false, nil
	}

	repaired, err := jsonrepair.JSONRepair(string(data))
	if err != nil {
		return data, false, nil
	}
	repairedBytes := []byte(repaired)
	if bytes.Equal(repairedBytes, data) {
		return data, false, nil
	}
	if !json.Valid(repairedBytes) {
		return data, false, nil
	}
	return repairedBytes, true, nil
}

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
