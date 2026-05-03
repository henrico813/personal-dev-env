package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"
)

const (
	defaultPort            = "4141"
	defaultOpenCodeBaseURL = "http://127.0.0.1:4199"
	defaultModel           = "opencode-inline"
	defaultAgent           = "inline"
	defaultTimeout         = 60 * time.Second
)

type config struct {
	port            string
	opencodeBaseURL string
	defaultModel    string
	inlineAgent     string
	timeout         time.Duration
}

type chatRequest struct {
	Model    string        `json:"model"`
	Messages []chatMessage `json:"messages"`
}

type chatMessage struct {
	Role    string `json:"role"`
	Content any    `json:"content"`
}

type structuredInline struct {
	Code      string `json:"code,omitempty"`
	Language  string `json:"language,omitempty"`
	Placement string `json:"placement"`
}

type sessionResponse struct {
	ID string `json:"id"`
}

type sessionMessageResponse struct {
	Info struct {
		Error *struct {
			Data struct {
				Message string `json:"message"`
			} `json:"data"`
		} `json:"error"`
		Structured *structuredInline `json:"structured"`
	} `json:"info"`
}

func main() {
	var healthcheck bool
	var showHelp bool
	flag.BoolVar(&healthcheck, "healthcheck", false, "Exit 0 when the local shim is reachable")
	flag.BoolVar(&showHelp, "help", false, "Show usage")
	flag.Parse()

	cfg := loadConfig()
	if showHelp {
		fmt.Fprintf(os.Stdout, "opencode-inline-shim serves /healthz, /v1/models, and /v1/chat/completions on 127.0.0.1:%s\n", cfg.port)
		return
	}
	if healthcheck {
		if err := runHealthcheck(cfg); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		return
	}
	if err := runServer(cfg); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func loadConfig() config {
	return config{
		port:            getenv("OPENCODE_INLINE_SHIM_PORT", defaultPort),
		opencodeBaseURL: strings.TrimRight(getenv("OPENCODE_BASE_URL", defaultOpenCodeBaseURL), "/"),
		defaultModel:    getenv("OPENCODE_INLINE_MODEL", defaultModel),
		inlineAgent:     getenv("OPENCODE_INLINE_AGENT", defaultAgent),
		timeout:         defaultTimeout,
	}
}

func getenv(name, fallback string) string {
	if value := strings.TrimSpace(os.Getenv(name)); value != "" {
		return value
	}
	return fallback
}

func runHealthcheck(cfg config) error {
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, "http://127.0.0.1:"+cfg.port+"/healthz", nil)
	if err != nil {
		return err
	}
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return fmt.Errorf("healthcheck returned %s", response.Status)
	}
	return nil
}

func backendReachable(ctx context.Context, cfg config) error {
	backendURL, err := url.Parse(cfg.opencodeBaseURL)
	if err != nil {
		return fmt.Errorf("parse OpenCode base URL: %w", err)
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, backendURL.String(), nil)
	if err != nil {
		return err
	}
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	_, _ = io.Copy(io.Discard, response.Body)
	return nil
}

func runServer(cfg config) error {
	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		ctx, cancel := context.WithTimeout(r.Context(), 2*time.Second)
		defer cancel()
		if err := backendReachable(ctx, cfg); err != nil {
			writeJSON(w, http.StatusServiceUnavailable, map[string]any{"ok": false, "backend": err.Error()})
			return
		}
		writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
	})
	mux.HandleFunc("/v1/models", func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodGet {
			writeError(w, http.StatusMethodNotAllowed, "method_not_allowed", "use GET /v1/models")
			return
		}
		writeJSON(w, http.StatusOK, map[string]any{
			"object": "list",
			"data":   []map[string]string{{"id": cfg.defaultModel, "object": "model"}},
		})
	})
	mux.HandleFunc("/v1/chat/completions", func(w http.ResponseWriter, r *http.Request) {
		handleChatCompletions(cfg, w, r)
	})

	server := &http.Server{
		Addr:              "127.0.0.1:" + cfg.port,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()
	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()

	fmt.Fprintf(os.Stderr, "opencode inline shim listening on http://127.0.0.1:%s\n", cfg.port)
	err := server.ListenAndServe()
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}

func handleChatCompletions(cfg config, w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, "method_not_allowed", "use POST /v1/chat/completions")
		return
	}

	requestBody, err := decodeChatRequest(r.Body)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid_request_error", err.Error())
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), cfg.timeout)
	defer cancel()
	structured, err := requestStructuredInline(ctx, cfg, requestBody)
	if err != nil {
		status := http.StatusBadGateway
		if errors.Is(err, context.DeadlineExceeded) {
			status = http.StatusGatewayTimeout
		}
		writeError(w, status, "opencode_inline_error", err.Error())
		return
	}

	model := requestBody.Model
	if strings.TrimSpace(model) == "" {
		model = cfg.defaultModel
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"id":      fmt.Sprintf("opencode-inline-%d", time.Now().UnixMilli()),
		"object":  "chat.completion",
		"created": time.Now().Unix(),
		"model":   model,
		"choices": []map[string]any{{
			"index": 0,
			"message": map[string]any{
				"role":    "assistant",
				"content": mustJSON(structured),
			},
			"finish_reason": "stop",
		}},
	})
}

