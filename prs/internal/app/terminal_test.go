package app

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
)

// Opens the diff screen from the PR list screen, as d does, and then
// waits in the file picker until a signal stops it.
const hangingFzf = `#!/bin/sh
case "$*" in
*" files "*) touch "$READY"; exec sleep 30 ;;
*) "$PRS" _diff example/repo 12 ;;
esac
`

// Exits 111, the character code of o, so the diff view opens the file
// picker as o does.
const pickerLess = `#!/bin/sh
exit 111
`

// A dropped SSH connection, a TERM or Ctrl-C reaches every prs process
// while the diff screen is open and the list refresh is still running, or
// while the first list is still being fetched. Each process must remove its
// temporary folder, which holds the diff, and stop its gh call, or gh keeps
// running after prs is gone.
func TestRunCleansUpWhenStopped(t *testing.T) {
	tests := []struct {
		name   string
		signal syscall.Signal
		saved  bool
	}{
		{
			name:   "hang-up",
			signal: syscall.SIGHUP,
			saved:  true,
		},
		{
			name:   "TERM",
			signal: syscall.SIGTERM,
			saved:  true,
		},
		{
			name:   "Ctrl-C",
			signal: syscall.SIGINT,
			saved:  true,
		},
		{
			name:   "hang-up during the first fetch",
			signal: syscall.SIGHUP,
			saved:  false,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			bin := writeScripts(
				t,
				map[string]string{
					"gh":   fakeGH,
					"fzf":  hangingFzf,
					"less": pickerLess,
				},
			)
			temp := t.TempDir()
			ready := filepath.Join(
				bin,
				"ready",
			)
			ghPID := filepath.Join(
				bin,
				"gh-pid",
			)
			// A saved list makes the PR list screen open at once and refresh
			// in the background, and without one prs fetches before fzf.
			t.Setenv(
				"XDG_CACHE_HOME",
				t.TempDir(),
			)
			if tt.saved {
				saveList(
					t,
					cache.List{
						FetchedAt: time.Now(),
						PullRequests: []github.PullRequest{
							listedPR,
						},
					},
				)
			}
			// prs starts with every gh call hanging.
			prs, exited := startPRs(
				t,
				"PRS="+os.Args[0],
				"READY="+ready,
				"GH_ANSWER=slow",
				"GH_PID="+ghPID,
				"TMPDIR="+temp,
				"PATH="+bin+":"+os.Getenv("PATH"),
			)
			// The file picker opens when there is a saved list.
			if tt.saved {
				waitForFile(
					t,
					ready,
				)
			}
			// The slow gh has started, for the refresh or the first fetch.
			waitForFile(
				t,
				ghPID,
			)

			// The signal reaches every process, as a closed terminal's does.
			if err := syscall.Kill(
				-prs.Process.Pid,
				tt.signal,
			); err != nil {
				t.Fatal(err)
			}
			waitForExit(
				t,
				exited,
			)

			// The diff screen's prs runs under fzf, not as this test's child,
			// so it may still be removing its folder.
			left := waitForEmptyDir(
				t,
				temp,
			)
			if left > 0 {
				t.Errorf(
					"prs left %d temporary folders",
					left,
				)
			}
			// The slow gh ignores the signal, so only prs can have ended it.
			if pid := findRunningFakeGH(
				t,
				ghPID,
			); pid != 0 {
				_ = syscall.Kill(
					pid,
					syscall.SIGKILL,
				)
				t.Errorf(
					"gh %d still runs after prs stopped",
					pid,
				)
			}
		})
	}
}
