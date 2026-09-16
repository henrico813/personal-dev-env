package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestOpenCodeSystemdUnits(t *testing.T) {
	checks := map[string]string{
		"dot_config/systemd/user/opencode-web.service": `[Unit]
Description=OpenCode attach server

[Service]
Type=exec
Environment=PATH=%h/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin
EnvironmentFile=%h/.config/opencode/server.env
ExecCondition=/bin/sh -c '[ -n "$$OPENCODE_SERVER_USERNAME" ] && [ -n "$$OPENCODE_SERVER_PASSWORD" ]'
ExecStart=%h/.local/bin/opencode serve --hostname 0.0.0.0 --port 4096
Restart=always
RestartSec=5
MemoryMax=1G

[Install]
WantedBy=default.target
`,
		"dot_config/systemd/user/opencode-web-health.service": `[Unit]
Description=Check the OpenCode web server

[Service]
Type=oneshot
EnvironmentFile=%h/.config/opencode/server.env
ExecStart=/bin/sh -c 'printf "user = \"%%s:%%s\"\n" "$$OPENCODE_SERVER_USERNAME" "$$OPENCODE_SERVER_PASSWORD" | /usr/bin/curl --config - --fail --silent --show-error --max-time 10 http://127.0.0.1:4096/global/health >/dev/null || /usr/bin/systemctl --user restart opencode-web.service'
`,
		"dot_config/systemd/user/opencode-web-health.timer": `[Unit]
Description=Check the OpenCode web server periodically

[Timer]
OnBootSec=1min
OnUnitActiveSec=1min
Unit=opencode-web-health.service

[Install]
WantedBy=timers.target
`,
	}
	for name, want := range checks {
		got := readChezMoiFile(t, name)
		if got != want {
			t.Errorf("%s = %q, want %q", name, got, want)
		}
		if name == "dot_config/systemd/user/opencode-web-health.service" {
			curlCommand := strings.SplitN(got, " || ", 2)[0]
			if strings.Contains(curlCommand, "--user") {
				t.Errorf("%s passes credentials with --user", name)
			}
		}
	}
}

func TestHealthCommandReadsCredentialsFromStdin(t *testing.T) {
	home := t.TempDir()
	curl := filepath.Join(home, "curl")
	writeExecutable(t, curl, `#!/bin/sh
printf '%s\n' "$@" >"$CURL_ARGS"
cat >"$CURL_CONFIG"
`)

	unit := readChezMoiFile(t, "dot_config/systemd/user/opencode-web-health.service")
	line := ""
	for _, candidate := range strings.Split(unit, "\n") {
		if strings.HasPrefix(candidate, "ExecStart=") {
			line = candidate
			break
		}
	}
	command := strings.TrimPrefix(line, "ExecStart=/bin/sh -c '")
	command = strings.TrimSuffix(command, "'")
	// Systemd unescapes dollar signs and percent specifiers before invoking the shell.
	command = strings.ReplaceAll(command, "$$", "$")
	command = strings.ReplaceAll(command, "%%", "%")
	command = strings.Replace(command, "/usr/bin/curl", curl, 1)
	command = strings.Replace(command, "/usr/bin/systemctl", filepath.Join(home, "systemctl"), 1)

	configPath := filepath.Join(home, "curl-config")
	argsPath := filepath.Join(home, "curl-args")
	process := exec.Command("sh", "-c", command)
	process.Env = []string{
		"CURL_CONFIG=" + configPath,
		"CURL_ARGS=" + argsPath,
		"OPENCODE_SERVER_USERNAME=test-user",
		"OPENCODE_SERVER_PASSWORD=test-password",
	}
	if output, err := process.CombinedOutput(); err != nil {
		t.Fatalf("health command failed: %v\n%s", err, output)
	}
	if got := readPasswordFile(t, configPath); got != "user = \"test-user:test-password\"\n" {
		t.Fatalf("curl config = %q", got)
	}
	if got := readPasswordFile(t, argsPath); strings.Contains(got, "test-password") || strings.Contains(got, "--user") {
		t.Fatalf("curl arguments expose credentials: %q", got)
	}
	if got := readPasswordFile(t, argsPath); !strings.Contains(got, "http://127.0.0.1:4096/global/health") {
		t.Fatalf("curl arguments omit health URL: %q", got)
	}
}

func TestOpenCodeSetupHandlesCredentials(t *testing.T) {
	checks := map[string]struct {
		credentials bool
		want        string
	}{
		"regular-credentials": {
			credentials: true,
			want:        "--user daemon-reload\n--user enable opencode-web.service opencode-web-health.timer\n--user restart opencode-web.service\n--user start opencode-web-health.timer\n",
		},
		"missing-credentials": {
			want: "--user daemon-reload\n--user disable --now opencode-web.service opencode-web-health.timer\n",
		},
	}
	for name, check := range checks {
		t.Run(name, func(t *testing.T) {
			home := t.TempDir()
			if check.credentials {
				writeOpenCodeCredentials(t, home, validOpenCodeCredentials)
			}
			bin := filepath.Join(home, ".local", "bin")
			writeExecutable(t, filepath.Join(bin, "systemctl"), fakeSystemctlScript)
			script := filepath.Join(home, "setup.sh")
			writeExecutable(t, script, renderProfileTemplate(t, "run_after_configure_opencode_web.sh.tmpl", "full"))

			command := exec.Command("sh", script)
			command.Env = []string{"HOME=" + home, "PATH=" + bin}
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf("setup failed: %v\n%s", err, output)
			}
			if got := readPasswordFile(t, filepath.Join(home, "systemctl-arguments")); got != check.want {
				t.Fatalf("systemctl arguments = %q, want %q", got, check.want)
			}
		})
	}
}

func readChezMoiFile(t *testing.T, name string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("..", "..", "..", "chezmoi", name))
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}
