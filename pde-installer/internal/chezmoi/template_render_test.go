package chezmoi

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"text/template"
)

func TestIgnoreTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {
			profile: "terminal",
			want: []string{
				".config/aquaproj-aqua/aqua.yaml",
				".config/alacritty",
				".config/wezterm",
				".config/nvim",
				".config/opencode",
				".config/systemd/user/opencode-web.service",
				".config/systemd/user/opencode-web-health.service",
				".config/systemd/user/opencode-web-health.timer",
				".codex",
				".agents",
				".pi",
				".config/nvim/pack/plugins/start/blink.cmp",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				".config/aquaproj-aqua/aqua-terminal.yaml",
				".config/nvim/pack/plugins/start/blink.cmp",
			},
			omit: []string{
				".config/alacritty",
				".config/wezterm",
				".config/opencode",
				".config/systemd/user/opencode-web.service",
				".config/systemd/user/opencode-web-health.service",
				".config/systemd/user/opencode-web-health.timer",
			},
		},
	}
	assertProfileTemplates(t, ".chezmoiignore.tmpl", tests)
}

func TestExternalTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {
			profile: "terminal",
			want:    []string{".tmux/plugins/tmux-resurrect", "type = \"archive\""},
			omit: []string{
				"obsidian.nvim",
				".pi/agent/settings.json",
				".agents/skills/obsidian-zettel/SKILL.md",
				".codex/skills/obsidian-zettel/SKILL.md",
				".agents/skills/go-development/Apache-2.0.txt",
				".agents/skills/go-development/README.md",
				".agents/skills/go-development/SKILL.md",
				".agents/skills/go-development/examples/composition-original.go",
				".agents/skills/go-development/references/composition.md",
				".agents/skills/go-development/references/uber-go-guide.md",
				".codex/skills/go-development/Apache-2.0.txt",
				".codex/skills/go-development/README.md",
				".codex/skills/go-development/SKILL.md",
				".codex/skills/go-development/examples/composition-original.go",
				".codex/skills/go-development/references/composition.md",
				".codex/skills/go-development/references/uber-go-guide.md",
				// Rust Agent Skills package.
				".agents/skills/rust-development/SKILL.md",
				".agents/skills/rust-development/examples/.gitignore",
				".agents/skills/rust-development/examples/Cargo.lock",
				".agents/skills/rust-development/examples/Cargo.toml",
				".agents/skills/rust-development/examples/README.md",
				".agents/skills/rust-development/examples/src/buffered_output.rs",
				".agents/skills/rust-development/examples/src/device.rs",
				".agents/skills/rust-development/examples/src/foreign.rs",
				".agents/skills/rust-development/examples/src/identity.rs",
				".agents/skills/rust-development/examples/src/lib.rs",
				".agents/skills/rust-development/examples/src/main.rs",
				".agents/skills/rust-development/examples/src/report.rs",
				".agents/skills/rust-development/examples/src/retry.rs",
				".agents/skills/rust-development/examples/src/threaded.rs",
				".agents/skills/rust-development/examples/tests/cli.rs",
				".agents/skills/rust-development/references/api-design.md",
				".agents/skills/rust-development/references/cargo.md",
				".agents/skills/rust-development/references/concurrency.md",
				".agents/skills/rust-development/references/design-choices.md",
				".agents/skills/rust-development/references/documentation.md",
				".agents/skills/rust-development/references/errors-resources.md",
				".agents/skills/rust-development/references/evaluation.md",
				".agents/skills/rust-development/references/examples.md",
				".agents/skills/rust-development/references/ownership.md",
				".agents/skills/rust-development/references/sources.md",
				".agents/skills/rust-development/references/testing.md",
				".agents/skills/rust-development/references/unsafe.md",
				// Rust Codex package.
				".codex/skills/rust-development/SKILL.md",
				".codex/skills/rust-development/examples/.gitignore",
				".codex/skills/rust-development/examples/Cargo.lock",
				".codex/skills/rust-development/examples/Cargo.toml",
				".codex/skills/rust-development/examples/README.md",
				".codex/skills/rust-development/examples/src/buffered_output.rs",
				".codex/skills/rust-development/examples/src/device.rs",
				".codex/skills/rust-development/examples/src/foreign.rs",
				".codex/skills/rust-development/examples/src/identity.rs",
				".codex/skills/rust-development/examples/src/lib.rs",
				".codex/skills/rust-development/examples/src/main.rs",
				".codex/skills/rust-development/examples/src/report.rs",
				".codex/skills/rust-development/examples/src/retry.rs",
				".codex/skills/rust-development/examples/src/threaded.rs",
				".codex/skills/rust-development/examples/tests/cli.rs",
				".codex/skills/rust-development/references/api-design.md",
				".codex/skills/rust-development/references/cargo.md",
				".codex/skills/rust-development/references/concurrency.md",
				".codex/skills/rust-development/references/design-choices.md",
				".codex/skills/rust-development/references/documentation.md",
				".codex/skills/rust-development/references/errors-resources.md",
				".codex/skills/rust-development/references/evaluation.md",
				".codex/skills/rust-development/references/examples.md",
				".codex/skills/rust-development/references/ownership.md",
				".codex/skills/rust-development/references/sources.md",
				".codex/skills/rust-development/references/testing.md",
				".codex/skills/rust-development/references/unsafe.md",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"obsidian.nvim",
				".pi/agent/settings.json",
				"implement-plan/SKILL.md",
				".agents/skills/obsidian-zettel/SKILL.md",
				".codex/skills/obsidian-zettel/SKILL.md",
				".agents/skills/go-development/Apache-2.0.txt",
				".agents/skills/go-development/README.md",
				".agents/skills/go-development/SKILL.md",
				".agents/skills/go-development/examples/composition-original.go",
				".agents/skills/go-development/references/composition.md",
				".agents/skills/go-development/references/uber-go-guide.md",
				".codex/skills/go-development/Apache-2.0.txt",
				".codex/skills/go-development/README.md",
				".codex/skills/go-development/SKILL.md",
				".codex/skills/go-development/examples/composition-original.go",
				".codex/skills/go-development/references/composition.md",
				".codex/skills/go-development/references/uber-go-guide.md",
				// Rust Agent Skills package.
				".agents/skills/rust-development/SKILL.md",
				".agents/skills/rust-development/examples/.gitignore",
				".agents/skills/rust-development/examples/Cargo.lock",
				".agents/skills/rust-development/examples/Cargo.toml",
				".agents/skills/rust-development/examples/README.md",
				".agents/skills/rust-development/examples/src/buffered_output.rs",
				".agents/skills/rust-development/examples/src/device.rs",
				".agents/skills/rust-development/examples/src/foreign.rs",
				".agents/skills/rust-development/examples/src/identity.rs",
				".agents/skills/rust-development/examples/src/lib.rs",
				".agents/skills/rust-development/examples/src/main.rs",
				".agents/skills/rust-development/examples/src/report.rs",
				".agents/skills/rust-development/examples/src/retry.rs",
				".agents/skills/rust-development/examples/src/threaded.rs",
				".agents/skills/rust-development/examples/tests/cli.rs",
				".agents/skills/rust-development/references/api-design.md",
				".agents/skills/rust-development/references/cargo.md",
				".agents/skills/rust-development/references/concurrency.md",
				".agents/skills/rust-development/references/design-choices.md",
				".agents/skills/rust-development/references/documentation.md",
				".agents/skills/rust-development/references/errors-resources.md",
				".agents/skills/rust-development/references/evaluation.md",
				".agents/skills/rust-development/references/examples.md",
				".agents/skills/rust-development/references/ownership.md",
				".agents/skills/rust-development/references/sources.md",
				".agents/skills/rust-development/references/testing.md",
				".agents/skills/rust-development/references/unsafe.md",
				// Rust Codex package.
				".codex/skills/rust-development/SKILL.md",
				".codex/skills/rust-development/examples/.gitignore",
				".codex/skills/rust-development/examples/Cargo.lock",
				".codex/skills/rust-development/examples/Cargo.toml",
				".codex/skills/rust-development/examples/README.md",
				".codex/skills/rust-development/examples/src/buffered_output.rs",
				".codex/skills/rust-development/examples/src/device.rs",
				".codex/skills/rust-development/examples/src/foreign.rs",
				".codex/skills/rust-development/examples/src/identity.rs",
				".codex/skills/rust-development/examples/src/lib.rs",
				".codex/skills/rust-development/examples/src/main.rs",
				".codex/skills/rust-development/examples/src/report.rs",
				".codex/skills/rust-development/examples/src/retry.rs",
				".codex/skills/rust-development/examples/src/threaded.rs",
				".codex/skills/rust-development/examples/tests/cli.rs",
				".codex/skills/rust-development/references/api-design.md",
				".codex/skills/rust-development/references/cargo.md",
				".codex/skills/rust-development/references/concurrency.md",
				".codex/skills/rust-development/references/design-choices.md",
				".codex/skills/rust-development/references/documentation.md",
				".codex/skills/rust-development/references/errors-resources.md",
				".codex/skills/rust-development/references/evaluation.md",
				".codex/skills/rust-development/references/examples.md",
				".codex/skills/rust-development/references/ownership.md",
				".codex/skills/rust-development/references/sources.md",
				".codex/skills/rust-development/references/testing.md",
				".codex/skills/rust-development/references/unsafe.md",
			},
			omit: []string{"PDE_PROFILE must be exactly"},
		},
	}
	assertProfileTemplates(t, ".chezmoiexternal.toml.tmpl", tests)
}

func TestZshTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {
			profile: "terminal",
			want: []string{
				"aqua-terminal.yaml",
				"aqua-terminal-checksums.json",
				"colored-man-pages",
				"HISTSIZE=1000000",
			},
			omit: []string{
				"keychain --eval",
				"node{{",
				"list-npm-globals",
				"alias vim=",
				"oca()",
				"ocw()",
				"ocw-password()",
				"server.env",
				"EDITOR=$(which nvim)",
				"/aqua.yaml",
				"/aqua-checksums.json",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"oca() (",
				"ocw() (",
				"ocw-password() (",
				"$HOME/.config/opencode/server.env",
			},
			omit: []string{
				"aqua-terminal.yaml",
				"aqua-terminal-checksums.json",
			},
		},
	}
	assertProfileTemplates(t, "dot_zshrc.tmpl", tests)
}

const (
	fakeCurlScript = `#!/bin/sh
count_file="$HOME/curl-count"
count=0
[ -f "$count_file" ] && count=$(cat "$count_file")
printf '%s\n' "$@" >>"$HOME/curl-arguments"
failures=${PDE_TEST_CURL_FAILURES:-0}
if [ "$count" -lt "$failures" ]; then
    printf '%s\n' $((count + 1)) >"$count_file"
    exit 1
fi
exit 0
`
	fakeSystemctlScript = `#!/bin/sh
printf '%s\n' "$*" >"$HOME/systemctl-arguments"
`
	fakeSleepScript = `#!/bin/sh
exit 0
`
	validOpenCodeCredentials = `OPENCODE_SERVER_USERNAME=opencode
OPENCODE_SERVER_PASSWORD=secret
`
	emptyOpenCodePassword     = "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=\n"
	duplicateOpenCodeUsername = `OPENCODE_SERVER_USERNAME=opencode
OPENCODE_SERVER_USERNAME=other
OPENCODE_SERVER_PASSWORD=secret
`
	malformedOpenCodeCredentials = `OPENCODE_SERVER_USERNAME=opencode
UNEXPECTED=value
`
	ocwScript = `
ocw --hostname 0.0.0.0 --port 4096
result=$?
print -r -- "${OPENCODE_SERVER_USERNAME-}:${OPENCODE_SERVER_PASSWORD-}" >"$HOME/caller"
exit "$result"
`
	ocaScript = `
oca --session forwarded
result=$?
print -r -- "${OPENCODE_SERVER_USERNAME-}:${OPENCODE_SERVER_PASSWORD-}" >"$HOME/caller"
exit "$result"
`
	fakeOpenCodeScript = `#!/bin/sh
printf '%s:%s\n' "$OPENCODE_SERVER_USERNAME" "$OPENCODE_SERVER_PASSWORD" >"$HOME/opencode-credentials"
printf '%s\n' "$@" >"$HOME/opencode-arguments"
`
)

