package chezmoi

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestOpenCodeSystemdUnits(t *testing.T) {
	service := readChezMoiFile(t, "dot_config/systemd/user/opencode-web.service")
	for _, want := range []string{
		"Environment=PATH=%h/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin",
		"EnvironmentFile=%h/.config/opencode/server.env",
		"ExecStart=%h/.local/bin/opencode serve --hostname 127.0.0.1 --port 4096",
		"Restart=on-failure",
	} {
		if !strings.Contains(service, want) {
			t.Errorf("service omits %q", want)
		}
	}
	health := readChezMoiFile(t, "dot_config/systemd/user/opencode-web-health.service")
	for _, want := range []string{
		"ExecStart=/bin/sh -c '/usr/bin/curl --fail --silent --show-error --max-time 2 http://127.0.0.1:4096/ || /usr/bin/systemctl --user restart opencode-web.service'",
	} {
		if !strings.Contains(health, want) {
			t.Errorf("health service omits %q", want)
		}
	}
	timer := readChezMoiFile(t, "dot_config/systemd/user/opencode-web-health.timer")
	if !strings.Contains(timer, "Unit=opencode-web-health.service") {
		t.Error("health timer does not trigger health service")
	}
}

func TestOpenCodeSetupRequiresRegularCredentials(t *testing.T) {
	script := filepath.Join("..", "..", "..", "chezmoi", "run_after_configure_opencode_web.sh.tmpl")
	data, err := os.ReadFile(script)
	if err != nil {
		t.Fatal(err)
	}
	text := string(data)
	for _, want := range []string{
		"[ -f \"$env_file\" ] && [ ! -L \"$env_file\" ]",
		"systemctl --user enable --now \"$service\" \"$timer\"",
		"systemctl --user disable --now \"$service\" \"$timer\"",
		"run ocw-password",
	} {
		if !strings.Contains(text, want) {
			t.Errorf("setup script omits %q", want)
		}
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
