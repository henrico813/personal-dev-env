package app

import (
	"bytes"
	"context"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"testing"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
)

// Runs prs instead of the tests when a test starts this binary as prs.
func TestMain(m *testing.M) {
	if os.Getenv("PRS_TEST_RUN_AS_PRS") == "1" {
		os.Exit(Run(
			os.Args[1:],
			os.Stdin,
			os.Stdout,
			os.Stderr,
		))
	}
	os.Exit(m.Run())
}

// These tests run prs with gh, and sometimes fzf and less, replaced by
// scripts, so gh's JSON goes through the real decoding and rendering.

// One open pull request as gh's GraphQL search reply has it.
const listReply = `{"data": {"search": {"nodes": [{"number": 12, "title": "Fix login",
  "state": "OPEN", "author": {"login": "alice"}, "repository": {"nameWithOwner": "example/repo"},
  "updatedAt": "2026-10-31T10:00:00Z", "labels": {"nodes": []}, "commits": {"nodes": []}}]}}}`

// Answers as gh does when GitHub works, lists nothing, refuses with its
// reason, fails, or is slow, as GH_ANSWER says. It logs each call as list or
// fetch to GH_LOG when that is set.
const fakeGH = `#!/bin/sh
case "$*" in
*"search("*) call=list ;;
*) call=fetch ;;
esac
if [ -n "$GH_LOG" ]; then
  echo "$call" >> "$GH_LOG"
fi
case "$GH_ANSWER" in
empty)
  if [ "$1" = api ]; then
    echo '{"data": {"search": {"nodes": []}}}'
    exit 0
  fi ;;
refuse)
  echo 'HTTP 401: Bad credentials (https://api.github.com/graphql)' >&2
  exit 1 ;;
fail)
  echo 'HTTP 502: Bad Gateway' >&2
  exit 1 ;;
slow)
  # Each API call hangs and ignores the signals that stop prs, so prs's own
  # cleanup has to end it. The trap comes first, so no signal ends it early.
  if [ "$1" = api ]; then
    trap '' HUP INT TERM
    echo $$ > "$GH_PID"
    exec sleep 30
  fi ;;
slow-fetch)
  # The list answers at once, and each fetch for one pull request hangs.
  if [ "$call" = fetch ]; then
    exec sleep 30
  fi ;;
esac
case "$1" in
api) echo '` + listReply + `' ;;
pr) printf 'diff --git a/a.go b/a.go\n+a\ndiff --git a/b.go b/b.go\n+b\ndiff --git a/c.go b/c.go\n+c\n' ;;
esac
`

// The reasons the fake gh gives when it refuses and when it fails.
const (
	refusedReason = "HTTP 401: Bad credentials"
	failedReason  = "HTTP 502: Bad Gateway"
)

// The pull request that the fake gh lists.
var listedPR = github.PullRequest{
	Number:     12,
	Title:      "Fix login",
	State:      "OPEN",
	Author:     github.Actor{Login: "alice"},
	Repository: github.Repository{NameWithOwner: "example/repo"},
	UpdatedAt: time.Date(
		2026,
		10,
		31,
		10,
		0,
		0,
		0,
		time.UTC,
	),
}

// A failed GitHub call must say which call failed and show gh's reason.
// Otherwise an empty list or a blank page looks like there is nothing to
// read.
func TestRunSaysWhatFailedAndWhy(t *testing.T) {
	tests := []struct {
		name       string
		answer     string
		args       []string
		want       string
		wantReason string
	}{
		{
			name:       "PR list with nothing saved",
			answer:     "fail",
			args:       nil,
			want:       "Could not get your pull requests from GitHub.",
			wantReason: failedReason,
		},
		{
			name:   "this folder's repository",
			answer: "refuse",
			args: []string{
				"12",
			},
			want:       "Could not find this folder's GitHub repository.",
			wantReason: refusedReason,
		},
		{
			name:   "PR summary screen",
			answer: "refuse",
			args: []string{
				"_summary",
				"example/repo",
				"12",
			},
			want:       "Could not get #12.",
			wantReason: refusedReason,
		},
		{
			name:   "comments screen",
			answer: "fail",
			args: []string{
				"_comments",
				"example/repo",
				"12",
			},
			want:       "Could not get the comments of #12.",
			wantReason: failedReason,
		},
		{
			name:   "diff screen",
			answer: "refuse",
			args: []string{
				"_diff",
				"example/repo",
				"12",
			},
			want:       "Could not get the diff of #12.",
			wantReason: refusedReason,
		},
		{
			name:   "one pull request",
			answer: "fail",
			args: []string{
				"example/repo",
				"12",
			},
			want:       "Could not get #12.",
			wantReason: failedReason,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output := runPRs(
				t,
				tt.answer,
				tt.args...,
			)

			for _, want := range []string{
				tt.want,
				tt.wantReason,
			} {
				if !strings.Contains(
					output,
					want,
				) {
					t.Errorf(
						"output omits %q:\n%s",
						want,
						output,
					)
				}
			}
		})
	}
}

