package main

import (
	"errors"
	"os"
	"strings"
	"testing"
)

// Tests must never start copies of the test binary or reach a live Herdr
// server; tests that need either set it up themselves.
func TestMain(m *testing.M) {
	runnerStart = func(rec record) record { return rec }
	moshiProgram = func() (string, error) { return "", errors.New("tests set moshiProgram") }
	os.Unsetenv("HERDR_ENV")
	os.Unsetenv("OPENCODE_SESSION_ID")
	os.Exit(m.Run())
}

// discardStderr hides this program's messages for one test.
func discardStderr(t *testing.T) {
	t.Helper()
	old := stderr
	stderr = &strings.Builder{}
	t.Cleanup(func() { stderr = old })
}
