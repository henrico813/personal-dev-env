package cmd

import (
	"io"
	"os"
	"os/exec"
	"strings"
	"testing"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
)

// Unsetting HERDR_ENV keeps go test from opening popups in your Herdr, and
// the no-op runnerStart keeps approvals from starting pde-gh. A test that
// needs either sets up a fake, as TestHerdrPopupUsesSocketRequest does.
func TestMain(m *testing.M) {
	runnerStart = func(rec approval.Record) approval.Record { return rec }
	os.Unsetenv("HERDR_ENV")
	os.Unsetenv("OPENCODE_SESSION_ID")
	os.Exit(runOutsideCheckout(m))
}

// Tests run from a temp dir, outside this checkout, so its own ci/scan never
// checks test text. Returns the tests' exit code.
func runOutsideCheckout(m *testing.M) int {
	dir, err := os.MkdirTemp("", "pdev-cmd-test")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(dir)
	if err := os.Chdir(dir); err != nil {
		panic(err)
	}
	return m.Run()
}

// discardStderr hides this program's messages for one test.
func discardStderr(t *testing.T) {
	t.Helper()
	approvaltest.Replace[io.Writer](t, &stderr, &strings.Builder{})
}

// A fixed large PID can belong to a live process. A child that has exited and
// been waited for has a free PID, unless the kernel has already reused it.
func deadPID(t *testing.T) int {
	t.Helper()
	cmd := exec.Command("true")
	if err := cmd.Run(); err != nil {
		t.Fatal(err)
	}
	return cmd.Process.Pid
}
