package chezmoi

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestModifierMergesMemoryPlugin(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name  string
		input string
		want  []string
	}{
		{name: "empty", input: "", want: []string{"opencode-mem@2.25.0"}},
		{name: "existing", input: `{"plugin":["example@1","opencode-mem@2.25.0"]}`, want: []string{"example@1", "opencode-mem@2.25.0"}},
		{name: "duplicate", input: `{"plugin":["opencode-mem@2.25.0","opencode-mem@2.25.0"]}`, want: []string{"opencode-mem@2.25.0"}},
		{name: "unversioned", input: `{"plugin":["opencode-mem"]}`, want: []string{"opencode-mem@2.25.0"}},
		{name: "other version", input: `{"plugin":["opencode-mem@2.24.0"]}`, want: []string{"opencode-mem@2.25.0"}},
		{name: "tuple", input: `{"plugin":[["opencode-mem@2.24.0",{}]]}`, want: []string{"opencode-mem@2.25.0"}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output, err := runModifier(t, tt.input)
			if err != nil {
				t.Fatal(err)
			}
			var config struct {
				Plugin []string `json:"plugin"`
			}
			if err := json.Unmarshal(output, &config); err != nil {
				t.Fatal(err)
			}
			if strings.Join(config.Plugin, "|") != strings.Join(tt.want, "|") {
				t.Fatalf("plugin = %v, want %v", config.Plugin, tt.want)
			}
		})
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
