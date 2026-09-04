package tmux

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
	"runtime"
	"strconv"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

func TestReconcileActivatesAndRollsBack(t *testing.T) {
	home := t.TempDir()
	commands := filepath.Join(t.TempDir(), "commands")
	makeBin := t.TempDir()
	writeFile(t, filepath.Join(makeBin, "make"), makeFixture(commands), 0o755)
	t.Setenv("PATH", makeBin+string(os.PathListSeparator)+os.Getenv("PATH"))

	archive := sourceArchive(t)
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
	installedVersion, status, err := manager.Probe()
	if err != nil || installedVersion != version || status != "current" {
		t.Fatalf("Probe() = %q, %q, %v", installedVersion, status, err)
	}
	data, err := os.ReadFile(commands)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), "make -j "+strconv.Itoa(runtime.NumCPU())+"\n") {
		t.Fatalf("commands missing parallel build:\n%s", data)
	}
	if strings.Contains(string(data), "sudo") {
		t.Fatalf("commands use sudo:\n%s", data)
	}
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	assertFile(t, filepath.Join(manager.ReleaseRoot(), "old"), "old release\n")
	assertFile(t, launcher, "old launcher\n")
}

func TestReconcileSkipsCurrentVersion(t *testing.T) {
	home := t.TempDir()
	manager := New(home, run.Runner{})
	binary := filepath.Join(manager.ReleaseRoot(), "bin", "tmux")
	writeFile(t, binary, "#!/bin/sh\nprintf '%s\\n' 'tmux "+version+"'\n", 0o755)
	launcher := filepath.Join(home, ".local", "bin", "tmux")
	if err := os.MkdirAll(filepath.Dir(launcher), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(binary, launcher); err != nil {
		t.Fatal(err)
	}
	journal, err := manager.Reconcile()
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	if len(journal.Changes) != 0 {
		t.Fatalf("Reconcile() changes = %d, want 0", len(journal.Changes))
	}
}

func TestExtractRejectsUnsafeEntries(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name   string
		header tar.Header
	}{
		{name: "traversal", header: tar.Header{Name: "tmux-" + version + "/../../outside", Typeflag: tar.TypeReg}},
		{name: "absolute", header: tar.Header{Name: "/tmp/outside", Typeflag: tar.TypeReg}},
		{name: "symlink", header: tar.Header{Name: "tmux-" + version + "/link", Typeflag: tar.TypeSymlink, Linkname: "/tmp"}},
	} {
		test := test
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			archive := archiveWithHeader(t, test.header)
			path := filepath.Join(t.TempDir(), "archive.tar.gz")
			writeFile(t, path, string(archive), 0o644)
			if err := extractArchive(path, filepath.Join(t.TempDir(), "source"), "tmux-"+version); err == nil {
				t.Fatal("extractArchive() error = nil")
			}
		})
	}
}

func TestReconcileRejectsWrongChecksum(t *testing.T) {
	archive := sourceArchive(t)
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		_, _ = response.Write(archive)
	}))
	defer server.Close()
	manager := New(t.TempDir(), run.Runner{})
	manager.archiveURL = server.URL
	manager.archiveSHA256 = strings.Repeat("0", sha256.Size*2)
	if _, err := manager.Reconcile(); err == nil || !strings.Contains(err.Error(), "checksum mismatch") {
		t.Fatalf("Reconcile() error = %v", err)
	}
}

func sourceArchive(t *testing.T) []byte {
	t.Helper()
	configure := "#!/bin/sh\nset -eu\nprintf '%s\\n' \"${1#--prefix=}\" > .prefix\n"
	header := tar.Header{Name: "tmux-" + version + "/configure", Mode: 0o755, Size: int64(len(configure)), Typeflag: tar.TypeReg}
	return archiveEntries(t, []tarEntry{{header: header, content: configure}})
}

type tarEntry struct {
	header  tar.Header
	content string
}

func archiveWithHeader(t *testing.T, header tar.Header) []byte {
	t.Helper()
	return archiveEntries(t, []tarEntry{{header: header}})
}

func archiveEntries(t *testing.T, entries []tarEntry) []byte {
	t.Helper()
	var compressed bytes.Buffer
	zipper := gzip.NewWriter(&compressed)
	archive := tar.NewWriter(zipper)
	for _, entry := range entries {
		if err := archive.WriteHeader(&entry.header); err != nil {
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

func makeFixture(log string) string {
	return "#!/bin/sh\nset -eu\nprintf 'make %s\\n' \"$*\" >> " + shellQuote(log) + "\n" +
		"case \" $* \" in\n  *' install '*)\n    destdir=\n    for arg do case $arg in DESTDIR=*) destdir=${arg#DESTDIR=};; esac; done\n    IFS= read -r prefix < .prefix\n    mkdir -p \"$destdir$prefix/bin\"\n    printf '%s\\n' '#!/bin/sh' \"printf '%s\\\\n' 'tmux " + version + "'\" > \"$destdir$prefix/bin/tmux\"\n    chmod +x \"$destdir$prefix/bin/tmux\"\n  ;;\nesac\n"
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

func shellQuote(value string) string {
	return "'" + value + "'"
}