func TestOCWScopesCredentials(t *testing.T) {
	tests := []struct {
		name        string
		credentials string
		symlink     bool
		inherited   bool
		wantSuccess bool
		wantCaller  string
	}{
		{
			name:        "valid",
			credentials: validOpenCodeCredentials,
			wantSuccess: true,
			wantCaller:  ":\n",
		},
		{name: "missing", wantCaller: ":\n"},
		{name: "empty", credentials: emptyOpenCodePassword, wantCaller: ":\n"},
		{name: "duplicate", credentials: duplicateOpenCodeUsername, wantCaller: ":\n"},
		{name: "malformed", credentials: malformedOpenCodeCredentials, wantCaller: ":\n"},
		{name: "symlink", credentials: validOpenCodeCredentials, symlink: true, wantCaller: ":\n"},
		{
			name:        "ignores inherited",
			credentials: validOpenCodeCredentials,
			inherited:   true,
			wantSuccess: true,
			wantCaller:  "inherited:inherited\n",
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeOpenCodeZshRuntime(t, home)
			if test.symlink {
				writeOpenCodeSymlink(t, home, test.credentials)
			} else if test.credentials != "" {
				writeOpenCodeCredentials(t, home, test.credentials)
			}
			script := ocwScript
			if test.inherited {
				script = withInheritedOpenCodeCredentials(script)
			}
			command := openCodeZshCommand(home, script)
			output, err := command.CombinedOutput()
			if got := err == nil; got != test.wantSuccess {
				t.Fatalf("ocw success = %t, want %t: %v\n%s", got, test.wantSuccess, err, output)
			}

			caller, err := os.ReadFile(filepath.Join(home, "caller"))
			if err != nil {
				t.Fatal(err)
			}
			if got := string(caller); got != test.wantCaller {
				t.Fatalf("caller credentials = %q, want %q", got, test.wantCaller)
			}
			if !test.wantSuccess {
				if _, err := os.Stat(filepath.Join(home, "opencode-arguments")); !os.IsNotExist(err) {
					t.Fatalf("opencode ran: %v", err)
				}
				return
			}
			credentials, err := os.ReadFile(filepath.Join(home, "opencode-credentials"))
			if err != nil {
				t.Fatal(err)
			}
			if got := string(credentials); got != "opencode:secret\n" {
				t.Fatalf("opencode credentials = %q", got)
			}
			arguments, err := os.ReadFile(filepath.Join(home, "opencode-arguments"))
			if err != nil {
				t.Fatal(err)
			}
			if got := string(arguments); got != "web\n--hostname\n0.0.0.0\n--port\n4096\n" {
				t.Fatalf("opencode arguments = %q", got)
			}
		})
	}
}

