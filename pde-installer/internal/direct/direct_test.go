package direct

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

// A blocked font path is broken rather than missing.
func TestFontProbeReturnsFilesystemErrors(t *testing.T) {
	t.Parallel()
	home := t.TempDir()
	blockingPath := filepath.Join(home, ".local", "share", "fonts")
	if err := os.MkdirAll(filepath.Dir(blockingPath), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blockingPath, []byte("blocked"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err := New(home, t.TempDir(), run.Runner{}).Probe(Fonts()[0])
	if err == nil {
		t.Fatal("Probe() error = nil")
	}
}

func TestToolsMapSupportedArchitectures(t *testing.T) {
	t.Parallel()
	for _, architecture := range []string{"amd64", "arm64"} {
		tools, err := toolsForPlatform("linux", architecture)
		if err != nil {
			t.Fatalf("toolsForPlatform(%q) error = %v", architecture, err)
		}
		if len(tools) != 5 {
			t.Fatalf("toolsForPlatform(%q) count = %d", architecture, len(tools))
		}
		for _, tool := range tools {
			if tool.Version == "" || len(tool.SHA256) != sha256.Size*2 {
				t.Errorf("%s metadata is incomplete", tool.Name)
			}
		}
	}
}

func TestToolsRejectUnsupportedPlatforms(t *testing.T) {
	t.Parallel()
	for _, platform := range [][2]string{{"darwin", "amd64"}, {"linux", "386"}} {
		if _, err := toolsForPlatform(platform[0], platform[1]); err == nil {
			t.Errorf("toolsForPlatform(%q, %q) error = nil", platform[0], platform[1])
		}
	}
}

func TestToolVersionsRequireBoundaries(t *testing.T) {
	t.Parallel()
	tool := Tool{Version: "1.2.3", VersionPrefix: "tool "}
	for output, want := range map[string]bool{
		"tool 1.2.3":       true,
		"tool 1.2.3 extra": true,
		"tool 1.2.30":      false,
		"other tool 1.2.3": false,
	} {
		if got := hasToolVersion(output, tool); got != want {
			t.Errorf("hasToolVersion(%q) = %t, want %t", output, got, want)
		}
	}
}

func TestToolsActivateAndRollBack(t *testing.T) {
	home := t.TempDir()
	archive := toolArchive(t, "fixture-1.2.3", "fixture", "1.2.3")
	digest := sha256.Sum256(archive)
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		_, _ = response.Write(archive)
	}))
	defer server.Close()

	tool := Tool{
		Name: "fixture", Version: "1.2.3", Archive: "fixture.tar.gz",
		URL: server.URL, SHA256: hex.EncodeToString(digest[:]), Directory: "fixture",
		Binary: "fixture", VersionPrefix: "fixture ", Links: []string{"fixture"},
		VersionArgs: []string{"--version"}, Kind: archiveTool,
	}
	manager := New(home, t.TempDir(), run.Runner{})
	oldRoot := filepath.Join(manager.ToolsRoot(), "old")
	oldLauncher := filepath.Join(home, ".local", "bin", "fixture")
	writeDirectFile(t, oldRoot, "old root\n", 0o644)
	writeDirectFile(t, oldLauncher, "old launcher\n", 0o755)

	journal, err := manager.reconcileTools([]Tool{tool})
	if err != nil {
		t.Fatalf("reconcileTools() error = %v", err)
	}
	output, err := manager.Runner.Query("read fixture", run.Command{Name: oldLauncher, Args: []string{"--version"}})
	if err != nil || strings.TrimSpace(string(output)) != "fixture 1.2.3" {
		t.Fatalf("fixture output = %q, %v", output, err)
	}
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	assertDirectFile(t, oldRoot, "old root\n")
	assertDirectFile(t, oldLauncher, "old launcher\n")
}

func TestOfficialToolsInstall(t *testing.T) {
	if os.Getenv("PDE_TEST_OFFICIAL_TOOLS") != "1" {
		t.Skip("set PDE_TEST_OFFICIAL_TOOLS=1 to download release archives")
	}
	manager := New(t.TempDir(), t.TempDir(), run.Runner{})
	journal, err := manager.ReconcileTools()
	if err != nil {
		t.Fatalf("ReconcileTools() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	tools, err := Tools()
	if err != nil {
		t.Fatal(err)
	}
	for _, tool := range tools {
		installed, state, err := manager.ToolProbe(tool.Name)
		if err != nil || installed != tool.Version || state != "current" {
			t.Errorf("ToolProbe(%q) = %q, %q, %v", tool.Name, installed, state, err)
		}
	}
}

func toolArchive(t *testing.T, root, name, version string) []byte {
	t.Helper()
	var compressed bytes.Buffer
	zipper := gzip.NewWriter(&compressed)
	archive := tar.NewWriter(zipper)
	content := "#!/bin/sh\nprintf '%s\\n' '" + name + " " + version + "'\n"
	entries := []struct {
		name, content string
		mode          int64
	}{
		{name: root + "/bin/" + name, content: content, mode: 0o755},
	}
	for _, entry := range entries {
		if err := archive.WriteHeader(&tar.Header{Name: entry.name, Mode: entry.mode, Size: int64(len(entry.content))}); err != nil {
			t.Fatal(err)
		}
		if _, err := io.WriteString(archive, entry.content); err != nil {
			t.Fatal(err)
		}
	}
	if err := archive.Close(); err != nil {
		t.Fatal(err)
	}
	if err := zipper.Close(); err != nil {
		t.Fatal(err)
	}
	return compressed.Bytes()
}

func writeDirectFile(t *testing.T, path, content string, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), mode); err != nil {
		t.Fatal(err)
	}
}

func assertDirectFile(t *testing.T, path, want string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != want {
		t.Fatalf("%s = %q, want %q", path, data, want)
	}
}
