package jsoninput

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"

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
