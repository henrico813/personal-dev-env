package main

import (
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"
)

// An approval can arrive after the agent's 90-second wait. The runner then
// runs gh once and saves its output, so the agent's rerun still prints the
// new PR's URL. A runner restarted by the popup or phone inherits their
// PATH, which may lack gh, so it runs the gh path saved with the request. It
// also runs gh in the request's directory, where gh finds the repository.
func TestRunnerSavesApprovedOutputForRerun(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake gh prints a PR URL and the directory it runs in, and logs
	// each call.
	dir, workdir := t.TempDir(), t.TempDir()
	calls := filepath.Join(dir, "calls")
	gh := writeScript(t, dir, "gh", "echo https://github.com/owner/repo/pull/7; echo call >> '"+calls+"'; pwd")
	// An empty PATH, so only the saved gh path can work.
	t.Setenv("PATH", t.TempDir())
	// Check the record every 10 milliseconds instead of every second, so the
	// test is quick.
	oldPoll := runnerPoll
	runnerPoll = 10 * time.Millisecond
	t.Cleanup(func() { runnerPoll = oldPoll })
	rec := record{ID: "runner", Args: []string{"pr", "create"}, State: statePending, GH: gh, Directory: workdir}
	if err := save(rec); err != nil {
		t.Fatal(err)
	}

	// Start the runner on the pending request, then approve it.
	result := make(chan int, 1)
	go func() { result <- runnerMain(rec.ID) }()

	if code := saveAnswer(rec.ID, "yes"); code != 0 {
		t.Fatalf("saveAnswer returned %d", code)
	}

	if code := receive(t, result); code != 0 {
		t.Fatalf("runner returned %d", code)
	}
	// The rerun gets an unusable gh path, so its output can only be the saved one.
	var rerun strings.Builder
	if code := reportOrRunOutput(rec.ID, "unused-gh", &rerun); code != 0 || !strings.Contains(rerun.String(), "pull/7") ||
		!strings.Contains(rerun.String(), workdir) {
		t.Fatalf("rerun returned %d with %q", code, rerun.String())
	}
	if n := countCalls(t, calls); n != 1 {
		t.Fatalf("gh ran %d times", n)
	}
}

// The saved gh can move after the request is made, such as on an upgrade.
// Recording a failed run would lose the approved write; leaving it approved
// lets a rerun from the same directory do it.
func TestRunnerWithMissingGHKeepsApproval(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	discardStderr(t)
	rec := record{ID: "moved", Args: []string{"pr", "create"}, State: stateApproved, GH: filepath.Join(t.TempDir(), "gh")}
	if err := save(rec); err != nil {
		t.Fatal(err)
	}

	code := runnerMain(rec.ID)

	got, err := load(rec.ID)
	if code == 0 || err != nil || got.State != stateApproved {
		t.Fatalf("runner returned %d, state %q, %v; want a failure and still approved", code, got.State, err)
	}
}

// A rerun of a pending request restarts a runner that died. Saving the new
// runner's PID must not restart the four-hour expiry, or an agent rerunning
// a request whose runner keeps dying would keep it pending forever.
func TestRunnerRestartKeepsRequestAge(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake runner just sleeps, standing in for one waiting for an answer.
	fake := writeScript(t, t.TempDir(), "runner", "sleep 30")
	oldStart, oldProgram := runnerStart, runnerProgram
	runnerStart, runnerProgram = startDetachedRunner, func() (string, error) { return fake, nil }
	t.Cleanup(func() { runnerStart, runnerProgram = oldStart, oldProgram })
	// The saved runner PID belongs to a process that has already exited, and
	// the request was made three hours ago.
	saved := time.Now().Add(-3 * time.Hour).Round(0)
	rec := record{ID: "dead", RunnerPID: deadPID(t), State: statePending, Updated: saved}
	if err := writeRecord(rec); err != nil {
		t.Fatal(err)
	}

	state, err := loadOrCreate(request{ID: rec.ID, Args: []string{"pr", "comment", "12"}}, "")

	if err != nil || state != statePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	got, err := load(rec.ID)
	if err != nil {
		t.Fatal(err)
	}
	// Kill the new runner's group at the end; it would otherwise sleep for 30 seconds.
	if got.RunnerPID != 0 {
		t.Cleanup(func() { _ = syscall.Kill(-got.RunnerPID, syscall.SIGKILL) })
	}
	if got.RunnerPID == rec.RunnerPID || !processAlive(got.RunnerPID) {
		t.Fatalf("record %#v", got)
	}
	if !got.Updated.Equal(saved) {
		t.Errorf("rerun moved the last change from %v to %v", saved, got.Updated)
	}
}

