package ghtool

import (
	"io"
	"os/exec"
	"strings"
	"syscall"
	"testing"
	"time"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
)

// Agents rerun a waiting command to learn what happened. While the first
// phone question is still waiting, a rerun must not send another, or the phone
// would show several questions for one request.
func TestMoshiRerunKeepsLiveAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	fake := approvaltest.WriteScript(t, t.TempDir(), "ask", "sleep 30")
	// The fake question program just sleeps, standing in for a phone question
	// still waiting for an answer.
	approvaltest.Replace(t, &approveProgram, func() (string, error) { return fake, nil })
	discardStderr(t)
	// Create the request, which starts the first question.
	req := approval.Request{ID: "live", Args: []string{"pr", "comment", "12"}}
	if _, err := loadOrCreate(req, "", false); err != nil {
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
	state, err := loadOrCreate(req, "", false)

	if err != nil || state != approval.StatePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	second, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if first.MoshiPID == 0 || second.MoshiPID != first.MoshiPID || !approval.ProcessAlive(first.MoshiPID) {
		t.Fatalf("PIDs %d and %d", first.MoshiPID, second.MoshiPID)
	}
}

// In live testing a phone question ended before it reached the phone, leaving
// the request waiting with nothing to answer there. A rerun must send a new
// question when the saved one is no longer running. That must keep the
// record's age; otherwise a request whose question keeps failing would stay
// pending forever as the agent reruns it. It must also clear the old
// failure, or the agent would be told that the new question failed.
func TestMoshiRerunRestartsDeadAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake question program just sleeps, standing in for a new question
	// waiting for an answer.
	fake := approvaltest.WriteScript(t, t.TempDir(), "ask", "sleep 30")
	approvaltest.Replace(t, &approveProgram, func() (string, error) { return fake, nil })
	// The saved PID belongs to a process that has already exited, and the
	// request was made three hours ago.
	saved := time.Now().Add(-3 * time.Hour).Round(0)
	// It failed, so the record holds its reason.
	rec := approval.Record{ID: "dead", MoshiPID: deadPID(t), MoshiError: "exit status 4", State: approval.StatePending, Updated: saved}
	if err := approval.Write(rec); err != nil {
		t.Fatal(err)
	}

	state, err := loadOrCreate(approval.Request{ID: rec.ID, Args: []string{"pr", "comment", "12"}}, "", false)

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
	if got.MoshiPID == rec.MoshiPID || !approval.ProcessAlive(got.MoshiPID) {
		t.Fatalf("record %#v", got)
	}
	if !got.Updated.Equal(saved) {
		t.Errorf("rerun moved the last change from %v to %v", saved, got.Updated)
	}
	if got.MoshiError != "" {
		t.Errorf("old phone failure kept: %q", got.MoshiError)
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

// The phone question runs detached with its output thrown away, so a
// failure it saved must reach the agent. Otherwise the agent keeps telling
// the user to answer on a phone that was never asked.
func TestReportOrRunNamesFailedPhone(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	rec := approval.Record{ID: "unpaired", State: approval.StatePending, MoshiError: "exit status 4: moshi-hook: not paired"}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	var messages strings.Builder
	approvaltest.Replace[io.Writer](t, &stderr, &messages)

	reportOrRun(rec.ID, "")

	for _, want := range []string{"phone question failed", "not paired", "moshi-hook status"} {
		if !strings.Contains(messages.String(), want) {
			t.Errorf("output lacks %q:\n%s", want, messages.String())
		}
	}
}
