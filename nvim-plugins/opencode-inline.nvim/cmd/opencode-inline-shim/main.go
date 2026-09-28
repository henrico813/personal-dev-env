package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"os/signal"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"
)

const (
	defaultPort         = "4141"
	defaultOpenCodeURL  = "http://127.0.0.1:4199"
	defaultOpenCodeUser = "opencode"
	transportModel      = "opencode-inline"
	defaultAgent        = "inline"
	defaultTimeout      = 60 * time.Second
	backendPollInterval = 150 * time.Millisecond
)

// errBackendDown means no server answered, so auto-starting OpenCode may help.
var errBackendDown = errors.New("OpenCode is not reachable")

type config struct {
	port            string
	opencodeBaseURL string
	inlineAgent     string
	username        string
	password        string
	timeout         time.Duration
}

type backendManager struct {
	cfg     config
	startMu sync.Mutex
}

func inlinePlacements() []string {
	return []string{"replace", "add", "before", "new"}
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

type inlineModel struct {
	ProviderID string
	ModelID    string
	Variant    string
}

type sessionResponse struct {
	Data struct {
		ID string `json:"id"`
	} `json:"data"`
}

type generateResponse struct {
	Data struct {
		Text string `json:"text"`
	} `json:"data"`
}

func main() {
	if err := run(os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(args []string) error {
	cfg, healthcheck, err := loadConfig(args)
	if errors.Is(err, flag.ErrHelp) {
		return nil
	}
	if err != nil {
		return err
	}
	if healthcheck {
		return runHealthcheck(cfg)
	}
	return runServer(cfg)
}

// loadConfig reads flags plus the OPENCODE_SERVER_* credentials that
// `opencode serve` itself uses, so an auto-started server shares them.
func loadConfig(args []string) (config, bool, error) {
	cfg := config{timeout: defaultTimeout}
	var healthcheck bool
	flags := flag.NewFlagSet("opencode-inline-shim", flag.ContinueOnError)
	flags.StringVar(&cfg.port, "port", defaultPort, "loopback port for the shim")
	flags.StringVar(&cfg.opencodeBaseURL, "opencode-url", defaultOpenCodeURL, "OpenCode 2 server URL")
	flags.StringVar(&cfg.inlineAgent, "agent", defaultAgent, "OpenCode agent for inline sessions")
	flags.BoolVar(&healthcheck, "healthcheck", false, "exit 0 when the shim and OpenCode are reachable")
	flags.Usage = func() {
		fmt.Fprintln(flags.Output(), "opencode-inline-shim serves /healthz, /v1/models, and /v1/chat/completions on 127.0.0.1:<port>")
		flags.PrintDefaults()
	}
	if err := flags.Parse(args); err != nil {
		return config{}, false, err
	}
	cfg.opencodeBaseURL = strings.TrimRight(strings.TrimSpace(cfg.opencodeBaseURL), "/")
	cfg.username = getenv("OPENCODE_SERVER_USERNAME", defaultOpenCodeUser)
	cfg.password = os.Getenv("OPENCODE_SERVER_PASSWORD")
	return cfg, healthcheck, nil
}

func getenv(name, fallback string) string {
	if value := strings.TrimSpace(os.Getenv(name)); value != "" {
		return value
	}
	return fallback
}

func (c config) authorize(request *http.Request) {
	if c.password != "" {
		request.SetBasicAuth(c.username, c.password)
	}
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

// backendReachable wraps errBackendDown only when nothing answered. A server
// that answers but rejects the request is not replaced by auto-start.
func backendReachable(ctx context.Context, cfg config) error {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, cfg.opencodeBaseURL+"/api/info", nil)
	if err != nil {
		return err
	}
	cfg.authorize(request)
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		return fmt.Errorf("%w: %w", errBackendDown, err)
	}
	defer response.Body.Close()
	_, _ = io.Copy(io.Discard, response.Body)
	if response.StatusCode == http.StatusUnauthorized {
		return errors.New("OpenCode rejected the server password; set OPENCODE_SERVER_PASSWORD to match the running server")
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return fmt.Errorf("OpenCode /api/info returned %s; OpenCode 2 is required", response.Status)
	}
	return nil
}

func backendServeArgs(cfg config) ([]string, error) {
	backendURL, err := url.Parse(cfg.opencodeBaseURL)
	if err != nil {
		return nil, fmt.Errorf("parse OpenCode base URL: %w", err)
	}
	if backendURL.Path != "" && backendURL.Path != "/" {
		return nil, fmt.Errorf("cannot auto-start OpenCode for base URL with path %q", backendURL.Path)
	}
	host := backendURL.Hostname()
	if host == "" {
		return nil, errors.New("cannot auto-start OpenCode without a hostname")
	}
	if host != "localhost" {
		ip := net.ParseIP(host)
		if ip == nil || !ip.IsLoopback() {
			return nil, fmt.Errorf("cannot auto-start non-loopback OpenCode host %q", host)
		}
	}
	port := backendURL.Port()
	if port == "" {
		return nil, errors.New("cannot auto-start OpenCode without an explicit port")
	}
	if _, err := strconv.Atoi(port); err != nil {
		return nil, fmt.Errorf("parse OpenCode port %q: %w", port, err)
	}
	return []string{"serve", "--hostname", host, "--port", port}, nil
}

func startBackendProcess(cfg config) error {
	args, err := backendServeArgs(cfg)
	if err != nil {
		return err
	}
	// OpenCode 2 generates a random password when none is set, which the shim could not send.
	if cfg.password == "" {
		return errors.New("set OPENCODE_SERVER_PASSWORD so the shim can authenticate to the OpenCode server it starts")
	}
	bin, err := exec.LookPath("opencode")
	if err != nil {
		return fmt.Errorf("find opencode: %w", err)
	}
	cmd := exec.Command(bin, args...)
	cmd.Env = os.Environ()
	cmd.Stdout = io.Discard
	cmd.Stderr = io.Discard
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("start opencode serve: %w", err)
	}
	return nil
}

func (m *backendManager) ensureReachable(ctx context.Context) error {
	if err := backendReachable(ctx, m.cfg); !errors.Is(err, errBackendDown) {
		return err
	}

	m.startMu.Lock()
	defer m.startMu.Unlock()

	if err := backendReachable(ctx, m.cfg); !errors.Is(err, errBackendDown) {
		return err
	}
	if err := startBackendProcess(m.cfg); err != nil {
		return err
	}

	for {
		if err := backendReachable(ctx, m.cfg); !errors.Is(err, errBackendDown) {
			return err
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(backendPollInterval):
		}
	}
}

func runServer(cfg config) error {
	backend := &backendManager{cfg: cfg}
	mux := http.NewServeMux()
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		ctx, cancel := context.WithTimeout(r.Context(), 2*time.Second)
		defer cancel()
		if err := backend.ensureReachable(ctx); err != nil {
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
			"data":   []map[string]string{{"id": transportModel, "object": "model"}},
		})
	})
	mux.HandleFunc("/v1/chat/completions", func(w http.ResponseWriter, r *http.Request) {
		handleChatCompletions(backend, w, r)
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

func handleChatCompletions(backend *backendManager, w http.ResponseWriter, r *http.Request) {
	cfg := backend.cfg
	if r.Method != http.MethodPost {
		writeError(w, http.StatusMethodNotAllowed, "method_not_allowed", "use POST /v1/chat/completions")
		return
	}

	requestBody, err := decodeChatRequest(r.Body)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid_request_error", err.Error())
		return
	}

	selectedModel, err := selectedInlineModel(requestBody)
	if err != nil {
		writeInlineErrorCompletion(w, responseModel(requestBody), normalizeInlineError(err))
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), cfg.timeout)
	defer cancel()
	if err := backend.ensureReachable(ctx); err != nil {
		writeInlineErrorCompletion(w, responseModel(requestBody), normalizeInlineError(err))
		return
	}
	structured, err := requestInline(ctx, cfg, requestBody, selectedModel)
	if err != nil {
		writeInlineErrorCompletion(w, responseModel(requestBody), normalizeInlineError(err))
		return
	}
	if err := validateStructuredInline(structured); err != nil {
		writeInlineErrorCompletion(w, responseModel(requestBody), normalizeInlineError(err))
		return
	}

	writeInlineCompletion(w, responseModel(requestBody), mustJSON(structured))
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

// requestInline uses OpenCode 2.0.18's one-shot session generate route. It
// accepts only a prompt and returns text, so JSON is requested in the prompt
// and validated here instead of by a server-side schema.
func requestInline(ctx context.Context, cfg config, requestBody chatRequest, selectedModel *inlineModel) (*structuredInline, error) {
	sessionPayload := map[string]any{
		"title":       "CodeCompanion Inline",
		"permissions": []map[string]string{{"action": "*", "resource": "*", "effect": "deny"}},
	}
	// OpenCode rejects unknown agents; an empty --agent uses its default agent.
	if cfg.inlineAgent != "" {
		sessionPayload["agent"] = cfg.inlineAgent
	}
	if selectedModel != nil {
		sessionPayload["model"] = openCodeModel(selectedModel)
	}
	var session sessionResponse
	if err := postJSON(ctx, cfg, "/api/session", sessionPayload, &session); err != nil {
		return nil, err
	}
	if strings.TrimSpace(session.Data.ID) == "" {
		return nil, errors.New("OpenCode did not return a session ID")
	}
	defer cleanupSession(cfg, session.Data.ID)

	var generated generateResponse
	path := "/api/session/" + url.PathEscape(session.Data.ID) + "/generate"
	if err := postJSON(ctx, cfg, path, map[string]string{"prompt": buildPrompt(requestBody.Messages)}, &generated); err != nil {
		return nil, err
	}
	return parseInlineText(generated.Data.Text)
}

func buildPrompt(messages []chatMessage) string {
	instructions := []string{
		"You are an inline editing backend for CodeCompanion.",
		"This endpoint only supports edit responses, never chat or explanation mode.",
		fmt.Sprintf(`Respond with only one JSON object: {"code": "...", "language": "...", "placement": "%s"}.`, strings.Join(inlinePlacements(), "|")),
		"Return code suitable for direct insertion into the current buffer.",
	}
	var conversation []string
	for _, message := range messages {
		text := strings.TrimSpace(contentToText(message.Content))
		if text == "" {
			continue
		}
		if message.Role == "system" {
			instructions = append(instructions, text)
			continue
		}
		conversation = append(conversation, fmt.Sprintf("<message role=\"%s\">\n%s\n</message>", message.Role, text))
	}
	if len(conversation) == 0 {
		conversation = append(conversation, "<message role=\"user\">Return a replace edit.</message>")
	}
	return "<instructions>\n" + strings.Join(instructions, "\n") + "\n</instructions>\n\n" + strings.Join(conversation, "\n\n")
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

// parseInlineText accepts bare JSON or one fenced JSON block.
func parseInlineText(text string) (*structuredInline, error) {
	trimmed := strings.TrimSpace(text)
	if strings.HasPrefix(trimmed, "```") {
		_, body, _ := strings.Cut(trimmed, "\n")
		trimmed = strings.TrimSpace(strings.TrimSuffix(strings.TrimSpace(body), "```"))
	}
	var value structuredInline
	if err := json.Unmarshal([]byte(trimmed), &value); err != nil {
		return nil, errors.New("OpenCode returned text that is not inline edit JSON")
	}
	return &value, nil
}

func cleanupSession(cfg config, sessionID string) {
	if strings.TrimSpace(sessionID) == "" {
		return
	}

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodDelete, cfg.opencodeBaseURL+"/api/session/"+url.PathEscape(sessionID), nil)
	if err != nil {
		return
	}
	cfg.authorize(request)
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		return
	}
	defer response.Body.Close()
	_, _ = io.Copy(io.Discard, response.Body)
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
	cfg.authorize(request)
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
		message := bestErrorMessage(data)
		if message == "" {
			message = strings.TrimSpace(response.Status)
		}
		return fmt.Errorf("OpenCode %s: %s", response.Status, message)
	}
	if err := json.Unmarshal(data, out); err != nil {
		return errors.New("OpenCode returned invalid JSON")
	}
	return nil
}