// Reruns and answers check the saved PIDs of the runner and the phone
// question, and start a new one when a PID is missing or dead. Without a
// runner, a late approval waits for the agent to rerun the command; without
// the phone question's PID, a rerun sends a second question to the phone.
// The runner's save writes the whole record, so it must get the record that
// already holds the phone question's PID.
func TestRequestSavesRunnerAndPhonePIDs(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake runner and phone question just sleep, standing in for ones
	// waiting for an answer. The real startDetachedRunner starts the runner.
	dir := t.TempDir()
	runner := writeScript(t, dir, "runner", "sleep 30")
	ask := writeScript(t, dir, "ask", "sleep 30")
	oldStart, oldRunner, oldMoshi := runnerStart, runnerProgram, moshiProgram
	runnerStart = startDetachedRunner
	runnerProgram = func() (string, error) { return runner, nil }
	moshiProgram = func() (string, error) { return ask, nil }
	t.Cleanup(func() { runnerStart, runnerProgram, moshiProgram = oldStart, oldRunner, oldMoshi })
	discardStderr(t)

	_, err := loadOrCreate(request{ID: "new", Args: []string{"pr", "comment", "12"}}, "")

	if err != nil {
		t.Fatal(err)
	}
	got, err := load("new")
	if err != nil {
		t.Fatal(err)
	}
	// Kill both process groups at the end; they would otherwise sleep for 30
	// seconds.
	for _, pid := range []int{got.RunnerPID, got.MoshiPID} {
		pid := pid
		if pid != 0 {
			t.Cleanup(func() { _ = syscall.Kill(-pid, syscall.SIGKILL) })
		}
	}
	if got.RunnerPID == 0 || !processAlive(got.RunnerPID) || got.MoshiPID == 0 || !processAlive(got.MoshiPID) {
		t.Fatalf("runner PID %d, phone question PID %d; want both running", got.RunnerPID, got.MoshiPID)
	}
}

// A runner killed or lost on reboot would leave a late approval unrun until
// the agent happened to rerun the command, so an approval must start a new
// one. The phone question used to save its answer with its own code, so the
// phone path is checked as well as the terminal path.
func TestApprovalRestartsDeadRunner(t *testing.T) {
	// The fake moshi-hook approves at once for the phone case.
	dir := t.TempDir()
	writeScript(t, dir, "moshi-hook", "exit 0")
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	for _, tc := range []struct {
		name   string
		answer func(id string)
	}{
		{name: "terminal", answer: func(id string) { saveAnswer(id, "yes") }},
		{name: "phone", answer: func(id string) { moshiAsk([]string{id}) }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			// Record runner starts instead of starting the test binary.
			var started []string
			oldStart := runnerStart
			runnerStart = func(rec record) record { started = append(started, rec.ID); return rec }
			t.Cleanup(func() { runnerStart = oldStart })
			// The saved runner PID belongs to a process that has already exited.
			if err := save(record{ID: "late", State: statePending, RunnerPID: deadPID(t)}); err != nil {
				t.Fatal(err)
			}

			tc.answer("late")

			if len(started) != 1 || started[0] != "late" {
				t.Fatalf("runners started = %v", started)
			}
		})
	}
}
