package main

import (
	"errors"
	"os"
	"testing"
)

// Tests must never reach a live Herdr server or start phone questions; tests
// that need either set it up themselves.
func TestMain(m *testing.M) {
	moshiProgram = func() (string, error) { return "", errors.New("tests set moshiProgram") }
	os.Unsetenv("HERDR_ENV")
	os.Exit(m.Run())
}