func mustJSON(value any) string {
	data, err := json.Marshal(value)
	if err != nil {
		return `{"code":"","placement":"replace"}`
	}
	return string(data)
}

func responseModel(requestBody chatRequest) string {
	model := strings.TrimSpace(requestBody.Model)
	if model == "" {
		return transportModel
	}
	return model
}

// selectedInlineModel returns nil for the transport alias so OpenCode uses its default model.
func selectedInlineModel(requestBody chatRequest) (*inlineModel, error) {
	model := strings.TrimSpace(requestBody.Model)
	if model == "" || model == transportModel {
		return nil, nil
	}
	parsed, err := parseInlineModel(model)
	if err != nil {
		return nil, err
	}
	return &parsed, nil
}

// parseInlineModel accepts provider/model[#variant]. Only the first slash
// separates the provider because OpenCode 2 model IDs can contain slashes.
func parseInlineModel(model string) (inlineModel, error) {
	raw := strings.TrimSpace(model)
	base, variant, hasVariant := strings.Cut(raw, "#")
	provider, id, ok := strings.Cut(base, "/")
	if !ok || provider == "" || id == "" {
		return inlineModel{}, fmt.Errorf("invalid inline model %q; expected provider/model[#variant]", model)
	}
	if hasVariant && variant == "" {
		return inlineModel{}, fmt.Errorf("invalid inline model %q; variant must be non-empty", model)
	}
	return inlineModel{ProviderID: provider, ModelID: id, Variant: variant}, nil
}

