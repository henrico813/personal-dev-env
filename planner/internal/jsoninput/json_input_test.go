package jsoninput

import (
	"encoding/json"
	"errors"
	"reflect"
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
	got := Lint(data, err)
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
	got := Lint(data, err)
	if !strings.Contains(got, "title") {
		t.Fatalf("expected field name in output, got %q", got)
	}
}

func TestLintPassthroughUnknownError(t *testing.T) {
	err := errors.New("some arbitrary error")
	got := Lint(nil, err)
	if got != "some arbitrary error" {
		t.Fatalf("expected passthrough, got %q", got)
	}
}

func TestDecodeStrictRejectsTrailingData(t *testing.T) {
	var s string
	if err := DecodeStrict([]byte(`"valid" trailing`), &s); err == nil {
		t.Fatal("expected error for trailing data after JSON value")
	}
}

func TestMaybeRepairEscapesRawNewlinesInsideStrings(t *testing.T) {
	input := []byte("{\"message\":\"hello\nworld\"}")
	got, repaired, err := MaybeRepair(input)
	if err != nil {
		t.Fatalf("MaybeRepair: %v", err)
	}
	if !repaired {
		t.Fatal("expected repair to occur")
	}
	var decoded map[string]string
	if err := json.Unmarshal(got, &decoded); err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
	if decoded["message"] != "hello\nworld" {
		t.Fatalf("decoded message = %q", decoded["message"])
	}
}

func TestMaybeRepairStripsCommentsAndTrailingCommas(t *testing.T) {
	input := []byte("{\n  // inline comment\n  \"items\": [1, 2,],\n  /* block comment */\n  \"name\": \"planner\",\n}\n")
	got, repaired, err := MaybeRepair(input)
	if err != nil {
		t.Fatalf("MaybeRepair: %v", err)
	}
	if !repaired {
		t.Fatal("expected repair to occur")
	}
	var decoded struct {
		Items []int  `json:"items"`
		Name  string `json:"name"`
	}
	if err := json.Unmarshal(got, &decoded); err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
	if !reflect.DeepEqual(decoded.Items, []int{1, 2}) {
		t.Fatalf("items = %#v", decoded.Items)
	}
	if decoded.Name != "planner" {
		t.Fatalf("name = %q", decoded.Name)
	}
}

func TestMaybeRepairPreservesCommentMarkersInsideStrings(t *testing.T) {
	input := []byte("{\"text\":\"keep // and /* markers */,\",\"items\":[\"/*literal*/\",],}")
	got, repaired, err := MaybeRepair(input)
	if err != nil {
		t.Fatalf("MaybeRepair: %v", err)
	}
	if !repaired {
		t.Fatal("expected repair to occur")
	}
	var decoded struct {
		Text  string   `json:"text"`
		Items []string `json:"items"`
	}
	if err := json.Unmarshal(got, &decoded); err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
	if decoded.Text != "keep // and /* markers */," {
		t.Fatalf("text = %q", decoded.Text)
	}
	if !reflect.DeepEqual(decoded.Items, []string{"/*literal*/"}) {
		t.Fatalf("items = %#v", decoded.Items)
	}
}

func TestMaybeRepairLeavesValidJSONUntouched(t *testing.T) {
	input := []byte(`{"title":"planner","items":[1,2]}`)
	got, repaired, err := MaybeRepair(input)
	if err != nil {
		t.Fatalf("MaybeRepair: %v", err)
	}
	if repaired {
		t.Fatal("expected valid JSON to skip repair")
	}
	if string(got) != string(input) {
		t.Fatalf("output changed: %q", string(got))
	}
}

func TestMaybeRepairLeavesOriginalBytesWhenRepairFails(t *testing.T) {
	input := []byte("{bad}")
	got, repaired, err := MaybeRepair(input)
	if err != nil {
		t.Fatalf("MaybeRepair: %v", err)
	}
	if repaired {
		t.Fatal("expected failed repair to leave input unrepaired")
	}
	if string(got) != string(input) {
		t.Fatalf("expected original bytes back, got %q", string(got))
	}
}
