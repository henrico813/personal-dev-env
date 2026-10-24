package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Split, so OpenCode's redaction leaves these lines readable to agents.
const (
	peekJWTValue   = "jwt-value-" + "0123456789"
	peekURLPass    = "url-pass-" + "42"
	peekAdminValue = "admin-pass-" + "9876"
)

// The stand-in sops prints this, and no secret in it may reach the output.
const peekPlainJSON = `{
	"stash": {
		"jwt_secret_key": "` + peekJWTValue + `",
		"video_extensions": ["mp4", "mkv"],
		"database_url": "postgres://stash:` + peekURLPass + `@db/stash"
	},
	"admins": [{"name": "alice", "password": "` + peekAdminValue + `"}]
}`

var peekSecretValues = []string{peekJWTValue, peekURLPass, peekAdminValue}

func TestSopsPeekListsKeysWithoutValues(t *testing.T) {
	t.Parallel()
	output, _, err := runSopsPeek(t, "config.yml.sops")
	if err != nil {
		t.Fatalf("%v: %s", err, output)
	}

	want := "stash.jwt_secret_key  string\n" +
		"stash.video_extensions  list of 2\n" +
		"stash.database_url  string\n" +
		"admins.0.name  string\n" +
		"admins.0.password  string\n"
	if output != want {
		t.Fatalf("output = %q, want %q", output, want)
	}
}

func TestSopsPeekPrintsNonSecretValue(t *testing.T) {
	t.Parallel()
	output, _, err := runSopsPeek(t, "config.yml.sops", "stash.video_extensions")
	if err != nil {
		t.Fatalf("%v: %s", err, output)
	}

	if want := "[\n  \"mp4\",\n  \"mkv\"\n]\n"; output != want {
		t.Fatalf("output = %q, want %q", output, want)
	}
}

func TestSopsPeekRefusesSecretValues(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name string
		path string
	}{
		{name: "secret name", path: "stash.jwt_secret_key"},
		{name: "secret key below", path: "admins.0"},
		{name: "password in a URL", path: "stash.database_url"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output, _, err := runSopsPeek(t, "config.yml.sops", tt.path)

			if err == nil || !strings.Contains(output, "refusing "+tt.path) {
				t.Fatalf("pde-sops-peek %s = %q, %v, want a refusal", tt.path, output, err)
			}
			for _, secret := range peekSecretValues {
				if strings.Contains(output, secret) {
					t.Fatalf("output %q holds %q", output, secret)
				}
			}
		})
	}
}

// sops reads a file named only .sops as binary, one value holding the whole
// file, so the format must come from the name.
func TestSopsPeekPicksFormatFromName(t *testing.T) {
	t.Parallel()
	tests := []struct {
		file string
		want string
	}{
		{file: "app.env.sops", want: "--decrypt --input-type dotenv --output-type json app.env.sops\n"},
		{file: "config.yml.sops", want: "--decrypt --input-type yaml --output-type json config.yml.sops\n"},
	}
	for _, tt := range tests {
		t.Run(tt.file, func(t *testing.T) {
			output, args, err := runSopsPeek(t, tt.file)
			if err != nil {
				t.Fatalf("%v: %s", err, output)
			}

			if args != tt.want {
				t.Fatalf("sops arguments = %q, want %q", args, tt.want)
			}
		})
	}
}

func TestSopsPeekRefusesUnknownFormat(t *testing.T) {
	t.Parallel()
	output, args, err := runSopsPeek(t, "notes.sops")

	if err == nil || args != "" {
		t.Fatalf("pde-sops-peek notes.sops = %q, %v with sops arguments %q, want a refusal before sops", output, err, args)
	}
}

// Runs pde-sops-peek with a stand-in sops that prints peekPlainJSON. Returns
// stdout and stderr together, and the arguments sops got, empty if it never ran.
func runSopsPeek(t *testing.T, args ...string) (string, string, error) {
	t.Helper()
	dir := t.TempDir()
	writeApplyFile(t, filepath.Join(dir, "plain.json"), peekPlainJSON)
	writeExecutable(t, filepath.Join(dir, "sops"), `#!/bin/sh
echo "$@" >"$FAKE_SOPS_DIR/args"
cat "$FAKE_SOPS_DIR/plain.json"
`)
	script := filepath.Join(repoRoot(t), "chezmoi", "dot_local", "bin", "executable_pde-sops-peek")
	command := exec.Command("sh", append([]string{script}, args...)...)
	command.Dir = dir
	command.Env = append(command.Environ(),
		"PATH="+dir+":"+os.Getenv("PATH"),
		"FAKE_SOPS_DIR="+dir,
	)
	output, err := command.CombinedOutput()
	recorded, readErr := os.ReadFile(filepath.Join(dir, "args"))
	if readErr != nil && !os.IsNotExist(readErr) {
		t.Fatal(readErr)
	}
	return string(output), string(recorded), err
}