func openCodeModel(model *inlineModel) map[string]string {
	ref := map[string]string{"providerID": model.ProviderID, "id": model.ModelID}
	if model.Variant != "" {
		ref["variant"] = model.Variant
	}
	return ref
}

func writeInlineCompletion(w http.ResponseWriter, model, content string) {
	writeJSON(w, http.StatusOK, map[string]any{
		"id":      fmt.Sprintf("opencode-inline-%d", time.Now().UnixMilli()),
		"object":  "chat.completion",
		"created": time.Now().Unix(),
		"model":   model,
		"choices": []map[string]any{{
			"index": 0,
			"message": map[string]any{
				"role":    "assistant",
				"content": content,
			},
			"finish_reason": "stop",
		}},
	})
}

func writeInlineErrorCompletion(w http.ResponseWriter, model, message string) {
	if strings.TrimSpace(message) == "" {
		message = "Inline request failed"
	}
	writeInlineCompletion(w, model, mustJSON(map[string]string{"error": message}))
}

func validateStructuredInline(value *structuredInline) error {
	if value == nil {
		return errors.New("OpenCode did not return structured output")
	}
	if strings.TrimSpace(value.Code) == "" {
		return errors.New("OpenCode returned structured output without code")
	}
	if !allowedInlinePlacement(strings.TrimSpace(value.Placement)) {
		return fmt.Errorf("OpenCode returned unsupported placement %q", value.Placement)
	}
	return nil
}