func TestOCAUsesReadyDefaultServer(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	command := openCodeZshCommand(home, `oca --session forwarded`)

	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("oca failed: %v\n%s", err, output)
	}

	calls, err := os.ReadFile(filepath.Join(home, "curl-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := strings.Count(string(calls), "http://127.0.0.1:4096"); got != 1 {
		t.Fatalf("curl probes = %d, want 1", got)
	}
	if _, err := os.Stat(filepath.Join(home, "systemctl-arguments")); !os.IsNotExist(err) {
		t.Fatalf("ready server restarted: %v", err)
	}
	arguments, err := os.ReadFile(filepath.Join(home, "opencode-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := string(arguments); !strings.Contains(got, "attach\nhttp://127.0.0.1:4096\n--dir\n") || !strings.HasSuffix(got, "--session\nforwarded\n") {
		t.Fatalf("opencode arguments = %q", got)
	}
}

func TestOCARecoversDefaultServer(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	writeOpenCodeCredentials(t, home, validOpenCodeCredentials)
	command := openCodeZshCommand(home, `oca --session forwarded`)
	command.Env = append(command.Env, "PDE_TEST_CURL_FAILURES=3")

	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("oca failed: %v\n%s", err, output)
	}

	calls, err := os.ReadFile(filepath.Join(home, "curl-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := strings.Count(string(calls), "http://127.0.0.1:4096"); got != 4 {
		t.Fatalf("curl probes = %d, want 4", got)
	}
	units, err := os.ReadFile(filepath.Join(home, "systemctl-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := string(units); got != "--user restart opencode-web.service\n" {
		t.Fatalf("systemctl arguments = %q", got)
	}
	arguments, err := os.ReadFile(filepath.Join(home, "opencode-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := string(arguments); !strings.Contains(got, "attach\nhttp://127.0.0.1:4096\n--dir\n") || !strings.HasSuffix(got, "--session\nforwarded\n") {
		t.Fatalf("opencode arguments = %q", got)
	}
	credentials, err := os.ReadFile(filepath.Join(home, "opencode-credentials"))
	if err != nil {
		t.Fatal(err)
	}
	if got := string(credentials); got != "opencode:secret\n" {
		t.Fatalf("opencode credentials = %q", got)
	}
}

func TestOCAStopsAfterElevenProbes(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	command := openCodeZshCommand(home, `oca --session forwarded`)
	command.Env = append(command.Env, "PDE_TEST_CURL_FAILURES=11")

	output, err := command.CombinedOutput()
	if err == nil {
		t.Fatalf("oca succeeded: %s", output)
	}
	if !strings.Contains(string(output), "OpenCode server did not become ready: http://127.0.0.1:4096") {
		t.Fatalf("missing readiness error: %s", output)
	}
	calls, err := os.ReadFile(filepath.Join(home, "curl-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := strings.Count(string(calls), "http://127.0.0.1:4096"); got != 11 {
		t.Fatalf("curl probes = %d, want 11", got)
	}
	if _, err := os.Stat(filepath.Join(home, "opencode-arguments")); !os.IsNotExist(err) {
		t.Fatalf("opencode ran: %v", err)
	}
}

func TestOCAOverrideSkipsRecovery(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	command := openCodeZshCommand(home, `export OPENCODE_ATTACH_URL=http://example.test:4096
oca --session forwarded`)
	command.Env = append(command.Env, "PDE_TEST_CURL_FAILURES=10")

	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("oca failed: %v\n%s", err, output)
	}
	if _, err := os.Stat(filepath.Join(home, "curl-arguments")); !os.IsNotExist(err) {
		t.Fatalf("override ran recovery probe: %v", err)
	}
	if _, err := os.Stat(filepath.Join(home, "systemctl-arguments")); !os.IsNotExist(err) {
		t.Fatalf("override restarted server: %v", err)
	}
	arguments, err := os.ReadFile(filepath.Join(home, "opencode-arguments"))
	if err != nil {
		t.Fatal(err)
	}
	if got := string(arguments); !strings.Contains(got, "attach\nhttp://example.test:4096\n--dir\n") {
		t.Fatalf("opencode arguments = %q", got)
	}
}

func TestOCAScopesCredentials(t *testing.T) {
	tests := []struct {
		name        string
		credentials string
		symlink     bool
		inherited   bool
		wantSuccess bool
		wantAuth    string
		wantCaller  string
	}{
		{name: "absent", wantSuccess: true, wantAuth: ":\n", wantCaller: ":\n"},
		{
			name:        "absent ignores inherited",
			inherited:   true,
			wantSuccess: true,
			wantAuth:    ":\n",
			wantCaller:  "inherited:inherited\n",
		},
		{
			name:        "valid",
			credentials: validOpenCodeCredentials,
			wantSuccess: true,
			wantAuth:    "opencode:secret\n",
			wantCaller:  ":\n",
		},
		{
			name:        "ignores inherited",
			credentials: validOpenCodeCredentials,
			inherited:   true,
			wantSuccess: true,
			wantAuth:    "opencode:secret\n",
			wantCaller:  "inherited:inherited\n",
		},
		{name: "malformed", credentials: malformedOpenCodeCredentials, wantCaller: ":\n"},
		{name: "symlink", credentials: validOpenCodeCredentials, symlink: true, wantCaller: ":\n"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeOpenCodeZshRuntime(t, home)
			if test.symlink {
				writeOpenCodeSymlink(t, home, test.credentials)
			} else if test.credentials != "" {
				writeOpenCodeCredentials(t, home, test.credentials)
			}

			script := ocaScript
			if test.inherited {
				script = withInheritedOpenCodeCredentials(script)
			}
			command := openCodeZshCommand(home, script)
			output, err := command.CombinedOutput()
			if got := err == nil; got != test.wantSuccess {
				t.Fatalf("oca success = %t, want %t: %v\n%s", got, test.wantSuccess, err, output)
			}

			caller, err := os.ReadFile(filepath.Join(home, "caller"))
			if err != nil {
				t.Fatal(err)
			}
			if got := string(caller); got != test.wantCaller {
				t.Fatalf("caller credentials = %q, want %q", got, test.wantCaller)
			}
			if !test.wantSuccess {
				if _, err := os.Stat(filepath.Join(home, "opencode-arguments")); !os.IsNotExist(err) {
					t.Fatalf("opencode ran: %v", err)
				}
				return
			}
			auth, err := os.ReadFile(filepath.Join(home, "opencode-credentials"))
			if err != nil {
				t.Fatal(err)
			}
			if got := string(auth); got != test.wantAuth {
				t.Fatalf("opencode credentials = %q, want %q", got, test.wantAuth)
			}
			arguments, err := os.ReadFile(filepath.Join(home, "opencode-arguments"))
			if err != nil {
				t.Fatal(err)
			}
			wantArguments := "attach\nhttp://127.0.0.1:4096\n--dir\n" + repoRoot(t) + "\n--session\nforwarded\n"
			if got := string(arguments); got != wantArguments {
				t.Fatalf("opencode arguments = %q, want %q", got, wantArguments)
			}
		})
	}
}

func TestCredentialRunnerRejectsOtherCommands(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	writeOpenCodeCredentials(t, home, validOpenCodeCredentials)

	command := openCodeZshCommand(home, `_opencode_run_with_credentials status`)
	if output, err := command.CombinedOutput(); err == nil {
		t.Fatalf("credential runner accepted status: %s", output)
	}
	if _, err := os.Stat(filepath.Join(home, "opencode-arguments")); !os.IsNotExist(err) {
		t.Fatalf("opencode ran: %v", err)
	}
}

func openCodeZshCommand(home, script string) *exec.Cmd {
	command := exec.Command("zsh", "-i", "-c", script)
	for _, value := range os.Environ() {
		if !strings.HasPrefix(value, "HOME=") &&
			!strings.HasPrefix(value, "ZDOTDIR=") &&
			!strings.HasPrefix(value, "OPENCODE_SERVER_USERNAME=") &&
			!strings.HasPrefix(value, "OPENCODE_SERVER_PASSWORD=") &&
			!strings.HasPrefix(value, "OPENCODE_ATTACH_URL=") {
			command.Env = append(command.Env, value)
		}
	}
	command.Env = append(command.Env, "HOME="+home, "ZDOTDIR="+home)
	return command
}

func writeOpenCodeZshRuntime(t *testing.T, home string) {
	t.Helper()
	writeApplyFile(t, filepath.Join(home, ".zshrc"), renderProfileTemplate(t, "dot_zshrc.tmpl", "full"))
	for _, plugin := range []string{"colored-man-pages", "command-not-found", "git", "git-extras", "node"} {
		writeApplyFile(t, zshPluginPath(home, "ohmyzsh/plugins/"+plugin, plugin+".plugin.zsh"), "")
	}
	for _, path := range []string{
		zshPluginPath(home, "powerlevel10k", "powerlevel10k.zsh-theme"),
		zshPluginPath(home, "zsh-z", "zsh-z.plugin.zsh"),
		zshPluginPath(home, "zsh-autosuggestions", "zsh-autosuggestions.zsh"),
		zshPluginPath(home, "zsh-history-substring-search", "zsh-history-substring-search.zsh"),
		zshPluginPath(home, "zsh-syntax-highlighting", "zsh-syntax-highlighting.zsh"),
	} {
		writeApplyFile(t, path, "")
	}
	writeExecutable(t, filepath.Join(home, ".local", "bin", "opencode"), fakeOpenCodeScript)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "curl"), fakeCurlScript)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), fakeSystemctlScript)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "sleep"), fakeSleepScript)
}

