package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"slices"
	"strings"
	"testing"
	"time"
)

func TestLoadConfigReadsFlagsAndCredentials(t *testing.T) {
	t.Setenv("OPENCODE_SERVER_USERNAME", "")
	t.Setenv("OPENCODE_SERVER_PASSWORD", "secret")

	cfg, healthcheck, err := loadConfig([]string{"--port", "5151", "--opencode-url", "http://127.0.0.1:5000/", "--agent", "edit", "--healthcheck"})

	if err != nil {
		t.Fatalf("loadConfig() error = %v", err)
	}
	if !healthcheck {
		t.Fatal("healthcheck = false")
	}
	want := config{port: "5151", opencodeBaseURL: "http://127.0.0.1:5000", inlineAgent: "edit", username: "opencode", password: "secret", timeout: defaultTimeout}
	if cfg != want {
		t.Fatalf("config = %#v, want %#v", cfg, want)
	}
}

// The installer verifies the binary with --help, so it must exit successfully.
func TestRunHelpSucceeds(t *testing.T) {
	if err := run([]string{"--help"}); err != nil {
		t.Fatalf("run(--help) error = %v", err)
	}
}

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

func TestSelectedInlineModel(t *testing.T) {
	tests := []struct {
		name  string
		model string
		want  *inlineModel
	}{
		{name: "opencode default", model: transportModel},
		{
			name:  "slash in model id",
			model: "openrouter/z-ai/glm-5.3-prime",
			want:  &inlineModel{ProviderID: "openrouter", ModelID: "z-ai/glm-5.3-prime"},
		},
		{
			name:  "variant suffix",
			model: "openai/gpt-5.4-mini#high",
			want:  &inlineModel{ProviderID: "openai", ModelID: "gpt-5.4-mini", Variant: "high"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := selectedInlineModel(chatRequest{Model: tt.model})
			if err != nil {
				t.Fatalf("selectedInlineModel() error = %v", err)
			}
			if (got == nil) != (tt.want == nil) || got != nil && *got != *tt.want {
				t.Fatalf("selectedInlineModel() = %#v, want %#v", got, tt.want)
			}
		})
	}
}

func TestParseInlineModelRejectsMalformedVariants(t *testing.T) {
	for _, model := range []string{"openai", "openai/", "/model", "openai/model#"} {
		t.Run(model, func(t *testing.T) {
			if _, err := parseInlineModel(model); err == nil {
				t.Fatal("expected parseInlineModel to fail")
			}
		})
	}
}

func TestOpenCodeModelOmitsEmptyVariant(t *testing.T) {
	got := openCodeModel(&inlineModel{ProviderID: "openai", ModelID: "gpt-5.4-mini"})

	if len(got) != 2 || got["providerID"] != "openai" || got["id"] != "gpt-5.4-mini" {
		t.Fatalf("model = %#v", got)
	}
}

func TestBackendReachableClassifiesServerResponses(t *testing.T) {
	tests := []struct {
		name     string
		status   int
		wantErr  bool
		wantDown bool
	}{
		{name: "opencode 2 info", status: http.StatusOK},
		{name: "rejected password", status: http.StatusUnauthorized, wantErr: true},
		{name: "not opencode 2", status: http.StatusNotFound, wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path != "/api/info" {
					http.NotFound(w, r)
					return
				}
				w.WriteHeader(tt.status)
			}))
			defer server.Close()

			err := backendReachable(context.Background(), config{opencodeBaseURL: server.URL})

			if (err != nil) != tt.wantErr {
				t.Fatalf("backendReachable() error = %v", err)
			}
			if errors.Is(err, errBackendDown) {
				t.Fatalf("answering server reported as down: %v", err)
			}
		})
	}
}

func TestBackendReachableReportsDownServer(t *testing.T) {
	err := backendReachable(context.Background(), config{opencodeBaseURL: "http://127.0.0.1:1"})

	if !errors.Is(err, errBackendDown) {
		t.Fatalf("backendReachable() error = %v, want errBackendDown", err)
	}
}

// A live server with another password must be reported, not replaced by auto-start.
func TestEnsureReachableReportsRejectedPassword(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusUnauthorized)
	}))
	defer server.Close()
	backend := &backendManager{cfg: config{opencodeBaseURL: server.URL, username: "opencode", password: "stale"}}

	err := backend.ensureReachable(context.Background())

	if err == nil || !strings.Contains(err.Error(), "rejected the server password") {
		t.Fatalf("ensureReachable() error = %v", err)
	}
}

func TestStartBackendRequiresServerPassword(t *testing.T) {
	err := startBackendProcess(config{opencodeBaseURL: "http://127.0.0.1:4199"})

	if err == nil || !strings.Contains(err.Error(), "OPENCODE_SERVER_PASSWORD") {
		t.Fatalf("startBackendProcess() error = %v", err)
	}
}

func TestBestErrorMessagePrefersStructuredMessages(t *testing.T) {
	got := bestErrorMessage([]byte(`{"error":{"message":"structured message"},"message":"fallback"}`))
	if got != "structured message" {
		t.Fatalf("bestErrorMessage() = %q", got)
	}
}

