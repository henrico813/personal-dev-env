package app

import (
	"context"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"syscall"
	"testing"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
)

// A failed refresh must keep the saved list and the list on screen.
// Otherwise the list empties while GitHub is down.
func TestRefreshListKeepsListOnFailure(t *testing.T) {
	saved := cache.List{
		FetchedAt: time.Now().Add(-5 * time.Minute),
		PullRequests: []github.PullRequest{
			listedPR,
		},
	}
	r, store, _ := newTestRefresher(
		t,
		saved,
	)
	useGH(
		t,
		"fail",
	)

	kept := r.refreshList(
		context.Background(),
		saved,
	)

	stored, err := store.ReadList()
	if err != nil || !kept.FetchedAt.Equal(saved.FetchedAt) || !stored.FetchedAt.Equal(saved.FetchedAt) ||
		len(stored.PullRequests) != 1 {
		t.Errorf(
			"after the failed refresh the saved list is from %v with %d pull requests (%v), want the one from %v",
			stored.FetchedAt,
			len(stored.PullRequests),
			err,
			saved.FetchedAt,
		)
	}
}

// A failed refresh must show the list's age in the prompt and gh's reason
// in the side pane, even when the list is empty, until a refresh that works
// reloads the rows and clears the age and the reason. Otherwise the list
// looks up to date while GitHub is down, or the error stays.
func TestRefreshListShowsFailureUntilSuccess(t *testing.T) {
	saved := cache.List{
		FetchedAt: time.Now().Add(-5 * time.Minute),
		PullRequests: []github.PullRequest{
			listedPR,
		},
	}
	r, store, sent := newTestRefresher(
		t,
		saved,
	)
	a := &prsApp{
		self:   "prs",
		stdout: io.Discard,
		now:    time.Now(),
		store:  store,
	}

	// A refresh fails.
	useGH(
		t,
		"fail",
	)
	r.refreshList(
		context.Background(),
		saved,
	)
	failedPrompt := buildListPrompt(
		r.dir,
		time.Now(),
	)
	failedDetails := printDetailsFor(
		t,
		a,
		r.dir,
		listedRow,
	)
	failedEmpty := printDetailsFor(
		t,
		a,
		r.dir,
		"",
	)
	// The next refresh works.
	useGH(
		t,
		"works",
	)
	r.refreshList(
		context.Background(),
		saved,
	)
	freshPrompt := buildListPrompt(
		r.dir,
		time.Now(),
	)
	freshEmpty := printDetailsFor(
		t,
		a,
		r.dir,
		"",
	)

	if failedPrompt != "prs updated 5m ago " || freshPrompt != "prs " {
		t.Errorf(
			"prompt is %q after the failure and %q after the success, want the age and then none",
			failedPrompt,
			freshPrompt,
		)
	}
	for name, details := range map[string]string{
		"selected row": failedDetails,
		"empty list":   failedEmpty,
	} {
		if !strings.Contains(
			details,
			"Could not refresh the list.\n"+failedReason,
		) {
			t.Errorf(
				"side pane on the %s omits gh's reason after the failure:\n%s",
				name,
				details,
			)
		}
	}
	if !strings.Contains(
		failedDetails,
		listedPR.Title,
	) || freshEmpty != "" {
		t.Errorf(
			"side pane shows\n%s\nafter the failure and %q after the success, want the details and then nothing",
			failedDetails,
			freshEmpty,
		)
	}
	want := []string{
		"transform(prs _prompt " + r.dir + ")+transform(prs _layout " + r.dir + " result)+refresh-preview",
		"reload(prs _rows)+refresh-preview",
	}
	if !slices.Equal(
		sent(),
		want,
	) {
		t.Errorf(
			"refreshes sent\n%q\nwant\n%q",
			sent(),
			want,
		)
	}
}

// While every background fetch waits on GitHub, the refresher must still
// start the list query on r, and must stop when its context ends. Otherwise
// r looks broken for as long as the fetch takes, or prs hangs at quit.
func TestRefresherRunStaysResponsiveDuringFetch(t *testing.T) {
	// Twice as many pull requests as workers, so some wait in the queue.
	var prs []github.PullRequest
	for number := 1; number <= 2*fetchWorkers; number++ {
		pr := listedPR
		pr.Number = number
		prs = append(
			prs,
			pr,
		)
	}
	r, _, _ := newTestRefresher(
		t,
		cache.List{PullRequests: prs},
	)
	ghLog := filepath.Join(
		t.TempDir(),
		"gh-log",
	)
	useGH(
		t,
		"slow-fetch",
	)
	t.Setenv(
		"GH_LOG",
		ghLog,
	)
	ctx, cancel := context.WithCancel(context.Background())
	t.Cleanup(cancel)
	requests := make(
		chan os.Signal,
		1,
	)
	ended := make(chan struct{})
	// The refresher starts fetching the pull requests, and every fetch hangs.
	go func() {
		defer close(ended)
		r.run(
			ctx,
			cache.List{PullRequests: prs},
			false,
			requests,
		)
	}()
	if !waitForLog(
		t,
		ghLog,
		"fetch",
		1,
	) {
		t.Fatal("the refresher started no fetch")
	}

	// The send stands in for the SIGUSR1 that r sends.
	requests <- syscall.SIGUSR1
	listed := waitForLog(
		t,
		ghLog,
		"list",
		1,
	)
	// prs quits, which ends the refresher's context.
	cancel()
	stopped := waitForStop(ended)

	if !listed {
		t.Error("r did not start the list query while the fetch was running")
	}
	if !stopped {
		t.Error("the refresher did not stop after its context ended")
	}
}
