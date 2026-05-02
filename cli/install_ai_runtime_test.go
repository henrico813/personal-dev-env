package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestVerifyPlannerLauncherRunsHelp(t *testing.T) {
	localBin := filepath.Join(t.TempDir(), ".local", "bin")
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		t.Fatalf("mkdir local bin: %v", err)
	}

	plannerPath := filepath.Join(localBin, "planner")
	script := `#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" != "help" ]]; then
	echo "unexpected args: $*" >&2
	exit 1
fi
exit 0
`
	if err := os.WriteFile(plannerPath, []byte(script), 0o755); err != nil {
		t.Fatalf("write planner stub: %v", err)
	}

	cfg := &Config{LocalBinDir: localBin}
	if err := verifyPlannerLauncher(cfg, Runner{}); err != nil {
		t.Fatalf("verify planner launcher: %v", err)
	}
}
