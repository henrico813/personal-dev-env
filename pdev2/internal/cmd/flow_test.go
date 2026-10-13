package cmd

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

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

type flowEnv struct {
	gh, calls, gate string
	output          *bytes.Buffer
}

// The fake gh answers the lookups made before asking, then logs each call
// and runs tail.
func setupFlow(t *testing.T, tail string) flowEnv {
	t.Helper()
	dir := t.TempDir()
	env := flowEnv{gate: filepath.Join(dir, "gate"), output: &bytes.Buffer{}}
	env.gh, env.calls = approvaltest.FakeGH(t, approvaltest.GHAnswers("repo view", "owner/repo"),
		approvaltest.GHAnswers("pr view", "title"), approvaltest.GHLogsCall, tail)
	t.Setenv("XDG_STATE_HOME", filepath.Join(dir, "state"))
	t.Setenv("PDE_GH_GATE", env.gate)
	approvaltest.Replace(t, &answerTimeout, 100*time.Millisecond)
	approvaltest.Replace[io.Writer](t, &stderr, env.output)
	return env
}

// newRequest runs pde-gh-write with args, as an agent does, and returns the
// request it leaves waiting. It fails unless that is the only one waiting.
func newRequest(t *testing.T, args []string) approval.Record {
	t.Helper()
	// Nobody answers, so the wait is 0 for the rest of the test.
	approvaltest.Replace(t, &answerTimeout, 0)
	if code := requestWrite(args); code != exitWaiting {
		t.Fatalf("unanswered request returned %d, want %d", code, exitWaiting)
	}
	waiting := approval.Queued()
	if len(waiting) != 1 {
		t.Fatalf("%d requests waiting, want 1", len(waiting))
	}
	return waiting[0]
}