func zshPluginPath(home, plugin, file string) string {
	return filepath.Join(home, ".local", "share", "zsh", "plugins", filepath.FromSlash(plugin), file)
}

func withInheritedOpenCodeCredentials(script string) string {
	return "export OPENCODE_SERVER_USERNAME=inherited OPENCODE_SERVER_PASSWORD=inherited\n" + script
}

func writeOpenCodeCredentials(t *testing.T, home, contents string) {
	t.Helper()
	writeOpenCodeCredentialsAt(t, filepath.Join(home, ".config", "opencode", "server.env"), contents)
}

func writeOpenCodeSymlink(t *testing.T, home, contents string) {
	t.Helper()
	target := filepath.Join(home, "credentials.env")
	writeOpenCodeCredentialsAt(t, target, contents)
	link := filepath.Join(home, ".config", "opencode", "server.env")
	if err := os.MkdirAll(filepath.Dir(link), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, link); err != nil {
		t.Fatal(err)
	}
}

func writeOpenCodeCredentialsAt(t *testing.T, path, contents string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(contents), 0o600); err != nil {
		t.Fatal(err)
	}
}

func TestTmuxTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {profile: "terminal", want: []string{"set -s set-clipboard on", "allow-passthrough on", "@resurrect-processes 'ssh'"}, omit: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'"}},
		"full":     {profile: "full", want: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'", "set -s set-clipboard on"}, omit: []string{"@resurrect-processes 'ssh'"}},
	}
	assertProfileTemplates(t, "dot_tmux.conf.tmpl", tests)
}

