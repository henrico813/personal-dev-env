package main

import (
	"errors"
	"os"
	"strings"
	"testing"
)

// Running go test inside Herdr must not open popups in your Herdr or send
// questions to your phone while you work. Unsetting HERDR_ENV keeps
// notifyRequest from contacting Herdr, and the failing moshiProgram keeps
// startMoshi from starting a phone question. A test that needs either sets up
// a fake itself, as TestHerdrPopupUsesSocketRequest does.
func TestMain(m *testing.M) {
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
