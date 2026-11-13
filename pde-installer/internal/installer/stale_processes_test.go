package installer

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
	"pde-installer/internal/testfile"
)

// Installs swap in a new hub by renaming, and the watch kept running the old,
// deleted hub, so the hub board showed an old build after five installs in two
// days. An install that leaves the hub alone must not restart the watch.
func TestHubWatchRestartsForReplacedHub(t *testing.T) {
	tests := []struct {
		name         string
		giveReplaced bool
		wantRestart  bool
	}{
		{
			name:         "hub replaced",
			giveReplaced: true,
			wantRestart:  true,
		},
		{
			name:         "hub unchanged",
			giveReplaced: false,
			wantRestart:  false,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			hub := filepath.Join(
				home,
				".local",
				"bin",
				"hub",
			)
			watchPID := startTestProcess(
				t,
				hub,
			)
			if tt.giveReplaced {
				removeTestFile(
					t,
					hub,
				)
			}
			calls := useFakeSystemctl(
				t,
				watchPID,
			)

			refreshStaleProcesses(
				home,
				run.Runner{
					Stdout: io.Discard,
					Stderr: io.Discard,
				},
			)

			restarted := strings.Contains(
				readFileOrEmpty(
					t,
					calls,
				),
				"--user try-restart hub-watch.service\n",
			)
			if restarted != tt.wantRestart {
				t.Fatalf(
					"restarted = %v, want %v",
					restarted,
					tt.wantRestart,
				)
			}
		})
	}
}

// Any tool still running a replaced binary keeps the old build's behavior
// with no sign of it. The owner needs the PID to know what to restart.
func TestStaleProcessGetsRestartWarning(t *testing.T) {
	home := t.TempDir()
	vibe := filepath.Join(
		home,
		".local",
		"bin",
		"vibe",
	)
	pid := startTestProcess(
		t,
		vibe,
	)
	removeTestFile(
		t,
		vibe,
	)
	useFakeSystemctl(
		t,
		0,
	)
	var stderr bytes.Buffer

	refreshStaleProcesses(
		home,
		run.Runner{
			Stdout: io.Discard,
			Stderr: &stderr,
		},
	)

	want := fmt.Sprintf(
		"warning: PID %d (vibe) runs a deleted binary from HOME; restart it to use the installed build\n",
		pid,
	)
	if stderr.String() != want {
		t.Fatalf(
			"stderr = %q, want %q",
			stderr.String(),
			want,
		)
	}
}

// Starts a copy of sleep at path, standing in for a tool installed in HOME,
// and returns its PID. The process ends with the test.
func startTestProcess(t *testing.T, path string) int {
	t.Helper()
	sleep, err := exec.LookPath("sleep")
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(sleep)
	if err != nil {
		t.Fatal(err)
	}
	testfile.WriteExecutable(
		t,
		path,
		string(data),
	)
	process := exec.Command(
		path,
		"60",
	)
	if err := process.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = process.Process.Kill()
		_ = process.Wait()
	})
	return process.Process.Pid
}

// Deletes path, as an install does to the binary it replaces.
func removeTestFile(t *testing.T, path string) {
	t.Helper()
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
}

// Puts a systemctl first on PATH that reports watchPID as hub-watch's main
// process, and returns the file that logs its calls.
func useFakeSystemctl(t *testing.T, watchPID int) string {
	t.Helper()
	bin := t.TempDir()
	calls := filepath.Join(
		bin,
		"calls",
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			bin,
			"systemctl",
		),
		fmt.Sprintf(
			"#!/bin/sh\nprintf '%%s\\n' \"$*\" >>'%s'\nif [ \"$2\" = show ]; then echo %d; fi\n",
			calls,
			watchPID,
		),
	)
	t.Setenv(
		"PATH",
		bin+string(os.PathListSeparator)+os.Getenv("PATH"),
	)
	return calls
}

// Returns the file's text, or "" when the file does not exist.
func readFileOrEmpty(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return ""
	}
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}
