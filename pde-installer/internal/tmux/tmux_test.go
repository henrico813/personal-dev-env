package tmux

import (
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"pde-installer/internal/run"
)

func TestReconcileManagesStaticBinary(t *testing.T) {
	home := t.TempDir()
	archive := compressedBinary(t, "#!/bin/sh\nprintf 'tmux 3.7b\\n'\n")
	digest := sha256.Sum256(archive)
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		_, _ = response.Write(archive)
	}))
	defer server.Close()

	manager := New(home, run.Runner{})
	manager.archiveURL = server.URL
	manager.archiveSHA256 = hex.EncodeToString(digest[:])
	writeFile(t, filepath.Join(manager.ReleaseRoot(), "old"), "old release\n", 0o644)
	launcher := filepath.Join(home, ".local", "bin", "tmux")
	writeFile(t, launcher, "old launcher\n", 0o755)

	journal, err := manager.Reconcile()
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	installed, status, err := manager.Probe()
	if err != nil || installed != version || status != "current" {
		t.Fatalf("Probe() = %q, %q, %v", installed, status, err)
	}
	unchanged, err := manager.Reconcile()
	if err != nil {
		t.Fatalf("second Reconcile() error = %v", err)
	}
	if len(unchanged.Changes) != 0 {
		t.Fatalf("second Reconcile() changes = %d, want 0", len(unchanged.Changes))
	}
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	assertFile(t, filepath.Join(manager.ReleaseRoot(), "old"), "old release\n")
	assertFile(t, launcher, "old launcher\n")
}

func compressedBinary(t *testing.T, content string) []byte {
	t.Helper()
	var archive bytes.Buffer
	compressed := gzip.NewWriter(&archive)
	if _, err := compressed.Write([]byte(content)); err != nil {
		t.Fatal(err)
	}
	if err := compressed.Close(); err != nil {
		t.Fatal(err)
	}
	return archive.Bytes()
}

func writeFile(t *testing.T, path, content string, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), mode); err != nil {
		t.Fatal(err)
	}
}

func assertFile(t *testing.T, path, want string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != want {
		t.Fatalf("%s = %q, want %q", path, data, want)
	}
}
