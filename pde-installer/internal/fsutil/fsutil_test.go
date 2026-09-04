package fsutil

import (
	"archive/zip"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Installer mutations must remain inside the user's home directory.
func TestGuardHomeRejectsPathEscapes(t *testing.T) {
	home := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(home, "escape")); err != nil {
		t.Fatal(err)
	}
	if err := GuardHome(home, filepath.Join(home, "escape", "file")); err == nil {
		t.Fatal("GuardHome accepted a path through an outside symlink")
	}
	if err := GuardHome(home, filepath.Join(home, "..", "file")); err == nil {
		t.Fatal("GuardHome accepted a parent path")
	}
}

// Server errors must never become installed artifacts.
func TestDownloadRejectsBadResponses(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		http.Error(response, "no", http.StatusBadGateway)
	}))
	defer server.Close()
	destination := filepath.Join(t.TempDir(), "download")
	if err := Download(server.URL, destination, strings.Repeat("0", 64)); err == nil {
		t.Fatal("Download accepted an error response")
	}
	if _, err := os.Stat(destination); !os.IsNotExist(err) {
		t.Fatalf("destination exists after failed download: %v", err)
	}
}

// Corrupt downloads must not remain on disk.
func TestDownloadRejectsBadChecksums(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		_, _ = io.WriteString(response, "content")
	}))
	defer server.Close()
	destination := filepath.Join(t.TempDir(), "download")
	if err := Download(server.URL, destination, strings.Repeat("0", 64)); err == nil {
		t.Fatal("Download accepted a bad checksum")
	}
	if _, err := os.Stat(destination); !os.IsNotExist(err) {
		t.Fatalf("destination exists after checksum failure: %v", err)
	}
}

// Archive entries must not write outside the extraction directory.
func TestExtractZipRejectsPathTraversal(t *testing.T) {
	root := t.TempDir()
	archive := filepath.Join(root, "archive.zip")
	file, err := os.Create(archive)
	if err != nil {
		t.Fatal(err)
	}
	writer := zip.NewWriter(file)
	entry, err := writer.Create("../outside")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := io.WriteString(entry, "bad"); err != nil {
		t.Fatal(err)
	}
	if err := writer.Close(); err != nil {
		t.Fatal(err)
	}
	if err := file.Close(); err != nil {
		t.Fatal(err)
	}
	if err := ExtractZip(archive, filepath.Join(root, "output")); err == nil {
		t.Fatal("ExtractZip accepted path traversal")
	}
	if _, err := os.Stat(filepath.Join(root, "outside")); !os.IsNotExist(err) {
		t.Fatalf("archive wrote outside destination: %v", err)
	}
}

// A failed activation must restore the previous destination.
func TestJournalRestoresActivation(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	if err := journal.Rollback(); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "old")
	assertNoJournals(t, config.Directory)
}

// Recorded changes must preserve the user's previous file.
func TestJournalRestoresRecordedChanges(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	backup := filepath.Join(home, "backup")
	writeTestFile(t, destination, "old")
	if err := CopyPath(destination, backup); err != nil {
		t.Fatal(err)
	}
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.RecordReplaced(destination, backup); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(destination, []byte("new"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := journal.Rollback(); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "old")
	assertNoJournals(t, config.Directory)
}

// Successful changes must remove backups and temporary files.
func TestJournalRemovesCommittedCleanup(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	cleanup := filepath.Join(home, "cleanup")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	writeTestFile(t, filepath.Join(cleanup, "file"), "temporary")
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	if err := journal.AddCleanup(cleanup); err != nil {
		t.Fatal(err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "new")
	if _, err := os.Stat(cleanup); !os.IsNotExist(err) {
		t.Fatalf("cleanup path remains: %v", err)
	}
	assertNoJournals(t, config.Directory)
}

// Recovery must handle interruption before activation starts.
func TestJournalRecoversBeforeActivation(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	journal := interruptedJournal(t, config, stage, destination)
	if err := journal.persist(); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "old")
	if _, err := os.Stat(stage); !os.IsNotExist(err) {
		t.Fatalf("stage remains: %v", err)
	}
	assertNoJournals(t, config.Directory)
}

// Recovery must handle interruption during an atomic exchange.
func TestJournalRecoversDuringActivation(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	journal := interruptedJournal(t, config, stage, destination)
	if err := journal.persist(); err != nil {
		t.Fatal(err)
	}
	if _, err := Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "old")
	assertNoJournals(t, config.Directory)
}

// Recovery must restore changes when the process exits early.
func TestJournalRecoversAfterActivation(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "old")
	assertNoJournals(t, config.Directory)
}

// Recovery must safely finish a rollback started earlier.
func TestJournalResumesInterruptedRollback(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	if err := Rollback(destination, stage); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "old")
	assertNoJournals(t, config.Directory)
}

// Recovery must finish cleanup after a successful commit.
func TestJournalFinishesCommittedCleanup(t *testing.T) {
	home, config := journalTestConfig(t)
	destination := filepath.Join(home, "destination")
	stage := filepath.Join(home, "stage")
	cleanup := filepath.Join(home, "cleanup")
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	writeTestFile(t, filepath.Join(cleanup, "file"), "temporary")
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	if err := journal.AddCleanup(cleanup); err != nil {
		t.Fatal(err)
	}
	journal.state.Committed = true
	if err := journal.persist(); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, destination, "new")
	if _, err := os.Stat(cleanup); !os.IsNotExist(err) {
		t.Fatalf("cleanup path remains: %v", err)
	}
	assertNoJournals(t, config.Directory)
}

