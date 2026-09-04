package run

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Backends use exit codes to distinguish absence from failure.
func TestQueryPreservesExitStatus(t *testing.T) {
	t.Parallel()
	binary := writeCommand(t, "#!/bin/sh\nexit 7\n")
	_, err := (Runner{}).Query("run fixture", Command{Name: binary})
	var exitError *exec.ExitError
	if !errors.As(err, &exitError) {
		t.Fatalf("Query() error = %T %v", err, err)
	}
	if exitError.ExitCode() != 7 {
		t.Fatalf("exit code = %d, want 7", exitError.ExitCode())
	}
}

// Managed tools must receive the installer's explicit environment.
func TestRunUsesCommandEnvironment(t *testing.T) {
	t.Parallel()
	binary := writeCommand(t, "#!/bin/sh\nprintf '%s:%s' \"$PDE_TEST\" \"$1\"\n")
	output, err := (Runner{}).Query("run fixture", Command{
		Name: binary,
		Args: []string{"argument"},
		Env:  []string{"PDE_TEST=value"},
	})
	if err != nil {
		t.Fatal(err)
	}
	if got := string(output); got != "value:argument" {
		t.Fatalf("output = %q", got)
	}
}

// Dry-run must describe changes without executing commands.
func TestDryRunSkipsExecution(t *testing.T) {
	t.Parallel()
	root := t.TempDir()
	marker := filepath.Join(root, "marker")
	binary := writeCommand(t, "#!/bin/sh\ntouch \"$1\"\n")
	var output strings.Builder
	runner := Runner{DryRun: true, Stdout: &output}
	if err := runner.Run("create marker", Command{Name: binary, Args: []string{marker}}); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(marker); !os.IsNotExist(err) {
		t.Fatalf("dry-run created marker: %v", err)
	}
	if !strings.Contains(output.String(), "DRY-RUN: create marker") {
		t.Fatalf("dry-run output = %q", output.String())
	}
}

func writeCommand(t *testing.T, contents string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "command")
	if err := os.WriteFile(path, []byte(contents), 0o755); err != nil {
		t.Fatal(err)
	}
	return path
}
