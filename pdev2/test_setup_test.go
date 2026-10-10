package main

import (
	"errors"
	"os"
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
	os.Exit(m.Run())
}
