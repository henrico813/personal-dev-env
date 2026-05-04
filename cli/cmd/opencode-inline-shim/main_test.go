package main

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"slices"
	"strings"
	"testing"
)

func TestDecodeChatRequestAllowsOpenAICompatibleFields(t *testing.T) {
	body := io.NopCloser(strings.NewReader(`{
		"model":"opencode-inline",
		"messages":[{"role":"user","content":"hello"}],
		"stream":false,
		"temperature":0,
		"top_p":1,
		"presence_penalty":0,
		"frequency_penalty":0,
		"max_tokens":512
	}`))

	requestBody, err := decodeChatRequest(body)
	if err != nil {
		t.Fatalf("decode request body: %v", err)
	}
	if requestBody.Model != "opencode-inline" {
		t.Fatalf("model = %q", requestBody.Model)
	}
	if len(requestBody.Messages) != 1 {
		t.Fatalf("messages = %d", len(requestBody.Messages))
	}
}

func TestBackendReachableAcceptsAnyHTTPResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "missing", http.StatusNotFound)
	}))
	defer server.Close()

	cfg := config{opencodeBaseURL: server.URL}
	if err := backendReachable(context.Background(), cfg); err != nil {
		t.Fatalf("backend reachable: %v", err)
	}
}

func TestBackendReachableFailsWhenBackendIsDown(t *testing.T) {
	cfg := config{opencodeBaseURL: "http://127.0.0.1:1"}
	if err := backendReachable(context.Background(), cfg); err == nil {
		t.Fatal("expected backend reachability failure")
	}
}

func TestCleanupSessionSendsDelete(t *testing.T) {
	var method, path string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		method = r.Method
		path = r.URL.Path
		w.WriteHeader(http.StatusNoContent)
	}))
	defer server.Close()

	cfg := config{opencodeBaseURL: server.URL}
	cleanupSession(cfg, "session-123")

	if method != http.MethodDelete {
		t.Fatalf("method = %q", method)
	}
	if path != "/session/session-123" {
		t.Fatalf("path = %q", path)
	}
}

func TestRequestStructuredInlineUsesEditOnlySchema(t *testing.T) {
	var messagePayload map[string]any
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case r.Method == http.MethodPost && r.URL.Path == "/session":
			w.Header().Set("content-type", "application/json")
			_, _ = io.WriteString(w, `{"id":"session-123"}`)
		case r.Method == http.MethodPost && r.URL.Path == "/session/session-123/message":
			defer r.Body.Close()
			if err := json.NewDecoder(r.Body).Decode(&messagePayload); err != nil {
				t.Fatalf("decode message payload: %v", err)
			}
			w.Header().Set("content-type", "application/json")
			_, _ = io.WriteString(w, `{"info":{"structured":{"code":"return a + b","placement":"replace","language":"python"}}}`)
		case r.Method == http.MethodDelete && r.URL.Path == "/session/session-123":
			w.WriteHeader(http.StatusNoContent)
		default:
			t.Fatalf("unexpected request: %s %s", r.Method, r.URL.Path)
		}
	}))
	defer server.Close()

	cfg := config{opencodeBaseURL: server.URL, defaultModel: defaultModel, inlineAgent: defaultAgent}
	requestBody := chatRequest{
		Model: defaultModel,
		Messages: []chatMessage{
			{Role: "user", Content: "Fix the function so it returns a + b."},
		},
	}

	structured, err := requestStructuredInline(context.Background(), cfg, requestBody)
	if err != nil {
		t.Fatalf("request structured inline: %v", err)
	}
	if structured.Placement != "replace" {
		t.Fatalf("placement = %q", structured.Placement)
	}

	format, ok := messagePayload["format"].(map[string]any)
	if !ok {
		t.Fatalf("format payload missing: %#v", messagePayload["format"])
	}
	schema, ok := format["schema"].(map[string]any)
	if !ok {
		t.Fatalf("schema payload missing: %#v", format["schema"])
	}
	required, ok := schema["required"].([]any)
	if !ok {
		t.Fatalf("required payload missing: %#v", schema["required"])
	}
	if len(required) != 2 || required[0] != "code" || required[1] != "placement" {
		t.Fatalf("required = %#v", required)
	}

	properties, ok := schema["properties"].(map[string]any)
	if !ok {
		t.Fatalf("properties payload missing: %#v", schema["properties"])
	}
	placement, ok := properties["placement"].(map[string]any)
	if !ok {
		t.Fatalf("placement payload missing: %#v", properties["placement"])
	}
	enumValues, ok := placement["enum"].([]any)
	if !ok {
		t.Fatalf("placement enum missing: %#v", placement["enum"])
	}
	var placements []string
	for _, value := range enumValues {
		text, ok := value.(string)
		if !ok {
			t.Fatalf("placement enum contains non-string: %#v", value)
		}
		placements = append(placements, text)
	}
	if slices.Contains(placements, "chat") {
		t.Fatalf("placement enum unexpectedly contains chat: %#v", placements)
	}
	if !slices.Equal(placements, inlinePlacements) {
		t.Fatalf("placement enum = %#v", placements)
	}
}

func TestBuildPromptDefaultIsEditOnly(t *testing.T) {
	prompt, system := buildPrompt(nil)

	if len(system) != 0 {
		t.Fatalf("system = %#v", system)
	}
	if strings.Contains(prompt, "chat") {
		t.Fatalf("prompt = %q", prompt)
	}
	if !strings.Contains(prompt, "replace edit") {
		t.Fatalf("prompt = %q", prompt)
	}
}