func TestTemplatesRejectInvalidProfiles(t *testing.T) {
	for name, selected := range map[string]string{"missing": "", "unknown": "desktop"} {
		t.Run(name, func(t *testing.T) {
			if _, err := renderTemplate(t, ".chezmoiignore.tmpl", selected); err == nil {
				t.Fatal("render succeeded")
			}
		})
	}
}

func assertProfileTemplates(t *testing.T, templateName string, tests map[string]struct {
	profile string
	want    []string
	omit    []string
}) {
	t.Helper()
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			text := renderProfileTemplate(t, templateName, test.profile)
			for _, value := range test.want {
				if !strings.Contains(text, value) {
					t.Errorf("%s omits %q", templateName, value)
				}
			}
			for _, value := range test.omit {
				if strings.Contains(text, value) {
					t.Errorf("%s contains %q", templateName, value)
				}
			}
		})
	}
}

func TestExternalChecksumsMatchSources(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full")
	if strings.Count(text, "type = ") != strings.Count(text, "checksum.sha256") {
		t.Fatal("external types and checksums differ")
	}
	pattern := regexp.MustCompile(`(?m)^url = 'file://([^']+)'\nchecksum.sha256 = "([^"]+)"`)
	matches := pattern.FindAllStringSubmatch(text, -1)
	if len(matches) == 0 {
		t.Fatal("no local externals rendered")
	}
	for _, match := range matches {
		data, err := os.ReadFile(match[1])
		if err != nil {
			t.Fatal(err)
		}
		hash := sha256.Sum256(data)
		if got := hex.EncodeToString(hash[:]); got != match[2] {
			t.Errorf("checksum for %s = %s, want %s", match[1], got, match[2])
		}
	}
}

func renderProfileTemplate(t *testing.T, name, profile string) string {
	t.Helper()
	text, err := renderTemplate(t, name, profile)
	if err != nil {
		t.Fatal(err)
	}
	return text
}

func renderTemplate(t *testing.T, name, profile string) (string, error) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", name))
	if err != nil {
		t.Fatal(err)
	}
	functions := template.FuncMap{
		"env": func(key string) string {
			if key == "PDE_PROFILE" {
				return profile
			}
			if key == "PDE_REPO_ROOT" {
				return repoRoot(t)
			}
			return ""
		},
		"fail": func(message string) (string, error) { return "", &templateError{message} },
	}
	tmpl, err := template.New(name).Funcs(functions).Parse(string(data))
	if err != nil {
		return "", err
	}
	var output bytes.Buffer
	if err := tmpl.Execute(&output, nil); err != nil {
		return "", err
	}
	return output.String(), nil
}

type templateError struct{ message string }

func (e *templateError) Error() string { return e.message }

func repoRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	return root
}