func TestParseInlineTextAcceptsBareAndFencedJSON(t *testing.T) {
	for name, text := range map[string]string{
		"bare":   `{"code":"x","placement":"replace"}`,
		"fenced": "```json\n{\"code\":\"x\",\"placement\":\"replace\"}\n```",
	} {
		t.Run(name, func(t *testing.T) {
			got, err := parseInlineText(text)
			if err != nil {
				t.Fatalf("parseInlineText() error = %v", err)
			}
			if got.Code != "x" || got.Placement != "replace" {
				t.Fatalf("parseInlineText() = %#v", got)
			}
		})
	}
}

func TestParseInlineTextRejectsProse(t *testing.T) {
	if _, err := parseInlineText("Here is the edit you asked for."); err == nil {
		t.Fatal("expected prose to be rejected")
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

func TestHandleChatCompletionsReturnsInlineFailureEnvelope(t *testing.T) {
	backend := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/api/info":
			w.WriteHeader(http.StatusOK)
		case "/api/session":
			w.Header().Set("content-type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			_, _ = w.Write([]byte(`{"_tag":"InvalidRequestError","message":"provider config failed"}`))
		default:
			http.NotFound(w, r)
		}
	}))
	defer backend.Close()

	cfg := config{opencodeBaseURL: backend.URL, timeout: time.Second}
	req := httptest.NewRequest(http.MethodPost, "/v1/chat/completions", strings.NewReader(`{"messages":[{"role":"user","content":"hello"}]}`))
	rec := httptest.NewRecorder()

	handleChatCompletions(&backendManager{cfg: cfg}, rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d", rec.Code)
	}
	if content := completionContent(t, rec); content != `{"error":"provider config failed"}` {
		t.Fatalf("content = %q", content)
	}
}

// Protects the full OpenCode 2 exchange: session options, generate prompt,
// authenticated cleanup, and the edit JSON returned to CodeCompanion.
func TestChatCompletionUsesOpenCodeV2Session(t *testing.T) {
	var session, generate map[string]any
	var deleted bool
	backend := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if user, password, ok := r.BasicAuth(); !ok || user != "opencode" || password != "secret" {
			w.WriteHeader(http.StatusUnauthorized)
			return
		}
		w.Header().Set("content-type", "application/json")
		switch {
		case r.Method == http.MethodGet && r.URL.Path == "/api/info":
			_, _ = w.Write([]byte(`{"version":"2.0.26"}`))
		case r.Method == http.MethodPost && r.URL.Path == "/api/session":
			_ = json.NewDecoder(r.Body).Decode(&session)
			_, _ = w.Write([]byte(`{"data":{"id":"ses_123"}}`))
		case r.Method == http.MethodPost && r.URL.Path == "/api/session/ses_123/generate":
			_ = json.NewDecoder(r.Body).Decode(&generate)
			_, _ = w.Write([]byte(`{"data":{"text":"` + "```json\\n" + `{\"code\":\"x = 1\",\"placement\":\"replace\"}` + "\\n```" + `"}}`))
		case r.Method == http.MethodDelete && r.URL.Path == "/api/session/ses_123":
			deleted = true
			w.WriteHeader(http.StatusNoContent)
		default:
			http.NotFound(w, r)
		}
	}))
	defer backend.Close()
	cfg := config{opencodeBaseURL: backend.URL, inlineAgent: "inline", username: "opencode", password: "secret", timeout: time.Second}
	req := httptest.NewRequest(http.MethodPost, "/v1/chat/completions", strings.NewReader(`{
		"model":"openrouter/z-ai/glm-5.3-prime#high",
		"messages":[{"role":"system","content":"Use tabs."},{"role":"user","content":"set x"}]
	}`))
	rec := httptest.NewRecorder()

	handleChatCompletions(&backendManager{cfg: cfg}, rec, req)

	if content := completionContent(t, rec); content != `{"code":"x = 1","placement":"replace"}` {
		t.Fatalf("content = %q", content)
	}
	if session["agent"] != "inline" {
		t.Fatalf("session agent = %#v", session["agent"])
	}
	model, _ := session["model"].(map[string]any)
	if model["providerID"] != "openrouter" || model["id"] != "z-ai/glm-5.3-prime" || model["variant"] != "high" {
		t.Fatalf("session model = %#v", session["model"])
	}
	permissions, _ := session["permissions"].([]any)
	if len(permissions) != 1 {
		t.Fatalf("session permissions = %#v", session["permissions"])
	}
	prompt, _ := generate["prompt"].(string)
	if !strings.Contains(prompt, "Use tabs.") || !strings.Contains(prompt, "<message role=\"user\">\nset x\n</message>") {
		t.Fatalf("prompt = %q", prompt)
	}
	if !deleted {
		t.Fatal("session was not deleted")
	}
}

func TestBuildPromptDefaultsToEditOnly(t *testing.T) {
	prompt := buildPrompt(nil)

	if !strings.HasSuffix(prompt, "<message role=\"user\">Return a replace edit.</message>") {
		t.Fatalf("prompt = %q", prompt)
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

func completionContent(t *testing.T, rec *httptest.ResponseRecorder) string {
	t.Helper()
	var envelope struct {
		Choices []struct {
			Message struct {
				Content string `json:"content"`
			} `json:"message"`
		} `json:"choices"`
	}
	if err := json.Unmarshal(rec.Body.Bytes(), &envelope); err != nil {
		t.Fatalf("unmarshal envelope: %v", err)
	}
	if len(envelope.Choices) != 1 {
		t.Fatalf("choices = %d", len(envelope.Choices))
	}
	return envelope.Choices[0].Message.Content
}
