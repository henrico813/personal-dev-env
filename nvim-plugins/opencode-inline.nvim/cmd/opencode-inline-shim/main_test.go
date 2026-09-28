package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
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

func TestRunHelpSucceeds(t *testing.T) {
	if err := run([]string{"--help"}); err != nil {
		t.Fatalf("run(--help) error = %v", err)
	}
}

func TestDecodeChatRequestAllowsCompatibleFields(t *testing.T) {
	body := io.NopCloser(strings.NewReader(`{"model":"opencode-inline","messages":[{"role":"user","content":"hello"}],"stream":false,"temperature":0}`))

	requestBody, err := decodeChatRequest(body)
	if err != nil {
		t.Fatalf("decodeChatRequest() error = %v", err)
	}
	if requestBody.Model != transportModel || len(requestBody.Messages) != 1 {
		t.Fatalf("request = %#v", requestBody)
	}
}

func TestSelectedInlineModelSupportsVariants(t *testing.T) {
	got, err := selectedInlineModel(chatRequest{Model: "openrouter/z-ai/glm-5.3-prime#high"})
	if err != nil {
		t.Fatalf("selectedInlineModel() error = %v", err)
	}
	want := &inlineModel{ProviderID: "openrouter", ModelID: "z-ai/glm-5.3-prime", Variant: "high"}
	if *got != *want {
		t.Fatalf("model = %#v, want %#v", got, want)
	}
}

func TestSelectedInlineModelUsesDefaultAlias(t *testing.T) {
	got, err := selectedInlineModel(chatRequest{Model: transportModel})
	if err != nil {
		t.Fatalf("selectedInlineModel() error = %v", err)
	}
	if got != nil {
		t.Fatalf("model = %#v, want nil", got)
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

func TestBackendReachableClassifiesResponses(t *testing.T) {
	for _, test := range []struct {
		name   string
		status int
		want   bool
	}{
		{name: "success", status: http.StatusOK},
		{name: "unauthorized", status: http.StatusUnauthorized, want: true},
		{name: "not opencode", status: http.StatusNotFound, want: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				if r.URL.Path != "/api/info" {
					http.NotFound(w, r)
					return
				}
				w.WriteHeader(test.status)
			}))
			defer server.Close()

			err := backendReachable(context.Background(), config{opencodeBaseURL: server.URL})
			if (err != nil) != test.want {
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
		t.Fatalf("backendReachable() error = %v", err)
	}
}

func TestEnsureReachableReportsRejectedPassword(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusUnauthorized)
	}))
	defer server.Close()

	err := (&backendManager{cfg: config{opencodeBaseURL: server.URL, password: "stale"}}).ensureReachable(context.Background())
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

func TestParseInlineTextAcceptsJSON(t *testing.T) {
	for _, text := range []string{`{"code":"x","placement":"replace"}`, "```json\n{\"code\":\"x\",\"placement\":\"replace\"}\n```"} {
		got, err := parseInlineText(text)
		if err != nil {
			t.Fatalf("parseInlineText() error = %v", err)
		}
		if got.Code != "x" || got.Placement != "replace" {
			t.Fatalf("parseInlineText() = %#v", got)
		}
	}
}

func TestParseInlineTextRejectsProse(t *testing.T) {
	if _, err := parseInlineText("Here is the edit you asked for."); err == nil {
		t.Fatal("expected prose to be rejected")
	}
}

func TestBestErrorMessagePrefersStructuredMessages(t *testing.T) {
	got := bestErrorMessage([]byte(`{"error":{"message":"structured message"},"message":"fallback"}`))
	if got != "structured message" {
		t.Fatalf("bestErrorMessage() = %q", got)
	}
}

func TestNormalizeInlineErrorTimeout(t *testing.T) {
	if got := normalizeInlineError(context.DeadlineExceeded); got != "Inline request timed out" {
		t.Fatalf("normalizeInlineError() = %q", got)
	}
}

func TestValidateStructuredInlineRejectsInvalidValues(t *testing.T) {
	for _, test := range []struct {
		name  string
		value *structuredInline
	}{
		{name: "nil"},
		{name: "missing code", value: &structuredInline{Placement: "replace"}},
		{name: "bad placement", value: &structuredInline{Code: "x", Placement: "sideways"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			if err := validateStructuredInline(test.value); err == nil {
				t.Fatal("expected validation error")
			}
		})
	}
}

func TestBackendServeArgsAcceptsLoopbackTargets(t *testing.T) {
	got, err := backendServeArgs(config{opencodeBaseURL: "http://127.0.0.1:4203"})
	if err != nil {
		t.Fatalf("backendServeArgs() error = %v", err)
	}
	want := []string{"serve", "--hostname", "127.0.0.1", "--port", "4203"}
	if strings.Join(got, " ") != strings.Join(want, " ") {
		t.Fatalf("args = %#v, want %#v", got, want)
	}
}

func TestBackendServeArgsRejectsUnsupportedTargets(t *testing.T) {
	for _, target := range []string{"http://example.com:4199", "http://127.0.0.1", "http://127.0.0.1:4199/api"} {
		t.Run(target, func(t *testing.T) {
			if _, err := backendServeArgs(config{opencodeBaseURL: target}); err == nil {
				t.Fatal("expected backendServeArgs to fail")
			}
		})
	}
}

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
			_, _ = w.Write([]byte(`{"version":"2.0.18"}`))
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
	req := httptest.NewRequest(http.MethodPost, "/v1/chat/completions", strings.NewReader(`{"model":"openrouter/z-ai/glm-5.3-prime#high","messages":[{"role":"system","content":"Use tabs."},{"role":"user","content":"set x"}]}`))
	rec := httptest.NewRecorder()
	handleChatCompletions(&backendManager{cfg: cfg}, rec, req)

	if content := completionContent(t, rec); content != `{"code":"x = 1","placement":"replace"}` {
		t.Fatalf("content = %q", content)
	}
	if session["agent"] != "inline" || !deleted {
		t.Fatalf("session = %#v, deleted = %v", session, deleted)
	}
	model := session["model"].(map[string]any)
	if model["providerID"] != "openrouter" || model["id"] != "z-ai/glm-5.3-prime" || model["variant"] != "high" {
		t.Fatalf("model = %#v", model)
	}
	if !strings.Contains(generate["prompt"].(string), "Use tabs.") {
		t.Fatalf("prompt = %q", generate["prompt"])
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
