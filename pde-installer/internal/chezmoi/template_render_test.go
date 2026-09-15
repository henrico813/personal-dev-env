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
		"terminal": {profile: "terminal", want: []string{".config/aquaproj-aqua/aqua.yaml", ".config/alacritty", ".config/wezterm", ".config/nvim", ".config/opencode", ".codex", ".agents", ".pi", ".config/nvim/pack/plugins/start/blink.cmp"}},
		"full":     {profile: "full", want: []string{".config/aquaproj-aqua/aqua-terminal.yaml", ".config/nvim/pack/plugins/start/blink.cmp"}, omit: []string{".config/alacritty", ".config/wezterm", ".config/opencode"}},
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
				"server.env",
				"EDITOR=$(which nvim)",
				"/aqua.yaml",
				"/aqua-checksums.json",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"keychain --eval",
				"node",
				"list-npm-globals",
				"alias vim=",
				"oca() (",
				"ocw() (",
				"$HOME/.config/opencode/server.env",
				"_opencode_run_with_credentials",
				"command opencode attach",
				"command opencode \"$@\"",
				"EDITOR=$(which nvim)",
				"/aqua.yaml",
				"/aqua-checksums.json",
			},
			omit: []string{
				"aqua-terminal.yaml",
				"aqua-terminal-checksums.json",
			},
		},
	}
	assertProfileTemplates(t, "dot_zshrc.tmpl", tests)
}

func TestOCWScopesCredentials(t *testing.T) {
	tests := []struct {
		name        string
		credentials string
		dangling    bool
		wantSuccess bool
	}{
		{
			name:        "valid",
			credentials: "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=secret\n",
			wantSuccess: true,
		},
		{name: "missing"},
		{name: "empty", credentials: "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=\n"},
		{name: "duplicate", credentials: "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_USERNAME=other\nOPENCODE_SERVER_PASSWORD=secret\n"},
		{name: "malformed", credentials: "OPENCODE_SERVER_USERNAME=opencode\nUNEXPECTED=value\n"},
		{name: "dangling", dangling: true},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeOpenCodeZshRuntime(t, home)
			path := filepath.Join(home, ".config", "opencode", "server.env")
			if test.dangling {
				if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
					t.Fatal(err)
				}
				if err := os.Symlink(filepath.Join(home, "missing.env"), path); err != nil {
					t.Fatal(err)
				}
			} else if test.credentials != "" {
				if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
					t.Fatal(err)
				}
				if err := os.WriteFile(path, []byte(test.credentials), 0o600); err != nil {
					t.Fatal(err)
				}
			}
			command := openCodeZshCommand(home, `ocw --hostname 0.0.0.0 --port 4096; result=$?; print -r -- "${OPENCODE_SERVER_USERNAME-}:${OPENCODE_SERVER_PASSWORD-}" >"$HOME/caller"; exit "$result"`)
			output, err := command.CombinedOutput()
			if got := err == nil; got != test.wantSuccess {
				t.Fatalf("ocw success = %t, want %t: %v\n%s", got, test.wantSuccess, err, output)
			}

			caller, err := os.ReadFile(filepath.Join(home, "caller"))
			if err != nil {
				t.Fatal(err)
			}
			if got := string(caller); got != ":\n" {
				t.Fatalf("caller credentials = %q, want empty", got)
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

func TestOCAScopesCredentials(t *testing.T) {
	tests := []struct {
		name        string
		credentials string
		dangling    bool
		inherited   bool
		wantSuccess bool
		wantAuth    string
		wantCaller  string
	}{
		{name: "absent", wantSuccess: true, wantAuth: ":\n", wantCaller: ":\n"},
		{
			name:        "valid",
			credentials: "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=secret\n",
			wantSuccess: true,
			wantAuth:    "opencode:secret\n",
			wantCaller:  ":\n",
		},
		{
			name:        "ignores inherited",
			credentials: "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=secret\n",
			inherited:   true,
			wantSuccess: true,
			wantAuth:    "opencode:secret\n",
			wantCaller:  "inherited:inherited\n",
		},
		{name: "malformed", credentials: "OPENCODE_SERVER_USERNAME=opencode\nUNEXPECTED=value\n", wantCaller: ":\n"},
		{name: "dangling", dangling: true, wantCaller: ":\n"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			writeOpenCodeZshRuntime(t, home)
			path := filepath.Join(home, ".config", "opencode", "server.env")
			if test.dangling {
				if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
					t.Fatal(err)
				}
				if err := os.Symlink(filepath.Join(home, "missing.env"), path); err != nil {
					t.Fatal(err)
				}
			} else if test.credentials != "" {
				if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
					t.Fatal(err)
				}
				if err := os.WriteFile(path, []byte(test.credentials), 0o600); err != nil {
					t.Fatal(err)
				}
			}

			script := `oca --session forwarded; result=$?; print -r -- "${OPENCODE_SERVER_USERNAME-}:${OPENCODE_SERVER_PASSWORD-}" >"$HOME/caller"; exit "$result"`
			if test.inherited {
				script = `export OPENCODE_SERVER_USERNAME=inherited OPENCODE_SERVER_PASSWORD=inherited; ` + script
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

func TestCredentialRunnerKeepsCallerClean(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	path := filepath.Join(home, ".config", "opencode", "server.env")
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=secret\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	command := openCodeZshCommand(home, `_opencode_run_with_credentials web --hostname 0.0.0.0 --port 4096; result=$?; print -r -- "${OPENCODE_SERVER_USERNAME-}:${OPENCODE_SERVER_PASSWORD-}" >"$HOME/caller"; exit "$result"`)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("credential runner failed: %v\n%s", err, output)
	}
	caller, err := os.ReadFile(filepath.Join(home, "caller"))
	if err != nil {
		t.Fatal(err)
	}
	if got := string(caller); got != ":\n" {
		t.Fatalf("caller credentials = %q, want empty", got)
	}
}

func TestCredentialRunnerRejectsOtherCommands(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)

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
			!strings.HasPrefix(value, "OCW_ENV_FILE=") {
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
		writeApplyFile(t, filepath.Join(home, ".local", "share", "zsh", "plugins", "ohmyzsh", "plugins", plugin, plugin+".plugin.zsh"), "")
	}
	writeApplyFile(t, filepath.Join(home, ".local", "share", "zsh", "plugins", "powerlevel10k", "powerlevel10k.zsh-theme"), "")
	writeApplyFile(t, filepath.Join(home, ".local", "share", "zsh", "plugins", "zsh-z", "zsh-z.plugin.zsh"), "")
	writeApplyFile(t, filepath.Join(home, ".local", "share", "zsh", "plugins", "zsh-autosuggestions", "zsh-autosuggestions.zsh"), "")
	writeApplyFile(t, filepath.Join(home, ".local", "share", "zsh", "plugins", "zsh-history-substring-search", "zsh-history-substring-search.zsh"), "")
	writeApplyFile(t, filepath.Join(home, ".local", "share", "zsh", "plugins", "zsh-syntax-highlighting", "zsh-syntax-highlighting.zsh"), "")
	writeExecutable(t, filepath.Join(home, ".local", "bin", "opencode"), "#!/bin/sh\nprintf '%s:%s\\n' \"$OPENCODE_SERVER_USERNAME\" \"$OPENCODE_SERVER_PASSWORD\" >\"$HOME/opencode-credentials\"\nprintf '%s\\n' \"$@\" >\"$HOME/opencode-arguments\"\n")
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