// The installer checks each new build of prs by running prs _keys. If it
// needs gh or fails, every install rolls back all the tools it built.
func TestRunPrintsKeysWithoutGH(t *testing.T) {
	t.Setenv(
		"PATH",
		t.TempDir(),
	)
	var output bytes.Buffer

	// Run catches Ctrl-C, and the reset lets Ctrl-C stop go test again.
	defer signal.Reset(os.Interrupt)
	code := Run(
		[]string{
			"_keys",
		},
		strings.NewReader(""),
		&output,
		&output,
	)

	if code != 0 || !strings.Contains(
		output.String(),
		"PR summary screen",
	) {
		t.Errorf(
			"prs _keys exited %d and printed\n%s\nwant 0 and the PR list screen's help",
			code,
			output.String(),
		)
	}
}

// The side pane prints the status strip when it has 3 rows or fewer, and
// the longer details otherwise. A wrong choice cuts the checks off a phone
// with its keyboard up, or wastes the rows of a taller side pane.
func TestPrintDetailsFitsSidePaneRows(t *testing.T) {
	tests := []struct {
		name      string
		lines     string
		wantRows  int
		wantTitle bool
	}{
		{
			name:      "3 rows",
			lines:     "3",
			wantRows:  3,
			wantTitle: false,
		},
		{
			name:      "4 rows",
			lines:     "4",
			wantRows:  5,
			wantTitle: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			saveList(
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
				self: "prs",
				now:  listedPR.UpdatedAt.Add(2 * time.Hour),
			}

			got := captureOutput(
				a,
				func() {
					a.printDetails([]string{
						t.TempDir(),
						"example/repo",
						"12",
					})
				},
			)

			rows := strings.Count(
				got,
				"\n",
			)
			hasTitle := strings.Contains(
				got,
				listedPR.Title,
			)
			if rows != tt.wantRows || hasTitle != tt.wantTitle {
				t.Errorf(
					"side pane shows %d rows, title %v, want %d rows, title %v:\n%s",
					rows,
					hasTitle,
					tt.wantRows,
					tt.wantTitle,
					got,
				)
			}
		})
	}
}

// Records each file's first line that less shows, then quits less with
// the next key from the keys file, as the lesskey file's quit actions do.
const recordingLess = `#!/bin/sh
head -n 1 >> "$LOG"
key=$(head -c 1 "$KEYS")
tail -c +2 "$KEYS" > "$KEYS.rest" && mv "$KEYS.rest" "$KEYS"
exit $(printf '%d' "'$key")
`

// Logs the file the file picker opens on, then ends as the next line of
// the picks file says. A line such as "l 1" prints the key and file 1 as
// fzf does, and "l -" prints the key alone and exits 1, as fzf does when
// nothing matches the search.
const choosingFzf = `#!/bin/sh
pos=$(printf '%s\n' "$@" | sed -n 's/^--bind=load:pos(\([0-9]*\))$/\1/p')
echo "picker on $pos" >> "$LOG"
pick=$(head -n 1 "$PICKS")
tail -n +2 "$PICKS" > "$PICKS.rest" && mv "$PICKS.rest" "$PICKS"
key=${pick% *}
row=${pick#* }
if [ "$row" = - ]; then
  printf '%s\n' "$key"
  exit 1
fi
printf '%s\n%s\t+1 -0\ta.go\n' "$key" "$row"
`

