package app

import (
	"bytes"
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

// The PR list screen's row of listedPR, as fzf passes it in
// FZF_CURRENT_ITEM.
const listedRow = "example/repo\t12\t#12 Fix login"

// Writes each script into a new folder and returns the folder, for the
// test to put first on PATH.
func writeScripts(
	t *testing.T,
	scripts map[string]string,
) string {
	t.Helper()
	bin := t.TempDir()
	for name, script := range scripts {
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
	return bin
}

// Puts the fake gh first on PATH for the rest of the test, giving the
// answer named works, empty, refuse, fail, slow or slow-fetch.
func useGH(
	t *testing.T,
	answer string,
) {
	t.Helper()
	bin := writeScripts(
		t,
		map[string]string{
			"gh": fakeGH,
		},
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

// Saves list in a new cache for the rest of the test and returns the cache.
// A prs that startPRs starts reads the same cache.
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

// Returns a refresher for a new temporary folder, the cache it writes and
// a function that lists the actions it sent to fzf.
func newTestRefresher(
	t *testing.T,
	list cache.List,
) (
	*refresher,
	cache.Store,
	func() []string,
) {
	t.Helper()
	dir := t.TempDir()
	store := saveList(
		t,
		list,
	)
	sent := listenAsFzf(
		t,
		dir,
	)
	return newRefresher(
		"prs",
		dir,
		store,
	), store, sent
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

// Serves fzf's socket in dir and returns a function that lists the actions
// sent to it. Without it, the refresher's sends wait 3 seconds for a socket
// that never opens.
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

// Returns the ID of the fake gh's sleep that wrote its ID to ghPID, or 0
// once it has ended. The ID counts only while that sleep has ghPID in its
// environment, so a reused process ID or an unrelated sleep never does.
func findRunningFakeGH(
	t *testing.T,
	ghPID string,
) int {
	t.Helper()
	saved, err := os.ReadFile(ghPID)
	if err != nil {
		t.Fatal(err)
	}
	pid, err := strconv.Atoi(strings.TrimSpace(string(saved)))
	if err != nil {
		t.Fatal(err)
	}
	environ, err := os.ReadFile("/proc/" + strconv.Itoa(pid) + "/environ")
	if err != nil || !slices.Contains(
		strings.Split(
			string(environ),
			"\x00",
		),
		"GH_PID="+ghPID,
	) {
		return 0
	}
	return pid
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

// Waits until a refresher's run returns, for at most 10 seconds, and
// returns whether it did.
func waitForStop(ended <-chan struct{}) bool {
	select {
	case <-ended:
		return true
	case <-time.After(10 * time.Second):
		return false
	}
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

// Runs printDetails as fzf's side pane does, with item as the selected row,
// or no row when item is "".
func printDetailsFor(
	t *testing.T,
	a *prsApp,
	dir,
	item string,
) string {
	t.Helper()
	t.Setenv(
		"FZF_CURRENT_ITEM",
		item,
	)
	if item == "" {
		_ = os.Unsetenv("FZF_CURRENT_ITEM")
	}
	return captureOutput(
		a,
		func() {
			a.printDetails([]string{
				dir,
			})
		},
	)
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