func allowedInlinePlacement(placement string) bool {
	for _, allowed := range inlinePlacements() {
		if placement == allowed {
			return true
		}
	}
	return false
}

func normalizeInlineError(err error) string {
	if err == nil {
		return ""
	}
	if errors.Is(err, context.DeadlineExceeded) {
		return "Inline request timed out"
	}
	message := strings.TrimSpace(err.Error())
	if strings.HasPrefix(message, "OpenCode ") {
		if _, rest, ok := strings.Cut(message, ": "); ok {
			message = strings.TrimSpace(rest)
		}
	}
	return message
}

func bestErrorMessage(data []byte) string {
	trimmed := strings.TrimSpace(string(data))
	if trimmed == "" {
		return ""
	}

	var payload any
	if err := json.Unmarshal([]byte(trimmed), &payload); err != nil {
		if strings.HasPrefix(trimmed, "<") {
			return ""
		}
		return trimmed
	}
	if message := extractErrorMessage(payload); message != "" {
		return message
	}
	return trimmed
}

func extractErrorMessage(value any) string {
	switch typed := value.(type) {
	case map[string]any:
		if v, ok := typed["error"]; ok {
			if message := extractErrorMessage(v); message != "" {
				return message
			}
		}
		if v, ok := typed["message"]; ok {
			if message, ok := v.(string); ok && strings.TrimSpace(message) != "" {
				return strings.TrimSpace(message)
			}
		}
		if v, ok := typed["data"]; ok {
			if message := extractErrorMessage(v); message != "" {
				return message
			}
		}
	case string:
		return strings.TrimSpace(typed)
	}
	return ""
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