func decodeChatRequest(body io.ReadCloser) (chatRequest, error) {
	defer body.Close()
	var requestBody chatRequest
	decoder := json.NewDecoder(body)
	if err := decoder.Decode(&requestBody); err != nil {
		return chatRequest{}, fmt.Errorf("decode request body: %w", err)
	}
	if len(requestBody.Messages) == 0 {
		return chatRequest{}, errors.New("messages must contain at least one entry")
	}
	return requestBody, nil
}

func requestStructuredInline(ctx context.Context, cfg config, requestBody chatRequest) (*structuredInline, error) {
	prompt, system := buildPrompt(requestBody.Messages)
	sessionPayload := map[string]any{
		"title": "CodeCompanion Inline",
		"permission": []map[string]string{
			{"permission": "*", "pattern": "*", "action": "deny"},
			{"permission": "StructuredOutput", "pattern": "*", "action": "allow"},
		},
	}
	var session sessionResponse
	if err := postJSON(ctx, cfg, "/session", sessionPayload, &session); err != nil {
		return nil, err
	}

	payload := map[string]any{
		"agent": cfg.inlineAgent,
		"system": strings.Join(append([]string{
			"You are an inline editing backend for CodeCompanion.",
			"Use StructuredOutput exactly once and do not call other tools.",
			"Return code suitable for direct insertion into the current buffer.",
		}, system...), "\n"),
		"format": map[string]any{
			"type":       "json_schema",
			"retryCount": 0,
			"schema": map[string]any{
				"type":     "object",
				"required": []string{"placement"},
				"properties": map[string]any{
					"code":     map[string]string{"type": "string"},
					"language": map[string]string{"type": "string"},
					"placement": map[string]any{
						"type": "string",
						"enum": []string{"replace", "add", "before", "new", "chat"},
					},
				},
				"additionalProperties": false,
			},
		},
		"parts": []map[string]string{{"type": "text", "text": prompt}},
	}
	if model := strings.TrimSpace(requestBody.Model); model != "" && model != cfg.defaultModel {
		parts := strings.SplitN(model, "/", 2)
		if len(parts) == 2 {
			payload["model"] = map[string]string{"providerID": parts[0], "modelID": parts[1]}
		}
	}

	var response sessionMessageResponse
	if err := postJSON(ctx, cfg, "/session/"+session.ID+"/message", payload, &response); err != nil {
		return nil, err
	}
	if response.Info.Error != nil {
		return nil, errors.New(response.Info.Error.Data.Message)
	}
	if response.Info.Structured == nil {
		return nil, errors.New("OpenCode did not return structured output")
	}
	return response.Info.Structured, nil
}

func buildPrompt(messages []chatMessage) (string, []string) {
	var system []string
	var prompt []string
	for _, message := range messages {
		text := strings.TrimSpace(contentToText(message.Content))
		if text == "" {
			continue
		}
		if message.Role == "system" {
			system = append(system, text)
			continue
		}
		prompt = append(prompt, fmt.Sprintf("<message role=\"%s\">\n%s\n</message>", message.Role, text))
	}
	if len(prompt) == 0 {
		prompt = append(prompt, "<message role=\"user\">Return placement chat.</message>")
	}
	return strings.Join(prompt, "\n\n"), system
}

func contentToText(content any) string {
	switch value := content.(type) {
	case string:
		return value
	case []any:
		var parts []string
		for _, raw := range value {
			part, ok := raw.(map[string]any)
			if !ok || part["type"] != "text" {
				continue
			}
			text, _ := part["text"].(string)
			if strings.TrimSpace(text) != "" {
				parts = append(parts, text)
			}
		}
		return strings.Join(parts, "\n")
	default:
		return ""
	}
}

func postJSON(ctx context.Context, cfg config, path string, payload any, out any) error {
	body, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, cfg.opencodeBaseURL+path, bytes.NewReader(body))
	if err != nil {
		return err
	}
	request.Header.Set("content-type", "application/json")
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	data, err := io.ReadAll(response.Body)
	if err != nil {
		return err
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return fmt.Errorf("OpenCode %s: %s", response.Status, strings.TrimSpace(string(data)))
	}
	if err := json.Unmarshal(data, out); err != nil {
		return fmt.Errorf("decode OpenCode response: %w", err)
	}
	return nil
}

func mustJSON(value any) string {
	data, err := json.Marshal(value)
	if err != nil {
		return `{"placement":"chat"}`
	}
	return string(data)
}

func writeJSON(w http.ResponseWriter, status int, body any) {
	w.Header().Set("content-type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(body)
}

func writeError(w http.ResponseWriter, status int, kind, message string) {
	writeJSON(w, status, map[string]any{
		"error": map[string]string{
			"message": message,
			"type":    kind,
		},
	})
}
