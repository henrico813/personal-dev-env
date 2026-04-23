package main

import (
	"encoding/json"
	"errors"
	"strings"
	"testing"
)

func TestLintAnnotatesSyntaxErrorWithLineCol(t *testing.T) {
	data := []byte("{bad}")
	var v any
	err := json.Unmarshal(data, &v)
	if err == nil {
		t.Fatal("expected parse error")
	}
	got := lintJSON(data, err)
	if !strings.HasPrefix(got, "line ") {
		t.Fatalf("expected line/col prefix, got %q", got)
	}
}

func TestLintAnnotatesUnmarshalTypeError(t *testing.T) {
	data := []byte(`{"title": 123}`)
	type T struct {
		Title string `json:"title"`
	}
	var v T
	err := json.Unmarshal(data, &v)
	if err == nil {
		t.Fatal("expected unmarshal type error")
	}
	got := lintJSON(data, err)
	if !strings.Contains(got, "title") {
		t.Fatalf("expected field name in output, got %q", got)
	}
}

func TestLintPassthroughUnknownError(t *testing.T) {
	err := errors.New("some arbitrary error")
	got := lintJSON(nil, err)
	if got != "some arbitrary error" {
		t.Fatalf("expected passthrough, got %q", got)
	}
}
