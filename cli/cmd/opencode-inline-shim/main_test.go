package main

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
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