func TestJournalsCommitTogether(t *testing.T) {
	home, config := journalTestConfig(t)
	first := activateTestJournal(t, config, filepath.Join(home, "first"))
	second := activateTestJournal(t, config, filepath.Join(home, "second"))
	if err := CommitJournals(first, &Journal{}, second); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, filepath.Join(home, "first"), "new")
	assertTestFile(t, filepath.Join(home, "second"), "new")
	assertNoJournals(t, config.Directory)
}

func TestJournalGroupRecoveryCommitsAll(t *testing.T) {
	for _, test := range []struct {
		name   string
		marked int
	}{
		{name: "none", marked: 0},
		{name: "partial", marked: 1},
		{name: "all", marked: 2},
	} {
		t.Run(test.name, func(t *testing.T) {
			home, config := journalTestConfig(t)
			journals := []*Journal{
				activateTestJournal(t, config, filepath.Join(home, "first")),
				activateTestJournal(t, config, filepath.Join(home, "second")),
			}
			if _, err := writeCommitGroup(journals); err != nil {
				t.Fatal(err)
			}
			for _, journal := range journals[:test.marked] {
				if err := journal.markCommitted(); err != nil {
					t.Fatal(err)
				}
			}
			if err := RecoverJournals(config); err != nil {
				t.Fatal(err)
			}
			assertTestFile(t, filepath.Join(home, "first"), "new")
			assertTestFile(t, filepath.Join(home, "second"), "new")
			assertNoJournals(t, config.Directory)
		})
	}
}

func TestJournalGroupResumesCleanup(t *testing.T) {
	home, config := journalTestConfig(t)
	first := activateTestJournal(t, config, filepath.Join(home, "first"))
	second := activateTestJournal(t, config, filepath.Join(home, "second"))
	groupPath, err := writeCommitGroup([]*Journal{first, second})
	if err != nil {
		t.Fatal(err)
	}
	for _, journal := range []*Journal{first, second} {
		if err := journal.markCommitted(); err != nil {
			t.Fatal(err)
		}
	}
	if err := removeDurable(groupPath); err != nil {
		t.Fatal(err)
	}
	if err := first.finishCommit(); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, filepath.Join(home, "first"), "new")
	assertTestFile(t, filepath.Join(home, "second"), "new")
	assertNoJournals(t, config.Directory)
}

func TestJournalRecoveryRollsAllBack(t *testing.T) {
	home, config := journalTestConfig(t)
	activateTestJournal(t, config, filepath.Join(home, "first"))
	activateTestJournal(t, config, filepath.Join(home, "second"))
	if err := RecoverJournals(config); err != nil {
		t.Fatal(err)
	}
	assertTestFile(t, filepath.Join(home, "first"), "old")
	assertTestFile(t, filepath.Join(home, "second"), "old")
	assertNoJournals(t, config.Directory)
}

func TestJournalsIgnoreEmptyEntries(t *testing.T) {
	if err := CommitJournals(&Journal{}, &Journal{}); err != nil {
		t.Fatal(err)
	}
}

func TestJournalRejectsUnsafeCommitGroup(t *testing.T) {
	home, config := journalTestConfig(t)
	if err := os.MkdirAll(config.Directory, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(config.Directory, commitGroupPrefix+"unsafe"+commitGroupSuffix)
	state := commitGroupState{Version: journalVersion, Home: home, Journals: []string{"../outside.json"}}
	data, err := json.Marshal(state)
	if err != nil {
		t.Fatal(err)
	}
	if err := atomicWriteJSON(path, append(data, '\n')); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err == nil {
		t.Fatal("recovery accepted an unsafe commit group")
	}
}

// Stored journal data must never bypass HOME protection.
func TestJournalRejectsOutsidePaths(t *testing.T) {
	home, config := journalTestConfig(t)
	if err := os.MkdirAll(config.Directory, 0o700); err != nil {
		t.Fatal(err)
	}
	state := journalState{
		Version: journalVersion,
		Home:    home,
		Changes: []journalChange{{Change: Change{Destination: filepath.Join(home, "..", "outside")}}},
	}
	journal := &Journal{Home: home, directory: config.Directory, path: filepath.Join(config.Directory, "unsafe.json"), state: state}
	if err := journal.persist(); err != nil {
		t.Fatal(err)
	}
	if err := RecoverJournals(config); err == nil {
		t.Fatal("recovery accepted an outside path")
	}
}

func journalTestConfig(t *testing.T) (string, JournalConfig) {
	t.Helper()
	home := t.TempDir()
	config := JournalConfig{Home: home, Directory: filepath.Join(home, "journals")}
	return home, config
}

func interruptedJournal(t *testing.T, config JournalConfig, stage, destination string) *Journal {
	t.Helper()
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.ready(); err != nil {
		t.Fatal(err)
	}
	stageID, err := identifyPath(stage)
	if err != nil {
		t.Fatal(err)
	}
	originalID, err := identifyPath(destination)
	if err != nil {
		t.Fatal(err)
	}
	journal.state.Changes = append(journal.state.Changes, journalChange{
		Change:     Change{Destination: destination, Backup: stage, Existed: true},
		Stage:      stage,
		StageID:    stageID,
		OriginalID: originalID,
	})
	return journal
}

func activateTestJournal(t *testing.T, config JournalConfig, destination string) *Journal {
	t.Helper()
	stage := destination + ".stage"
	writeTestFile(t, destination, "old")
	writeTestFile(t, stage, "new")
	journal, err := NewJournal(config)
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Activate(stage, destination); err != nil {
		t.Fatal(err)
	}
	return journal
}

func writeTestFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func assertTestFile(t *testing.T, path, content string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != content {
		t.Fatalf("file content = %q, want %q", data, content)
	}
}

func assertNoJournals(t *testing.T, directory string) {
	t.Helper()
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if strings.HasSuffix(entry.Name(), ".json") {
			t.Fatalf("journal remains: %s", entry.Name())
		}
	}
}
