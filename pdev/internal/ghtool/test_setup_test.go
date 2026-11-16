package ghtool

import (
	"errors"
	"io"
	"os"
	"strings"
	"testing"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
)

// The failing approveProgram keeps go test from reaching your Herdr or phone
// through pde-pr-approve, and the no-op runnerStart from starting the test
// binary as a runner. A test that needs either sets up a fake itself.
func TestMain(m *testing.M) {
	runnerStart = func(rec approval.Record) approval.Record { return rec }
	approveProgram = func() (string, error) { return "", errors.New("tests set approveProgram") }
	os.Unsetenv("HERDR_ENV")
	os.Unsetenv("OPENCODE_SESSION_ID")
	os.Exit(runOutsideCheckout(m))
}

// Tests run from a temp dir, outside this checkout, so its own ci/scan never
// checks test text. Returns the tests' exit code.
func runOutsideCheckout(m *testing.M) int {
	dir, err := os.MkdirTemp("", "pdev-ghtool-test")
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
