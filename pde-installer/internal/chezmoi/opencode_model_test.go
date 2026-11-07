package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

// An OpenCode that lists no models on its first call, as in a folder it has
// not opened yet. It writes the arguments of its api and TUI calls to files in $HOME.
const fakeOpenCodeWithSlowModelList = `#!/bin/sh
case "$1" in
models)
    if [ -e "$HOME/listed" ]; then
        printf '%s\n' claude-code/claude-sonnet-5
    fi
    touch "$HOME/listed"
    ;;
api)
    shift
    printf '%s\n' "$@" >"$HOME/api"
    printf '%s\n' '{"data":{"id":"ses_test"}}'
    ;;
*)
    printf '%s\n' "$@" >"$HOME/opencode"
    ;;
esac
`

// Herdr handoffs pass --model to start the new session on the model they name.
// If the session is not made on that model and opened, it runs the global default.
func TestLauncherOpensSessionOnChosenModel(t *testing.T) {
	home := t.TempDir()
	binary := filepath.Join(
		home,
		".local",
		"share",
		"pde",
		"npm",
		"node_modules",
		".bin",
		"opencode",
	)
	writeExecutable(t, binary, fakeOpenCodeWithSlowModelList)
	launcher := writeMemcapLauncher(t, home)
	command := exec.Command(
		launcher,
		"--model",
		"claude-code/claude-sonnet-5#high",
		"--auto",
	)
	command.Dir = home
	command.Env = append(
		os.Environ(),
		"HOME="+home,
		"OPENCODE_MEMCAP_SCOPE=",
		"PDE_OPENCODE_MEMORY_MAX=off",
	)

	output, err := command.CombinedOutput()

	if err != nil {
		t.Fatalf("launcher failed: %v\n%s", err, output)
	}
	wantBody := `{"model":{"providerID":"claude-code","id":"claude-sonnet-5","variant":"high"},"location":{"directory":"` + home + `"}}`
	wantRequest := "post\n/api/session\n--data\n" + wantBody + "\n"
	request := readMemcapCalls(
		t,
		filepath.Join(home, "api"),
	)
	if request != wantRequest {
		t.Errorf("session request = %q, want %q", request, wantRequest)
	}
	wantArgv := "--session\nses_test\n--auto\n"
	argv := readMemcapCalls(
		t,
		filepath.Join(home, "opencode"),
	)
	if argv != wantArgv {
		t.Errorf("OpenCode argv = %q, want %q", argv, wantArgv)
	}
}
