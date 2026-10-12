package cmd

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"

	"pdev2/internal/approval"
)

// A phone answer counts only while the request is pending, so a late yes
// cannot touch a request that already ran. Whatever happens, the phone
// question removes its PID when it ends: the system reuses PIDs, and a stale
// one could look like a running question and stop reruns from sending a new
// one. Removing the PID alone must keep the record's age, or a request whose
// phone question keeps failing would never expire.
func TestMoshiAnswersPendingRequest(t *testing.T) {
	for _, tc := range []struct {
		name, hook, state, want string
		ageKept                 bool
	}{
		{"approve", "exit 0", approval.StatePending, approval.StateApproved, false},
		{"decline", "exit 1", approval.StatePending, approval.StateDeclined, false},
		{"handled", "exit 0", approval.StateRan, approval.StateRan, true},
		{"failed ask", "kill -9 $$", approval.StatePending, approval.StatePending, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			// A fake moshi-hook on PATH answers with the case's exit; kill -9
			// stands for an ask that died without an answer.
			dir := t.TempDir()
			fake := filepath.Join(dir, "moshi-hook")
			if err := os.WriteFile(fake, []byte("#!/bin/sh\n"+tc.hook+"\n"), 0o755); err != nil {
				t.Fatal(err)
			}
			t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
			// moshiAsk runs in this test process, so the PID saved for the
			// question is the test's own; the record was last changed an hour ago.
			saved := time.Now().Add(-time.Hour).Round(0)
			rec := approval.Record{ID: tc.name, State: tc.state, Updated: saved, MoshiPID: os.Getpid()}
			if err := approval.Write(rec); err != nil {
				t.Fatal(err)
			}

			code := moshiAsk([]string{rec.ID})

			if code != 0 {
				t.Fatalf("moshiAsk returned %d", code)
			}
			got, err := approval.Load(rec.ID)
			if err != nil {
				t.Fatal(err)
			}
			if got.State != tc.want || got.MoshiPID != 0 {
				t.Fatalf("state %q with PID %d, want %q and no PID", got.State, got.MoshiPID, tc.want)
			}
			if kept := got.Updated.Equal(saved); kept != tc.ageKept {
				t.Errorf("age kept = %t, want %t", kept, tc.ageKept)
			}
		})
	}
}

// A phone question outlived popup answers by up to four hours, leaving a
// question on the phone that could no longer decide anything.
func TestAnsweredRequestStopsPhoneAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake moshi-hook never answers, like a phone nobody looks at. It
	// runs as a child of the ask, so only a signal to the ask's whole group
	// reaches it. It touches ready once it can catch the signal, and stopped
	// when it gets one.
	dir := t.TempDir()
	ready, stopped := filepath.Join(dir, "ready"), filepath.Join(dir, "stopped")
	writeScript(t, dir, "moshi-hook", `trap 'touch "`+stopped+`"; exit 0' TERM; touch "`+ready+`"; sleep 30 & wait`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	old := moshiProgram
	moshiProgram = func() (string, error) { return writeScript(t, dir, "ask", `moshi-hook ask "$@"`), nil }
	t.Cleanup(func() { moshiProgram = old })
	// Start a real detached ask for a saved request.
	rec := approval.Record{ID: "popup", State: approval.StatePending}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	rec = startMoshi(rec)
	if rec.MoshiPID == 0 {
		t.Fatal("phone ask did not start")
	}
	t.Cleanup(func() { _ = syscall.Kill(-rec.MoshiPID, syscall.SIGKILL) })
	waitForFile(t, ready)

	// Answer as the popup does; that must stop the ask.
	saveAnswer(rec.ID, approval.ChoiceYes)

	waitForFile(t, stopped)
}

