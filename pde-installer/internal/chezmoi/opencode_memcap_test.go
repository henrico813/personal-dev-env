package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestOpenCodeMemcapLauncher(t *testing.T) {
	tests := []struct {
		name        string
		userBus     bool
		wantCap     bool
		wantWarning bool
	}{
		{name: "caps root launch", userBus: true, wantCap: true},
		{name: "falls back without user bus", wantWarning: true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			writeOpenCodeBinary(t, home)
			writeMemcapSystemdStubs(t, home, tt.userBus)
			wrapper := writeMemcapLauncher(t, home)
			command := exec.Command(wrapper, "serve", "--service")
			// Test the wrapper defaults instead of settings inherited from its runner.
			command.Env = append(os.Environ(),
				"HOME="+home,
				"PATH="+filepath.Join(home, ".local", "bin")+":"+os.Getenv("PATH"),
				"OPENCODE_MEMCAP_SCOPE=",
				"PDE_OPENCODE_MEMORY_MAX=",
				"PDE_OPENCODE_MEMORY_SWAP_MAX=",
			)
			output, err := command.CombinedOutput()
			if err != nil {
				t.Fatalf("launcher failed: %v\n%s", err, output)
			}
			calls := readMemcapCalls(t, filepath.Join(home, "systemd-run"))
			if tt.wantCap {
				for _, want := range []string{"--slice=opencode.slice", "MemoryMax=50%", "MemorySwapMax=512M", "OPENCODE_MEMCAP_SCOPE=1"} {
					if !strings.Contains(calls, want) {
						t.Errorf("systemd-run call %q omits %q", calls, want)
					}
				}
			} else if calls != "" {
				t.Errorf("systemd-run called for direct launch: %q", calls)
			}
			if got, want := readMemcapCalls(t, filepath.Join(home, "opencode")), "serve\n--service\n"; got != want {
				t.Errorf("OpenCode argv = %q, want %q", got, want)
			}
			if got := strings.Contains(string(output), "running uncapped"); got != tt.wantWarning {
				t.Errorf("uncapped warning = %t, want %t", got, tt.wantWarning)
			}
		})
	}
}

func writeMemcapLauncher(t *testing.T, home string) string {
	t.Helper()
	source := filepath.Join(repoRoot(t), "chezmoi", "dot_local", "bin", "executable_opencode")
	data, err := os.ReadFile(source)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(home, "launcher")
	writeExecutable(t, path, string(data))
	return path
}

func writeOpenCodeBinary(t *testing.T, home string) {
	t.Helper()
	writeExecutable(t, filepath.Join(home, ".local", "share", "pde", "npm", "node_modules", ".bin", "opencode"), "#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$HOME/opencode\"\n")
}

func writeMemcapSystemdStubs(t *testing.T, home string, userBus bool) {
	t.Helper()
	status := "exit 1"
	if userBus {
		status = "exit 0"
	}
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), "#!/bin/sh\n"+status+"\n")
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemd-run"), "#!/bin/sh\nprintf '%s\\n' \"$*\" >\"$HOME/systemd-run\"\nwhile [ \"$1\" != env ]; do shift; done\nexec \"$@\"\n")
}

func readMemcapCalls(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return ""
	}
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}
