package main

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

type flowEnv struct {
	gh, calls, gate string
	output          *bytes.Buffer
}

// The first gh on PATH is treated as the guard, so the fake real gh goes second.
func setupFlow(t *testing.T, tail string) flowEnv {
	t.Helper()
	dir := t.TempDir()
	guardDir, realDir := filepath.Join(dir, "guard"), filepath.Join(dir, "real")
	for _, sub := range []string{guardDir, realDir} {
		if err := os.Mkdir(sub, 0o700); err != nil {
			t.Fatal(err)
		}
	}
	env := flowEnv{calls: filepath.Join(dir, "calls"), gate: filepath.Join(dir, "gate"), output: &bytes.Buffer{}}
	writeScript(t, guardDir, "gh", "exit 99")
	env.gh = writeScript(t, realDir, "gh", "echo call >> '"+env.calls+"'\n"+tail)
	t.Setenv("PATH", guardDir+":"+realDir)
	t.Setenv("XDG_STATE_HOME", filepath.Join(dir, "state"))
	t.Setenv("PDE_GH_GATE", env.gate)
	oldWait, oldStderr := answerTimeout, stderr
	answerTimeout, stderr = 100*time.Millisecond, env.output
	t.Cleanup(func() { answerTimeout, stderr = oldWait, oldStderr })
	return env
}

func requestAndAnswer(t *testing.T, args []string, state string) string {
	t.Helper()
	if code := requestWrite(args); code != exitWaiting {
		t.Fatalf("unanswered request returned %d, want %d", code, exitWaiting)
	}
	req, err := parse(args, nil)
	if err != nil {
		t.Fatal(err)
	}
	rec, err := load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	rec.State = state
	if err := save(rec); err != nil {
		t.Fatal(err)
	}
	return req.ID
}

func countCalls(t *testing.T, calls string) int {
	t.Helper()
	data, err := os.ReadFile(calls)
	if os.IsNotExist(err) {
		return 0
	}
	if err != nil {
		t.Fatal(err)
	}
	return strings.Count(string(data), "call")
}

// Agents rerun the identical command to learn the outcome; that must never post
// twice. Exit 1, the most common gh failure, must be reported, not retried.
// The bash version this replaces could not tell a write that already ran from
// one that was declined.
func TestRerunReportsResultWithoutRunningAgain(t *testing.T) {
	cases := []struct {
		name     string
		exitCode int
	}{
		{name: "gh succeeds", exitCode: 0},
		{name: "gh fails", exitCode: 1},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// The fake gh logs each call to env.calls, then exits with the case's code.
			env := setupFlow(t, fmt.Sprintf("exit %d", tc.exitCode))
			args := []string{"pr", "create", "--title", "T"}
			// Create the pending record, then approve it as pde-pr-approve would.
			id := requestAndAnswer(t, args, stateApproved)

			// The first run starts gh; the second must only report the saved exit.
			first, second := requestWrite(args), requestWrite(args)

			if first != tc.exitCode || second != tc.exitCode {
				t.Errorf("runs returned %d and %d, want %d", first, second, tc.exitCode)
			}
			if calls := countCalls(t, env.calls); calls != 1 {
				t.Errorf("gh ran %d times, want 1", calls)
			}
			want := fmt.Sprintf("pde-gh-write: already ran %s, exit %d\n", id, tc.exitCode)
			if !strings.Contains(env.output.String(), want) {
				t.Errorf("output is missing %q:\n%s", want, env.output)
			}
		})
	}
}

// A declined request must stay declined, and rerunning a run that died midway
// could post twice.
func TestDecidedRequestsNeverRunGH(t *testing.T) {
	cases := []struct {
		state string
		code  int
		line  string
	}{
		{state: stateDeclined, code: exitDeclined, line: "pde-gh-write: declined "},
		{state: stateRunning, code: exitInterrupted, line: "pde-gh-write: interrupted "},
	}
	for _, tc := range cases {
		t.Run(tc.state, func(t *testing.T) {
			env := setupFlow(t, "")
			args := []string{"pr", "ready", "12"}
			// Put the record straight into the case's state, as an answer or a
			// killed run would leave it.
			id := requestAndAnswer(t, args, tc.state)

			code := requestWrite(args)

			if code != tc.code || !strings.Contains(env.output.String(), tc.line+id) {
				t.Errorf("run returned %d with output %q, want %d and %q", code, env.output, tc.code, tc.line)
			}
			if calls := countCalls(t, env.calls); calls != 0 {
				t.Errorf("gh ran %d times, want 0", calls)
			}
		})
	}
}

// gh's exit codes overlap this program's, so agents tell the two apart by the
// "pde-gh-write:" prefix. The line printed when the wait ends once lacked it.
func TestUnansweredRequestLinesHaveProgramPrefix(t *testing.T) {
	env := setupFlow(t, "")

	code := requestWrite([]string{"pr", "ready", "12"})

	if code != exitWaiting {
		t.Errorf("unanswered request returned %d, want %d", code, exitWaiting)
	}
	for _, line := range strings.Split(strings.TrimSpace(env.output.String()), "\n") {
		if !strings.HasPrefix(line, "pde-gh-write: ") {
			t.Errorf("line %q lacks the pde-gh-write: prefix", line)
		}
	}
}

// Two agents can submit the same approved request at once; only one may run gh.
func TestConcurrentRunsCallGHOnce(t *testing.T) {
	// The fake gh loops until the gate file exists, so the test decides when
	// the first run finishes.
	env := setupFlow(t, `while [ ! -f "$PDE_GH_GATE" ]; do /bin/sleep 0.01; done`)
	args := []string{"pr", "merge", "12", "--squash"}
	requestAndAnswer(t, args, stateApproved)
	// bytes.Buffer is not safe for the concurrent writes these runs make.
	stderr = io.Discard
	start := make(chan struct{})
	var group sync.WaitGroup

	// Start ten runs of the same approved command at the same moment.
	for i := 0; i < 10; i++ {
		group.Add(1)
		go func() {
			defer group.Done()
			<-start
			requestWrite(args)
		}()
	}
	close(start)
	// Hold the first gh open so the other runs queue on the lock, then let it
	// finish and wait for every run to return.
	waitUntilGHStarts(t, env.calls)
	releaseGH(t, env.gate)
	group.Wait()

	if calls := countCalls(t, env.calls); calls != 1 {
		t.Errorf("gh ran %d times, want 1", calls)
	}
}

func waitUntilGHStarts(t *testing.T, calls string) {
	t.Helper()
	deadline := time.Now().Add(2 * time.Second)
	for time.Now().Before(deadline) {
		if countCalls(t, calls) > 0 {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("gh did not start")
}

func releaseGH(t *testing.T, gate string) {
	t.Helper()
	if err := os.WriteFile(gate, nil, 0o600); err != nil {
		t.Fatal(err)
	}
}