// Agents rerun a waiting command to learn what happened. While the first
// phone question is still waiting, a rerun must not send another, or the phone
// would show several questions for one request.
func TestMoshiRerunKeepsLiveAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	fake := filepath.Join(t.TempDir(), "ask")
	if err := os.WriteFile(fake, []byte("#!/bin/sh\nsleep 30\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	// The fake question program just sleeps, standing in for a phone question
	// still waiting for an answer.
	old := moshiProgram
	moshiProgram = func() (string, error) { return fake, nil }
	t.Cleanup(func() { moshiProgram = old })
	discardStderr(t)
	// Create the request, which starts the first question.
	req := approval.Request{ID: "live", Args: []string{"pr", "comment", "12"}}
	if _, err := loadOrCreate(req, ""); err != nil {
		t.Fatal(err)
	}
	first, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	// The question runs in its own session, outside the test's process group,
	// so stop its whole group at the end.
	if first.MoshiPID != 0 {
		t.Cleanup(func() { _ = syscall.Kill(-first.MoshiPID, syscall.SIGKILL) })
	}

	// Rerun the same request while the first question is still running.
	state, err := loadOrCreate(req, "")

	if err != nil || state != approval.StatePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	second, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if first.MoshiPID == 0 || second.MoshiPID != first.MoshiPID || !processAlive(first.MoshiPID) {
		t.Fatalf("PIDs %d and %d", first.MoshiPID, second.MoshiPID)
	}
}

// In live testing a phone question ended before it reached the phone, leaving
// the request waiting with nothing to answer there. A rerun must send a new
// question when the saved one is no longer running. That must keep the
// record's age; otherwise a request whose question keeps failing would stay
// pending forever as the agent reruns it.
func TestMoshiRerunRestartsDeadAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake question program just sleeps, standing in for a new question
	// waiting for an answer.
	dir := t.TempDir()
	fake := filepath.Join(dir, "ask")
	if err := os.WriteFile(fake, []byte("#!/bin/sh\nsleep 30\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	old := moshiProgram
	moshiProgram = func() (string, error) { return fake, nil }
	t.Cleanup(func() { moshiProgram = old })
	// The saved PID belongs to a process that has already exited, and the
	// request was made three hours ago.
	saved := time.Now().Add(-3 * time.Hour).Round(0)
	rec := approval.Record{ID: "dead", MoshiPID: deadPID(t), State: approval.StatePending, Updated: saved}
	if err := approval.Write(rec); err != nil {
		t.Fatal(err)
	}

	state, err := loadOrCreate(approval.Request{ID: rec.ID, Args: []string{"pr", "comment", "12"}}, "")

	if err != nil || state != approval.StatePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	got, err := approval.Load(rec.ID)
	if err != nil {
		t.Fatal(err)
	}
	// Stop the new question's process group at the end; it would otherwise
	// sleep for 30 seconds.
	if got.MoshiPID != 0 {
		t.Cleanup(func() { _ = syscall.Kill(-got.MoshiPID, syscall.SIGKILL) })
	}
	if got.MoshiPID == rec.MoshiPID || !processAlive(got.MoshiPID) {
		t.Fatalf("record %#v", got)
	}
	if !got.Updated.Equal(saved) {
		t.Errorf("rerun moved the last change from %v to %v", saved, got.Updated)
	}
}

// A fixed large PID can belong to a live process. A child that has exited and
// been waited for has a free PID, unless the kernel has already reused it.
func deadPID(t *testing.T) int {
	t.Helper()
	cmd := exec.Command("true")
	if err := cmd.Run(); err != nil {
		t.Fatal(err)
	}
	return cmd.Process.Pid
}

// The phone answers from its own process, not through the popup or a
// terminal. Its answer must clear "Approval waiting" too, or the marker stays
// on the agent's pane after the request is decided.
func TestPhoneAnswerClearsMarker(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake moshi-hook approves at once; the fake herdr logs each call.
	dir := t.TempDir()
	log := filepath.Join(dir, "log")
	writeScript(t, dir, "moshi-hook", "exit 0")
	writeScript(t, dir, "herdr", `echo "$*" >> '`+log+`'`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	if err := approval.Save(approval.Record{ID: "phone", State: approval.StatePending, PaneID: "w1:p2"}); err != nil {
		t.Fatal(err)
	}

	moshiAsk([]string{"phone"})

	calls, _ := os.ReadFile(log)
	if !strings.Contains(string(calls), "pane report-metadata --source pde.approval w1:p2 --clear-title") {
		t.Fatalf("herdr calls = %q", calls)
	}
}
