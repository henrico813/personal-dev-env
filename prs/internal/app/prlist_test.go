package app

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
)

// The side pane prints the status strip when it has 3 rows or fewer, and
// the longer details otherwise. A wrong choice cuts the checks off a phone
// with its keyboard up, or wastes the rows of a taller side pane.
func TestPrintDetailsFitsSidePaneRows(t *testing.T) {
	tests := []struct {
		name      string
		lines     string
		wantTitle bool
	}{
		{
			name:      "3 rows",
			lines:     "3",
			wantTitle: false,
		},
		{
			name:      "4 rows",
			lines:     "4",
			wantTitle: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			store := saveList(
				t,
				cache.List{
					PullRequests: []github.PullRequest{
						listedPR,
					},
				},
			)
			t.Setenv(
				"FZF_PREVIEW_LINES",
				tt.lines,
			)
			t.Setenv(
				"FZF_PREVIEW_COLUMNS",
				"50",
			)
			a := &prsApp{
				self:  "prs",
				now:   listedPR.UpdatedAt.Add(2 * time.Hour),
				store: store,
			}

			got := printDetailsFor(
				t,
				a,
				t.TempDir(),
				listedRow,
			)

			hasTitle := strings.Contains(
				got,
				listedPR.Title,
			)
			if !strings.HasPrefix(
				got,
				"#12 OPEN",
			) || hasTitle != tt.wantTitle {
				t.Errorf(
					"side pane shows the title %v, want %v:\n%s",
					hasTitle,
					tt.wantTitle,
					got,
				)
			}
		})
	}
}

// Waits in place of the PR list screen's fzf, after writing the path of
// its --listen socket to READY, and runs its r binding's command once the
// PRESS file exists.
const listeningFzf = `#!/bin/sh
for arg; do
  case "$arg" in
  --listen=*) echo "${arg#--listen=}" > "$READY.tmp" && mv "$READY.tmp" "$READY" ;;
  --bind=r:execute-silent\(*\)) r=${arg#--bind=r:execute-silent\(}; r=${r%\)} ;;
  esac
done
while [ ! -e "$PRESS" ]; do sleep 0.01; done
sh -c "$r"
exec sleep 30
`

// The PR list screen must refresh a saved list at start, and again on each
// press of r, whose binding sends prs a refresh request. Otherwise the list
// stays as old as the last run, and r does nothing.
func TestPickPullRequestRefreshesOnSignal(t *testing.T) {
	bin := writeScripts(
		t,
		map[string]string{
			"gh":  fakeGH,
			"fzf": listeningFzf,
		},
	)
	ready := filepath.Join(
		bin,
		"ready",
	)
	press := filepath.Join(
		bin,
		"press",
	)
	ghLog := filepath.Join(
		bin,
		"gh-log",
	)
	saveList(
		t,
		cache.List{
			FetchedAt: time.Now(),
			PullRequests: []github.PullRequest{
				listedPR,
			},
		},
	)
	// prs starts with the fake fzf as its PR list screen.
	startPRs(
		t,
		"READY="+ready,
		"PRESS="+press,
		"GH_ANSWER=works",
		"GH_LOG="+ghLog,
		"TMPDIR="+t.TempDir(),
		"PATH="+bin+":"+os.Getenv("PATH"),
	)
	// The fake fzf names the socket, so the test can stand in for fzf.
	waitForFile(
		t,
		ready,
	)
	socket, err := os.ReadFile(ready)
	if err != nil {
		t.Fatal(err)
	}
	listenAsFzf(
		t,
		filepath.Dir(strings.TrimSpace(string(socket))),
	)
	// The saved list refreshes at start, so a second list query can only
	// come from r.
	if !waitForLog(
		t,
		ghLog,
		"list",
		1,
	) {
		t.Fatal("prs did not refresh the saved list at start")
	}

	// The fake fzf runs the r binding's command, as a press of r does.
	writeFile(
		t,
		press,
		"",
		0o600,
	)
	refreshed := waitForLog(
		t,
		ghLog,
		"list",
		2,
	)

	if !refreshed {
		t.Error("r did not start a second list query")
	}
}
