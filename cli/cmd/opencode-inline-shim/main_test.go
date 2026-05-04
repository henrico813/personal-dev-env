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
	"time"
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

func TestBestErrorMessagePrefersStructuredMessages(t *testing.T) {
	got := bestErrorMessage([]byte(`{"error":{"message":"structured message"},"message":"fallback"}`))
	if got != "structured message" {
		t.Fatalf("bestErrorMessage() = %q", got)
	}
}

func TestValidateStructuredInline(t *testing.T) {
	tests := []struct {
		name    string
		value   *structuredInline
		wantErr string
	}{
		{name: "nil", value: nil, wantErr: "OpenCode did not return structured output"},
		{name: "missing code", value: &structuredInline{Placement: "replace"}, wantErr: "OpenCode returned structured output without code"},
		{name: "bad placement", value: &structuredInline{Code: "x", Placement: "sideways"}, wantErr: "OpenCode returned unsupported placement \"sideways\""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := validateStructuredInline(tt.value)
			if err == nil {
				t.Fatal("expected error")
			}
			if err.Error() != tt.wantErr {
				t.Fatalf("error = %q", err.Error())
			}
		})
	}
}

func TestNormalizeInlineErrorTimeout(t *testing.T) {
	if got := normalizeInlineError(context.DeadlineExceeded); got != "Inline request timed out" {
		t.Fatalf("normalizeInlineError() = %q", got)
	}
}

func TestConfiguredInlineModelTreatsAliasAsNoOverride(t *testing.T) {
	cfg := config{inlineModel: transportModel}

	got, err := configuredInlineModel(cfg)
	if err != nil {
		t.Fatalf("configuredInlineModel() error = %v", err)
	}
	if got != "" {
		t.Fatalf("configuredInlineModel() = %q", got)
	}
}

func TestSelectedInlineModelUsesConfiguredOverrideForAliasRequest(t *testing.T) {
	cfg := config{inlineModel: "openai-codex/gpt-5.4-mini"}

	got, err := selectedInlineModel(chatRequest{Model: transportModel}, cfg)
	if err != nil {
		t.Fatalf("selectedInlineModel() error = %v", err)
	}
	if got != "openai-codex/gpt-5.4-mini" {
		t.Fatalf("selectedInlineModel() = %q", got)
	}
}

func TestSelectedInlineModelFallsBackToOpenCodeDefault(t *testing.T) {
	cfg := config{}

	got, err := selectedInlineModel(chatRequest{Model: transportModel}, cfg)
	if err != nil {
		t.Fatalf("selectedInlineModel() error = %v", err)
	}
	if got != "" {
		t.Fatalf("selectedInlineModel() = %q", got)
	}
}

func TestConfiguredInlineModelRejectsMalformedOverride(t *testing.T) {
	cfg := config{inlineModel: "gpt-5.4-mini"}

	if _, err := configuredInlineModel(cfg); err == nil {
		t.Fatal("expected malformed inline override to fail")
	}
}

func TestOpenCodeModelBuildsProviderModelPair(t *testing.T) {
	override := openCodeModel("openai-codex/gpt-5.4-mini")
	if override["providerID"] != "openai-codex" || override["modelID"] != "gpt-5.4-mini" {
		t.Fatalf("override = %#v", override)
	}
}

func TestHandleChatCompletionsReturnsInlineFailureEnvelope(t *testing.T) {
	backend := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/session":
			w.Header().Set("content-type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			_, _ = w.Write([]byte(`{"error":{"message":"provider config failed"}}`))
		default:
			http.NotFound(w, r)
		}
	}))
	defer backend.Close()

	cfg := config{opencodeBaseURL: backend.URL, timeout: time.Second}
	req := httptest.NewRequest(http.MethodPost, "/v1/chat/completions", strings.NewReader(`{"messages":[{"role":"user","content":"hello"}]}`))
	rec := httptest.NewRecorder()

	handleChatCompletions(cfg, rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}
	var envelope map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &envelope); err != nil {
		t.Fatalf("unmarshal envelope: %v", err)
	}
	choices, ok := envelope["choices"].([]any)
	if !ok || len(choices) != 1 {
		t.Fatalf("choices = %#v", envelope["choices"])
	}
	choice, ok := choices[0].(map[string]any)
	if !ok {
		t.Fatalf("choice = %#v", choices[0])
	}
	message, ok := choice["message"].(map[string]any)
	if !ok {
		t.Fatalf("message = %#v", choice["message"])
	}
	content, ok := message["content"].(string)
	if !ok {
		t.Fatalf("content = %#v", message["content"])
	}
	if content != `{"error":"provider config failed"}` {
		t.Fatalf("content = %q", content)
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

func TestBuildPromptDefaultsToEditOnly(t *testing.T) {
	prompt, system := buildPrompt(nil)

	if prompt != "<message role=\"user\">Return a replace edit.</message>" {
		t.Fatalf("prompt = %q", prompt)
	}
	if len(system) != 0 {
		t.Fatalf("system = %#v", system)
	}
}

func TestBackendServeArgsAcceptsLoopbackTargets(t *testing.T) {
	tests := []struct {
		name string
		url  string
		want []string
	}{
		{
			name: "localhost",
			url:  "http://localhost:4199",
			want: []string{"serve", "--hostname", "localhost", "--port", "4199"},
		},
		{
			name: "loopback ip",
			url:  "http://127.0.0.1:4203",
			want: []string{"serve", "--hostname", "127.0.0.1", "--port", "4203"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := backendServeArgs(config{opencodeBaseURL: tt.url})
			if err != nil {
				t.Fatalf("backendServeArgs: %v", err)
			}
			if !slices.Equal(got, tt.want) {
				t.Fatalf("args = %#v", got)
			}
		})
	}
}

func TestBackendServeArgsRejectsUnsupportedTargets(t *testing.T) {
	tests := []string{
		"http://example.com:4199",
		"http://127.0.0.1",
		"http://127.0.0.1:4199/api",
	}

	for _, target := range tests {
		t.Run(target, func(t *testing.T) {
			if _, err := backendServeArgs(config{opencodeBaseURL: target}); err == nil {
				t.Fatal("expected backendServeArgs to fail")
			}
		})
	}
}
