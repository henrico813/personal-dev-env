package app

import (
	"context"
	"io"
	"net"
	"os"
	"path/filepath"
	"strconv"
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
	self  string
	dir   string
	store cache.Store
}

func newRefresher(
	self string,
	dir string,
	store cache.Store,
) *refresher {
	return &refresher{
		self:  self,
		dir:   dir,
		store: store,
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
	// fzf's starting prompt already shows the list's age, so the first
	// refresh asks GitHub without waiting for fzf's socket.
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
		r.writeState(list.FetchedAt)
		r.send(
			ctx,
			r.buildPromptAction(),
		)
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
	list, err := fetchList(ctx)
	if ctx.Err() != nil {
		return old
	}
	if err == nil {
		err = r.store.WriteList(list)
	}
	if err != nil {
		r.writeState(old.FetchedAt)
		replaceFile(
			filepath.Join(
				r.dir,
				refreshErrorFile,
			),
			[]byte(err.Error()+"\n"),
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

// Saves the time of the list on screen, which the prompt shows as the
// list's age.
func (r *refresher) writeState(fetchedAt time.Time) {
	replaceFile(
		filepath.Join(
			r.dir,
			refreshStateFile,
		),
		[]byte(fetchedAt.Format(time.RFC3339Nano)+"\n"),
	)
}

func (r *refresher) buildPromptAction() string {
	return "transform(" + r.self + " _prompt " + quoteShell(r.dir) + ")"
}

// Sends fzf actions to fzf's socket as an HTTP request, which fzf answers
// and then closes. fzf may still be starting, so a send that fails is tried
// again for up to 3 seconds.
func (r *refresher) send(
	ctx context.Context,
	actions string,
) {
	socket := filepath.Join(
		r.dir,
		socketFile,
	)
	request := "POST / HTTP/1.1\r\nHost: fzf\r\nConnection: close\r\nContent-Length: " +
		strconv.Itoa(len(actions)) + "\r\n\r\n" + actions
	for deadline := time.Now().Add(3 * time.Second); ; {
		var dialer net.Dialer
		conn, err := dialer.DialContext(
			ctx,
			"unix",
			socket,
		)
		if err == nil {
			_ = conn.SetDeadline(time.Now().Add(time.Second))
			_, err = io.WriteString(
				conn,
				request,
			)
			if err == nil {
				// Reading until fzf closes waits for fzf to take the actions.
				// Once they are written they are never sent twice.
				_, _ = io.ReadAll(conn)
			}
			_ = conn.Close()
		}
		if err == nil || ctx.Err() != nil || time.Now().After(deadline) {
			return
		}
		select {
		case <-ctx.Done():
			return
		case <-time.After(50 * time.Millisecond):
		}
	}
}

// Writes data to path through a temporary file and a rename, so a reader
// in another prs process sees the old file or the new one, never half of
// one. The refresher is the one writer of the files it replaces.
func replaceFile(
	path string,
	data []byte,
) {
	temporary := path + ".tmp"
	if os.WriteFile(
		temporary,
		data,
		0o600,
	) == nil {
		_ = os.Rename(
			temporary,
			path,
		)
	}
}
