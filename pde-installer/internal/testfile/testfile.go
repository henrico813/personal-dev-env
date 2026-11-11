// Package testfile writes fake programs that tests run while other tests start processes.
package testfile

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

// Writes content to path as a program the test can run, creating its parent folders.
func WriteExecutable(t testing.TB, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	// Holding the fork lock keeps a process started by another test from inheriting the open file, which would make running it fail with text file busy (https://go.dev/issue/22315).
	syscall.ForkLock.RLock()
	defer syscall.ForkLock.RUnlock()
	if err := os.WriteFile(path, []byte(content), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o755); err != nil {
		t.Fatal(err)
	}
}
