package chezmoi

import (
	"bytes"
	"encoding/json"
	"os/exec"
	"path/filepath"
	"reflect"
	"testing"
)

// Auto memory loads saved notes into every later Claude session. The
// modifier turns it off, keeps the user's other settings, and fails on a
// settings file it cannot read, so chezmoi leaves that file alone.
func TestSettingsModifierOnlyDisablesAutoMemory(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name    string
		input   string
		want    map[string]any
		wantErr bool
	}{
		{
			name:  "missing-file",
			input: "",
			want:  map[string]any{"autoMemoryEnabled": false},
		},
		{
			name:  "existing-settings",
			input: `{"autoMemoryEnabled":true,"model":"opus","env":{"DISABLE_TELEMETRY":"1"}}`,
			want: map[string]any{
				"autoMemoryEnabled": false,
				"model":             "opus",
				"env":               map[string]any{"DISABLE_TELEMETRY": "1"},
			},
		},
		{
			name:    "invalid-json",
			input:   `{"model":`,
			wantErr: true,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			script := filepath.Join("..", "..", "..", "chezmoi", "dot_claude", "modify_private_settings.json")
			command := exec.Command("sh", script)
			command.Stdin = bytes.NewBufferString(test.input)

			output, err := command.CombinedOutput()

			if test.wantErr {
				if err == nil || bytes.Contains(output, []byte("autoMemoryEnabled")) {
					t.Fatalf("modifier replaced unreadable settings: %v\n%s", err, output)
				}
				return
			}
			if err != nil {
				t.Fatalf("modifier failed: %v\n%s", err, output)
			}
			var got map[string]any
			if err := json.Unmarshal(output, &got); err != nil {
				t.Fatal(err)
			}
			if !reflect.DeepEqual(got, test.want) {
				t.Fatalf("settings = %#v, want %#v", got, test.want)
			}
		})
	}
}