// requestAndAnswer makes a request, moves it to state, and returns its ID.
func requestAndAnswer(t *testing.T, args []string, state string) string {
	t.Helper()
	rec := newRequest(t, args)
	rec.State = state
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	return rec.ID
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
	// The fake gh logs each call to env.calls, then exits 1.
	env := setupFlow(t, "exit 1")
	args := []string{"pr", "create", "--title", "T"}
	// Create the pending record, then approve it as pde-pr-approve would.
	id := requestAndAnswer(t, args, approval.StateApproved)

	// The first run starts gh; the second must only report the saved exit.
	first, second := requestWrite(args), requestWrite(args)

	if first != 1 || second != 1 {
		t.Errorf("runs returned %d and %d, want 1", first, second)
	}
	if calls := countCalls(t, env.calls); calls != 1 {
		t.Errorf("gh ran %d times, want 1", calls)
	}
	want := fmt.Sprintf("pde-gh-write: already ran %s, exit 1\n", id)
	if !strings.Contains(env.output.String(), want) {
		t.Errorf("output is missing %q:\n%s", want, env.output)
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
		{state: approval.StateDeclined, code: exitDeclined, line: "pde-gh-write: declined "},
		{state: approval.StateRunning, code: exitInterrupted, line: "pde-gh-write: interrupted "},
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

// A decision is final. A yes saved after a decline, such as from
// pde-pr-approve left open while the popup declined, must not approve the
// request and let gh post.
func TestLateYesKeepsDeclinedRequest(t *testing.T) {
	env := setupFlow(t, "")
	args := []string{"pr", "ready", "12"}
	id := requestAndAnswer(t, args, approval.StateDeclined)

	saveAnswer(id, approval.ChoiceYes)
	code := requestWrite(args)

	if code != exitDeclined || countCalls(t, env.calls) != 0 {
		t.Fatalf("rerun returned %d after %d gh calls, want %d and none", code, countCalls(t, env.calls), exitDeclined)
	}
}

// gh must post the text the approver read. The body file is read when the
// request is made; if gh read the file when it ran, an agent could rewrite
// it after approval and post text nobody approved.
func TestPostsApprovedTextAfterFileChanges(t *testing.T) {
	// The fake gh saves the body it reads from standard input.
	env := setupFlow(t, approvaltest.GHSavesBody)
	body := filepath.Join(t.TempDir(), "body.md")
	if err := os.WriteFile(body, []byte("approved text"), 0o600); err != nil {
		t.Fatal(err)
	}
	id := requestAndAnswer(t, []string{"pr", "comment", "12", "--body-file", body}, approval.StateApproved)
	if err := os.WriteFile(body, []byte("rewritten text"), 0o600); err != nil {
		t.Fatal(err)
	}

	// The runner runs an approval that came after the agent stopped waiting.
	code := runnerMain(id)

	posted, err := os.ReadFile(env.calls + ".body")
	if code != 0 || err != nil || string(posted) != "approved text" {
		t.Fatalf("runner returned %d and gh read %q (%v), want the approved text", code, posted, err)
	}
}

// An approval lasts four hours from its last change. After that the same
// command must ask again, so a stale yes cannot post hours or days later.
func TestExpiredApprovalNeedsNewAnswer(t *testing.T) {
	env := setupFlow(t, "")
	args := []string{"pr", "ready", "12"}
	// Approve the request five hours ago.
	rec := newRequest(t, args)
	rec.State = approval.StateApproved
	rec.Updated = time.Now().Add(-5 * time.Hour)
	if err := approval.Write(rec); err != nil {
		t.Fatal(err)
	}

	code := requestWrite(args)

	if code != exitWaiting || countCalls(t, env.calls) != 0 {
		t.Fatalf("rerun returned %d after %d gh calls, want %d and none", code, countCalls(t, env.calls), exitWaiting)
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
	requestAndAnswer(t, args, approval.StateApproved)
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

// aqua installs gh as a symlink to aqua-proxy, which picks the tool by the
// name it was started under. Running the symlink's target made every
// approved write fail once aqua's directory was on PATH.
func TestApprovedWriteRunsGHThroughSymlink(t *testing.T) {
	env := setupFlow(t, `[ "${0##*/}" = gh ] || exit 127`)
	proxy := filepath.Join(filepath.Dir(env.gh), "aqua-proxy")
	if err := os.Rename(env.gh, proxy); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(proxy, env.gh); err != nil {
		t.Fatal(err)
	}
	args := []string{"pr", "ready", "12"}
	requestAndAnswer(t, args, approval.StateApproved)

	code := requestWrite(args)

	if code != 0 {
		t.Errorf("approved write returned %d, want 0:\n%s", code, env.output)
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

// The same command in another checkout must not reuse an approval, or gh
// would act on the first checkout's repository.
func TestOtherDirectoryNeedsNewApproval(t *testing.T) {
	env := setupFlow(t, "")
	args := []string{"pr", "ready", "12"}
	first, second := t.TempDir(), t.TempDir()
	// Approve and run the command in the first directory.
	chdir(t, first)
	requestAndAnswer(t, args, approval.StateApproved)
	if code := requestWrite(args); code != 0 {
		t.Fatalf("approved run returned %d", code)
	}

	// The same command from the second directory must wait for a new answer.
	chdir(t, second)
	code := requestWrite(args)

	if code != exitWaiting || countCalls(t, env.calls) != 1 {
		t.Fatalf("second directory returned %d after %d gh calls", code, countCalls(t, env.calls))
	}
}

// The repository lookup needs the network; a rerun where it fails must still
// find the request that already ran instead of asking to post again.
func TestRerunSurvivesFailedRepoLookup(t *testing.T) {
	env := setupFlow(t, "")
	args := []string{"pr", "create", "--title", "T"}
	id := requestAndAnswer(t, args, approval.StateApproved)
	if code := requestWrite(args); code != 0 {
		t.Fatalf("approved run returned %d", code)
	}
	// Replace the fake gh with one whose repository lookup fails.
	approvaltest.RewriteFakeGH(t, env.gh, approvaltest.GHFails("repo view"), approvaltest.GHLogsCall)

	code := requestWrite(args)

	if code != 0 || countCalls(t, env.calls) != 1 || !strings.Contains(env.output.String(), "already ran "+id) {
		t.Fatalf("rerun returned %d after %d gh calls: %q", code, countCalls(t, env.calls), env.output)
	}
}

func chdir(t *testing.T, dir string) {
	t.Helper()
	old, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = os.Chdir(old) })
}