// The diff screen's one prs process must keep the current file across
// less and fzf, with L and H stopping at the ends, the file picker opening
// on the current file, o and a search with no match closing the picker on
// it, and l opening the chosen file. Otherwise a key shows the wrong diff
// or closes the diff screen.
func TestOpenDiffTracksFileAcrossKeys(t *testing.T) {
	// less and fzf are scripts that log what they show and take their keys
	// from files.
	bin := t.TempDir()
	log := filepath.Join(
		bin,
		"log",
	)
	keys := filepath.Join(
		bin,
		"keys",
	)
	picks := filepath.Join(
		bin,
		"picks",
	)
	writeFile(
		t,
		filepath.Join(
			bin,
			"less",
		),
		recordingLess,
		0o755,
	)
	writeFile(
		t,
		filepath.Join(
			bin,
			"fzf",
		),
		choosingFzf,
		0o755,
	)
	// The diff view gets these keys in turn, and the file picker these picks.
	writeFile(
		t,
		keys,
		"LLLHoooh",
		0o600,
	)
	writeFile(
		t,
		picks,
		"o 1\nl -\nl 1\n",
		0o600,
	)
	t.Setenv(
		"LOG",
		log,
	)
	t.Setenv(
		"KEYS",
		keys,
	)
	t.Setenv(
		"PICKS",
		picks,
	)
	t.Setenv(
		"PATH",
		bin+":"+os.Getenv("PATH"),
	)

	runPRs(
		t,
		"works",
		"_diff",
		"example/repo",
		"12",
	)

	shown, _ := os.ReadFile(log)
	want := "diff --git a/a.go b/a.go\n" + // The first file.
		"diff --git a/b.go b/b.go\n" + // L
		"diff --git a/c.go b/c.go\n" + // L
		"diff --git a/c.go b/c.go\n" + // L stays on the last file.
		"diff --git a/b.go b/b.go\n" + // H
		"picker on 2\n" + // o opens the picker on b.go.
		"diff --git a/b.go b/b.go\n" + // The picker's o, with a.go selected.
		"picker on 2\n" + // o
		"diff --git a/b.go b/b.go\n" + // The picker's l with nothing matching.
		"picker on 2\n" + // o
		"diff --git a/a.go b/a.go\n" // The picker's l on a.go, then h.
	if string(shown) != want {
		t.Errorf(
			"screens shown were\n%s\nwant\n%s",
			shown,
			want,
		)
	}
}

