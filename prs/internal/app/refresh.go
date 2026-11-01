package app

import (
	"context"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
)

// How many pull requests the refresher fetches at once.
const fetchWorkers = 4

// Keeps the PR list screen fresh from inside the main prs process. It
// refreshes the list, fetches each listed pull request's description,
// files, comments and diff into the cache, and sends fzf actions to fzf's
// socket.
type refresher struct {
	// This binary's path, quoted for the shell, which the actions run.
	self   string
	dir    string
	store  cache.Store
	client *http.Client
}

func newRefresher(
	self string,
	dir string,
	store cache.Store,
) *refresher {
	socket := filepath.Join(
		dir,
		socketFile,
	)
	return &refresher{
		self:  self,
		dir:   dir,
		store: store,
		client: &http.Client{
			Transport: &http.Transport{
				DialContext: func(
					ctx context.Context,
					_,
					_ string,
				) (
					net.Conn,
					error,
				) {
					var dialer net.Dialer
					return dialer.DialContext(
						ctx,
						"unix",
						socket,
					)
				},
			},
		},
	}
}

// Refreshes the list first when it came from the cache, then fetches what
// is missing, and refreshes again at each press of r until ctx ends. It
// returns once its workers and the prompt's ticker have stopped.
func (r *refresher) run(
	ctx context.Context,
	list cache.List,
	refreshFirst bool,
	requests <-chan os.Signal,
) {
	queue := make(chan github.PullRequest)
	var workers sync.WaitGroup
	for range fetchWorkers {
		workers.Add(1)
		go func() {
			defer workers.Done()
			for pr := range queue {
				r.fetchEntries(
					ctx,
					pr,
				)
			}
		}()
	}
	workers.Add(1)
	go func() {
		defer workers.Done()
		r.tickPrompt(ctx)
	}()
	defer workers.Wait()
	defer close(queue)
	if refreshFirst {
		list = r.refreshList(
			ctx,
			list,
		)
	}
	for {
		requested := r.queueMissing(
			ctx,
			list.PullRequests,
			queue,
			requests,
		)
		if !requested {
			select {
			case <-ctx.Done():
				return
			case <-requests:
			}
		}
		list = r.refreshList(
			ctx,
			list,
		)
	}
}

// Fetches the list, saves it and reloads fzf's rows in place. When fetching
// or saving fails it keeps the old list and leaves the error for the side
// pane.
func (r *refresher) refreshList(
	ctx context.Context,
	old cache.List,
) cache.List {
	r.writeState(
		"running",
		old.FetchedAt,
	)
	r.send(
		ctx,
		r.buildPromptAction(),
	)
	list, err := fetchList(ctx)
	if ctx.Err() != nil {
		return old
	}
	if err == nil {
		err = r.store.WriteList(list)
	}
	if err != nil {
		r.writeState(
			"failed",
			old.FetchedAt,
		)
		_ = os.WriteFile(
			filepath.Join(
				r.dir,
				refreshErrorFile,
			),
			[]byte(err.Error()+"\n"),
			0o600,
		)
		r.send(
			ctx,
			r.buildPromptAction()+"+transform("+r.self+" _layout "+quoteShell(r.dir)+" result)+refresh-preview",
		)
		return old
	}
	_ = os.Remove(filepath.Join(
		r.dir,
		refreshStateFile,
	))
	_ = os.Remove(filepath.Join(
		r.dir,
		refreshErrorFile,
	))
	// The side pane reads the saved list, so it can show new details as
	// soon as the list is written.
	r.send(
		ctx,
		"reload("+r.self+" _rows)+refresh-preview",
	)
	r.store.RemoveUnlisted(list.PullRequests)
	return list
}

// Sends each pull request whose cache entries are missing to the workers,
// newest first. Returns true when r is pressed before all are sent, so the
// list refreshes first.
func (r *refresher) queueMissing(
	ctx context.Context,
	prs []github.PullRequest,
	queue chan<- github.PullRequest,
	requests <-chan os.Signal,
) bool {
	for _, pr := range prs {
		if r.store.HasExtras(pr) && r.store.HasDiff(pr) {
			continue
		}
		select {
		case queue <- pr:
		case <-requests:
			return true
		case <-ctx.Done():
			return false
		}
	}
	return false
}

// Fetches and saves a pull request's missing cache entries. A failure
// leaves the entry missing, so the screen that needs it fetches it again.
func (r *refresher) fetchEntries(
	ctx context.Context,
	pr github.PullRequest,
) {
	if !r.store.HasExtras(pr) {
		if extras, err := github.FetchExtras(
			ctx,
			pr.Repo(),
			pr.Number,
		); err == nil {
			_ = r.store.WriteExtras(
				pr,
				extras,
			)
		}
	}
	if !r.store.HasDiff(pr) {
		if diff, err := github.FetchDiff(
			ctx,
			pr.Repo(),
			pr.Number,
		); err == nil {
			_ = r.store.WriteDiff(
				pr,
				diff,
			)
		}
	}
}

// Updates the prompt every second while it shows the list's age, so the
// age counts up.
func (r *refresher) tickPrompt(ctx context.Context) {
	ticker := time.NewTicker(time.Second)
	defer ticker.Stop()
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			if isPresent(filepath.Join(
				r.dir,
				refreshStateFile,
			)) {
				r.send(
					ctx,
					r.buildPromptAction(),
				)
			}
		}
	}
}

// Saves the refresher's state and the saved list's time, which the prompt
// reads.
func (r *refresher) writeState(
	state string,
	fetchedAt time.Time,
) {
	_ = os.WriteFile(
		filepath.Join(
			r.dir,
			refreshStateFile,
		),
		[]byte(state+"\n"+fetchedAt.Format(time.RFC3339Nano)+"\n"),
		0o600,
	)
}

func (r *refresher) buildPromptAction() string {
	return "transform(" + r.self + " _prompt " + quoteShell(r.dir) + ")"
}

// Sends fzf actions to fzf's socket. fzf may still be starting, so a send
// that fails is tried again for up to 3 seconds.
func (r *refresher) send(
	ctx context.Context,
	actions string,
) {
	for deadline := time.Now().Add(3 * time.Second); ; {
		request, err := http.NewRequestWithContext(
			ctx,
			http.MethodPost,
			"http://fzf/",
			strings.NewReader(actions),
		)
		if err != nil {
			return
		}
		response, err := r.client.Do(request)
		if err == nil {
			_ = response.Body.Close()
			return
		}
		if ctx.Err() != nil || time.Now().After(deadline) {
			return
		}
		select {
		case <-ctx.Done():
			return
		case <-time.After(50 * time.Millisecond):
		}
	}
}
