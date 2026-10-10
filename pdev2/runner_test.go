package main

import (
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestRunnerSavesApprovedOutputForRerun(t *testing.T) {
	// An approval can arrive after the agent's 90-second wait. The runner then
	// runs gh once and saves its output, so the agent's rerun still prints the
	// new PR's URL. A runner restarted by the popup or phone inherits their
	// PATH, which may lack gh, so it runs the gh path saved with the request.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake gh prints a PR URL and logs each call.
	dir := t.TempDir()
	calls := filepath.Join(dir, "calls")
	gh := writeScript(t, dir, "gh", "echo https://github.com/owner/repo/pull/7; echo call >> '"+calls+"'")
	// An empty PATH, so only the saved gh path can work.
	t.Setenv("PATH", t.TempDir())
	oldPoll := runnerPoll
	runnerPoll = 10 * time.Millisecond
	t.Cleanup(func() { runnerPoll = oldPoll })
	rec := record{ID: "runner", Args: []string{"pr", "create"}, State: statePending, GH: gh}
	if err := save(rec); err != nil {
		t.Fatal(err)
	}

	// Start the runner on the pending request, then approve it.
	result := make(chan int, 1)
	go func() { result <- runnerMain(rec.ID) }()

	if code := saveAnswer(rec.ID, choiceYes); code != 0 {
		t.Fatalf("saveAnswer returned %d", code)
	}

	if code := receive(t, result); code != 0 {
		t.Fatalf("runner returned %d", code)
	}
	// The rerun gets an unusable gh path, so its output can only be the saved one.
	var rerun strings.Builder
	if code := reportOrRunOutput(rec.ID, "unused-gh", &rerun); code != 0 || !strings.Contains(rerun.String(), "pull/7") {
		t.Fatalf("rerun returned %d with %q", code, rerun.String())
	}
	if n := countCalls(t, calls); n != 1 {
		t.Fatalf("gh ran %d times", n)
	}
}

func TestRunnerWithMissingGHKeepsApproval(t *testing.T) {
	// The saved gh can move after the request is made, such as on an upgrade.
	// Recording a failed run would lose the approved write; leaving it approved
	// lets a rerun from the same directory do it.
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

func TestRunnerRestartKeepsRequestAge(t *testing.T) {
	// A rerun of a pending request restarts a runner that died. Saving the new
	// runner's PID must not restart the four-hour expiry, or an agent rerunning
	// a request whose runner keeps dying would keep it pending forever.
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

func TestApprovalRestartsDeadRunner(t *testing.T) {
	// A runner that was killed or lost on reboot would leave a late approval
	// never run, though the docs promise it will.
	// The fake moshi-hook approves at once for the phone case.
	dir := t.TempDir()
	writeScript(t, dir, "moshi-hook", "exit 0")
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	for _, tc := range []struct {
		name   string
		answer func(id string)
	}{
		{name: "terminal", answer: func(id string) { saveAnswer(id, choiceYes) }},
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