// A failed refresh must keep the saved list, show the list's age in the
// prompt and show gh's reason in the side pane, even when the list is
// empty. Otherwise the list empties, or looks up to date, while GitHub is
// down.
func TestRefreshListKeepsListShowsReason(t *testing.T) {
	dir := t.TempDir()
	saved := cache.List{
		FetchedAt: time.Now().Add(-5 * time.Minute),
		PullRequests: []github.PullRequest{
			listedPR,
		},
	}
	store := saveList(
		t,
		saved,
	)
	setScreen(t)
	// fzf asks for the layout of an empty list with no row to read.
	t.Setenv(
		"FZF_TOTAL_COUNT",
		"0",
	)
	a := &prsApp{
		self:   "prs",
		stdout: io.Discard,
		now:    time.Now(),
	}
	// The refresher sends its prompt to fzf's socket first, which this
	// test does not check.
	listenAsFzf(
		t,
		dir,
	)
	r := newRefresher(
		a.self,
		dir,
		store,
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
	prompt := buildListPrompt(
		dir,
		time.Now(),
	)
	details := captureOutput(
		a,
		func() {
			a.printDetails([]string{
				dir,
				"example/repo",
				"12",
			})
		},
	)
	emptyLayout := captureOutput(
		a,
		func() {
			a.printListLayout([]string{
				dir,
				"result",
			})
		},
	)

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
	if prompt != "prs updated 5m ago " {
		t.Errorf(
			"prompt is %q, want the list's age",
			prompt,
		)
	}
	for _, want := range []string{
		"Could not refresh the list.",
		failedReason,
		"Fix login",
	} {
		if !strings.Contains(
			details,
			want,
		) {
			t.Errorf(
				"side pane omits %q:\n%s",
				want,
				details,
			)
		}
	}
	if !strings.Contains(
		emptyLayout,
		"change-preview(cat ",
	) {
		t.Errorf(
			"empty list's layout is %q, want gh's reason in the side pane",
			emptyLayout,
		)
	}
}

// Once a refresh works after a failure, fzf must reload the rows, the side
// pane must go back to the details even when the list is still empty, and
// the prompt must drop the age. Otherwise the side pane stays on the error
// or runs cat on the deleted error file.
func TestRefreshListRecoversAfterFailure(t *testing.T) {
	dir := t.TempDir()
	store := saveList(
		t,
		cache.List{},
	)
	setScreen(t)
	// The list stays empty, so fzf asks for the layout with no row to read.
	t.Setenv(
		"FZF_TOTAL_COUNT",
		"0",
	)
	a := &prsApp{
		self:   "prs",
		stdout: io.Discard,
		now:    time.Now(),
	}
	sent := listenAsFzf(
		t,
		dir,
	)
	r := newRefresher(
		a.self,
		dir,
		store,
	)
	// A refresh fails first, and the empty list's side pane shows gh's reason.
	useGH(
		t,
		"fail",
	)
	kept := r.refreshList(
		context.Background(),
		cache.List{FetchedAt: time.Now().Add(-5 * time.Minute)},
	)
	captureOutput(
		a,
		func() {
			a.printListLayout([]string{
				dir,
				"result",
			})
		},
	)
	// The next refresh works and still finds no pull requests.
	useGH(
		t,
		"empty",
	)

	fresh := r.refreshList(
		context.Background(),
		kept,
	)
	prompt := buildListPrompt(
		dir,
		time.Now(),
	)
	layout := captureOutput(
		a,
		func() {
			a.printListLayout([]string{
				dir,
				"result",
			})
		},
	)

	if fresh.FetchedAt.Equal(kept.FetchedAt) || !strings.Contains(
		strings.Join(
			sent(),
			"\n",
		),
		"reload(prs _rows)",
	) {
		t.Errorf(
			"working refresh kept the list from %v and sent %q, want a new list and a reload",
			fresh.FetchedAt,
			sent(),
		)
	}
	if !strings.Contains(
		layout,
		" _meta "+dir+" {1} {2})+refresh-preview",
	) || strings.Contains(
		layout,
		"cat ",
	) {
		t.Errorf(
			"layout is %q, want the details back in the side pane",
			layout,
		)
	}
	if prompt != "prs " {
		t.Errorf(
			"prompt is %q, want no age",
			prompt,
		)
	}
}

// A refresh that works must delete the cache entries of pull requests that
// are no longer listed, and keep the listed ones. Otherwise the cache keeps
// every pull request ever listed, or fetches the listed ones again.
func TestRefreshListRemovesUnlistedEntries(t *testing.T) {
	dir := t.TempDir()
	closed := github.PullRequest{
		Number:     7,
		Repository: listedPR.Repository,
		UpdatedAt:  listedPR.UpdatedAt,
	}
	store := saveList(
		t,
		cache.List{},
	)
	for _, pr := range []github.PullRequest{
		closed,
		listedPR,
	} {
		if err := store.WriteDiff(
			pr,
			[]byte("diff"),
		); err != nil {
			t.Fatal(err)
		}
	}
	// The refresher sends its actions to fzf's socket, which this test does
	// not check.
	listenAsFzf(
		t,
		dir,
	)
	r := newRefresher(
		"prs",
		dir,
		store,
	)
	useGH(
		t,
		"works",
	)

	r.refreshList(
		context.Background(),
		cache.List{},
	)

	if store.HasDiff(closed) || !store.HasDiff(listedPR) {
		t.Errorf(
			"after the refresh the cache holds #%d %v and #%d %v, want false and true",
			closed.Number,
			store.HasDiff(closed),
			listedPR.Number,
			store.HasDiff(listedPR),
		)
	}
}

// r must refresh the list at once, even while the background fetch is
// still waiting on GitHub for other pull requests. Otherwise r looks
// broken for as long as the whole fetch takes.
func TestRefresherRunRefreshesListDuringFetch(t *testing.T) {
	dir := t.TempDir()
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
	store := saveList(
		t,
		cache.List{PullRequests: prs},
	)
	// The refresher sends its prompt to fzf's socket, which this test does
	// not check.
	listenAsFzf(
		t,
		dir,
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
	requests := make(
		chan os.Signal,
		1,
	)
	ended := make(chan struct{})
	r := newRefresher(
		"prs",
		dir,
		store,
	)
	// The refresher must stop once its context ends, or it leaks gh calls.
	t.Cleanup(func() {
		cancel()
		select {
		case <-ended:
		case <-time.After(10 * time.Second):
			t.Error("the refresher did not stop after its context ended")
		}
	})
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
	waitForLog(
		t,
		ghLog,
		"fetch",
		1,
	)

	// The send stands in for the SIGUSR1 that r's _refresh sends.
	requests <- syscall.SIGUSR1
	listed := waitForLog(
		t,
		ghLog,
		"list",
		1,
	)

	if !listed {
		t.Error("r did not start the list query while the fetch was running")
	}
}

// Waits in place of the PR list screen's fzf, after writing the path of
// its --listen socket to READY.
const listeningFzf = `#!/bin/sh
for arg; do
  case "$arg" in
  --listen=*) echo "${arg#--listen=}" > "$READY.tmp" && mv "$READY.tmp" "$READY" ;;
  esac
done
exec sleep 30
`

// The r key runs _refresh in another prs process, which must reach the
// main process's refresher. Otherwise r does nothing, and the list stays as
// old as the last start.
func TestRequestListRefreshStartsListQuery(t *testing.T) {
	bin := t.TempDir()
	cacheHome := t.TempDir()
	ready := filepath.Join(
		bin,
		"ready",
	)
	ghLog := filepath.Join(
		bin,
		"gh-log",
	)
	for name, script := range map[string]string{
		"gh":  fakeGH,
		"fzf": listeningFzf,
	} {
		writeFile(
			t,
			filepath.Join(
				bin,
				name,
			),
			script,
			0o755,
		)
	}
	saveOneListedPR(
		t,
		cacheHome,
	)
	// The main prs starts on the saved list and refreshes it once.
	_, _ = startPRs(
		t,
		"READY="+ready,
		"GH_ANSWER=works",
		"GH_LOG="+ghLog,
		"TMPDIR="+t.TempDir(),
		"XDG_CACHE_HOME="+cacheHome,
		"PATH="+bin+":"+os.Getenv("PATH"),
	)
	waitForFile(
		t,
		ready,
	)
	socket, err := os.ReadFile(ready)
	if err != nil {
		t.Fatal(err)
	}
	dir := filepath.Dir(strings.TrimSpace(string(socket)))
	listenAsFzf(
		t,
		dir,
	)
	if !waitForLog(
		t,
		ghLog,
		"list",
		1,
	) {
		t.Fatal("prs did not refresh the saved list at start")
	}

	defer signal.Reset(os.Interrupt)
	Run(
		[]string{
			"_refresh",
			dir,
		},
		strings.NewReader(""),
		io.Discard,
		io.Discard,
	)
	refreshed := waitForLog(
		t,
		ghLog,
		"list",
		2,
	)

	if !refreshed {
		t.Error("_refresh did not make the main prs query the list again")
	}
}

// While a search is open the help text row reads Esc cancel, and a tap on
// Esc must end the search. Otherwise you cannot leave the search by touch
// with the keyboard up.
func TestPrintListTapActionEndsSearch(t *testing.T) {
	t.Setenv(
		"FZF_COLUMNS",
		"50",
	)
	t.Setenv(
		"FZF_PROMPT",
		"/ ",
	)
	t.Setenv(
		"FZF_CLICK_FOOTER_COLUMN",
		"2",
	)

	output := runPRs(
		t,
		"works",
		"_tap",
		t.TempDir(),
	)

	if !strings.HasPrefix(
		output,
		"clear-query+",
	) {
		t.Errorf(
			"tap on Esc printed %q, want the action that ends the search",
			output,
		)
	}
}

// fzf binds single keys, so gg is two presses of g. A second g within a
// second must go to the first row, and a lone or late g must do nothing,
// or one stray g jumps to the top.
func TestPrintFirstOnQuickSecondG(t *testing.T) {
	tests := []struct {
		name        string
		pressedOnce bool
		ago         time.Duration
		want        string
	}{
		{
			name:        "second g soon after",
			pressedOnce: true,
			ago:         300 * time.Millisecond,
			want:        "first\n",
		},
		{
			name:        "second g after a pause",
			pressedOnce: true,
			ago:         2 * time.Second,
			want:        "",
		},
		{
			name: "lone g",
			want: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			if tt.pressedOnce {
				writeFile(
					t,
					filepath.Join(
						dir,
						"g",
					),
					time.Now().Add(-tt.ago).Format(time.RFC3339Nano),
					0o600,
				)
			}

			output := runPRs(
				t,
				"works",
				"_g",
				dir,
			)

			if output != tt.want {
				t.Errorf(
					"g printed %q, want %q",
					output,
					tt.want,
				)
			}
		})
	}
}

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
			bin := t.TempDir()
			temp := t.TempDir()
			cacheHome := t.TempDir()
			ready := filepath.Join(
				bin,
				"ready",
			)
			ghPID := filepath.Join(
				bin,
				"gh-pid",
			)
			for name, script := range map[string]string{
				"gh":   fakeGH,
				"fzf":  hangingFzf,
				"less": pickerLess,
			} {
				writeFile(
					t,
					filepath.Join(
						bin,
						name,
					),
					script,
					0o755,
				)
			}
			// A saved list makes the PR list screen open at once and refresh
			// in the background, and without one prs fetches before fzf.
			if tt.saved {
				saveOneListedPR(
					t,
					cacheHome,
				)
			}
			prs, exited := startPRs(
				t,
				"PRS="+os.Args[0],
				"READY="+ready,
				"GH_ANSWER=slow",
				"GH_PID="+ghPID,
				"TMPDIR="+temp,
				"XDG_CACHE_HOME="+cacheHome,
				"PATH="+bin+":"+os.Getenv("PATH"),
			)
			// The file picker opens when there is a saved list.
			if tt.saved {
				waitForFile(
					t,
					ready,
				)
			}
			// With a saved list the fake fzf never opens fzf's socket, so the
			// refresher's send retries for 3 seconds before its gh starts.
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
			if pid := readPID(
				t,
				ghPID,
			); isFakeGHRunning(
				pid,
				ghPID,
			) {
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

// Saves a PR list with the pull request the fake gh knows, so the PR list
// screen starts from the cache and refreshes in the background.
func saveOneListedPR(
	t *testing.T,
	cacheHome string,
) {
	t.Helper()
	dir := filepath.Join(
		cacheHome,
		"prs",
	)
	if err := os.MkdirAll(
		dir,
		0o700,
	); err != nil {
		t.Fatal(err)
	}
	writeFile(
		t,
		filepath.Join(
			dir,
			"list.json",
		),
		`{"fetchedAt": "2026-10-31T10:00:00Z", "pullRequests": [{"number": 12,
		  "repository": {"nameWithOwner": "example/repo"}, "updatedAt": "2026-10-31T10:00:00Z"}]}`,
		0o600,
	)
}

// Saves list in a new cache for the rest of the test and returns the cache.
func saveList(
	t *testing.T,
	list cache.List,
) cache.Store {
	t.Helper()
	t.Setenv(
		"XDG_CACHE_HOME",
		t.TempDir(),
	)
	store, err := cache.Open()
	if err != nil {
		t.Fatal(err)
	}
	if err := store.WriteList(list); err != nil {
		t.Fatal(err)
	}
	return store
}

// Tells the layout commands, as fzf does, that the screen is 50x40.
func setScreen(t *testing.T) {
	t.Helper()
	t.Setenv(
		"FZF_LINES",
		"40",
	)
	t.Setenv(
		"FZF_COLUMNS",
		"50",
	)
}

// Starts this test binary as prs in its own process group with env added,
// and returns it with a channel closed when it exits. The group is killed
// when the test ends, so a failed test leaves nothing running.
func startPRs(
	t *testing.T,
	env ...string,
) (
	*exec.Cmd,
	<-chan struct{},
) {
	t.Helper()
	prs := exec.Command(os.Args[0])
	prs.Env = append(
		append(
			os.Environ(),
			"PRS_TEST_RUN_AS_PRS=1",
			"NO_COLOR=1",
		),
		env...,
	)
	// Its own process group stands in for the terminal's.
	prs.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	if err := prs.Start(); err != nil {
		t.Fatal(err)
	}
	exited := make(chan struct{})
	go func() {
		_ = prs.Wait()
		close(exited)
	}()
	t.Cleanup(func() {
		_ = syscall.Kill(
			-prs.Process.Pid,
			syscall.SIGKILL,
		)
		<-exited
	})
	return prs, exited
}

// Fails the test unless exited closes within 5 seconds.
func waitForExit(
	t *testing.T,
	exited <-chan struct{},
) {
	t.Helper()
	select {
	case <-exited:
	case <-time.After(5 * time.Second):
		t.Error("prs did not exit within 5 seconds of the signal")
	}
}

// Serves fzf's socket in dir and returns a function that lists the
// actions sent to it.
func listenAsFzf(
	t *testing.T,
	dir string,
) func() []string {
	t.Helper()
	listener, err := net.Listen(
		"unix",
		filepath.Join(
			dir,
			socketFile,
		),
	)
	if err != nil {
		t.Fatal(err)
	}
	var mu sync.Mutex
	var sent []string
	server := &http.Server{Handler: http.HandlerFunc(func(
		_ http.ResponseWriter,
		request *http.Request,
	) {
		body, _ := io.ReadAll(request.Body)
		mu.Lock()
		defer mu.Unlock()
		sent = append(
			sent,
			string(body),
		)
	})}
	go func() { _ = server.Serve(listener) }()
	t.Cleanup(func() { _ = server.Close() })
	return func() []string {
		mu.Lock()
		defer mu.Unlock()
		return slices.Clone(sent)
	}
}

// Returns what work printed through a's stdout.
func captureOutput(
	a *prsApp,
	work func(),
) string {
	var out bytes.Buffer
	saved := a.stdout
	a.stdout = &out
	defer func() { a.stdout = saved }()
	work()
	return out.String()
}

// Puts the fake gh first on PATH for the rest of the test, giving the
// answer named works, empty, refuse, fail, slow or slow-fetch.
func useGH(
	t *testing.T,
	answer string,
) {
	t.Helper()
	bin := t.TempDir()
	writeFile(
		t,
		filepath.Join(
			bin,
			"gh",
		),
		fakeGH,
		0o755,
	)
	t.Setenv(
		"PATH",
		bin+":"+os.Getenv("PATH"),
	)
	t.Setenv(
		"GH_ANSWER",
		answer,
	)
}

func readPID(
	t *testing.T,
	path string,
) int {
	t.Helper()
	saved, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(strings.TrimSpace(string(saved)))
	if err != nil {
		t.Fatal(err)
	}
	return pid
}

// Returns whether pid is still the fake gh's sleep of this run, which has
// ghPID in its environment, so a reused process ID or an unrelated sleep is
// never mistaken for it.
func isFakeGHRunning(
	pid int,
	ghPID string,
) bool {
	environ, err := os.ReadFile("/proc/" + strconv.Itoa(pid) + "/environ")
	return err == nil && slices.Contains(
		strings.Split(
			string(environ),
			"\x00",
		),
		"GH_PID="+ghPID,
	)
}

// Returns how many entries dir still has once it is empty or 5 seconds
// have passed.
func waitForEmptyDir(
	t *testing.T,
	dir string,
) int {
	t.Helper()
	for deadline := time.Now().Add(5 * time.Second); ; {
		entries, err := os.ReadDir(dir)
		if err != nil {
			t.Fatal(err)
		}
		if len(entries) == 0 || time.Now().After(deadline) {
			return len(entries)
		}
		time.Sleep(10 * time.Millisecond)
	}
}

func waitForFile(
	t *testing.T,
	path string,
) {
	t.Helper()
	for deadline := time.Now().Add(10 * time.Second); time.Now().Before(deadline); {
		if _, err := os.Stat(path); err == nil {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf(
		"%s did not appear",
		path,
	)
}

// Returns whether the log at path gets count lines reading line within 5
// seconds.
func waitForLog(
	t *testing.T,
	path,
	line string,
	count int,
) bool {
	t.Helper()
	for deadline := time.Now().Add(5 * time.Second); time.Now().Before(deadline); {
		logged, _ := os.ReadFile(path)
		found := 0
		for _, logLine := range strings.Split(
			string(logged),
			"\n",
		) {
			if logLine == line {
				found++
			}
		}
		if found >= count {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return false
}

// Returns what prs printed with the fake gh giving answer, colors off and
// an empty cache.
func runPRs(
	t *testing.T,
	answer string,
	args ...string,
) string {
	t.Helper()
	useGH(
		t,
		answer,
	)
	t.Setenv(
		"NO_COLOR",
		"1",
	)
	t.Setenv(
		"TMPDIR",
		t.TempDir(),
	)
	t.Setenv(
		"XDG_CACHE_HOME",
		t.TempDir(),
	)

	// Run catches Ctrl-C, and the reset lets Ctrl-C stop go test again.
	defer signal.Reset(os.Interrupt)
	var output bytes.Buffer
	Run(
		args,
		strings.NewReader(""),
		&output,
		&output,
	)
	return output.String()
}

func writeFile(
	t *testing.T,
	path,
	content string,
	mode os.FileMode,
) {
	t.Helper()
	if err := os.WriteFile(
		path,
		[]byte(content),
		mode,
	); err != nil {
		t.Fatal(err)
	}
}
