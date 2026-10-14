package cmd

import (
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
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
			approvaltest.FakeMoshiHook(t, tc.hook)
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

// An answer given elsewhere must end the phone question's helper, or it
// keeps waiting on moshi-hook for up to four hours. A card moshi-hook has
// already sent stays on the phone; TestPhoneWaitsForPopupInHerdr covers how
// most requests avoid sending one.
func TestAnsweredRequestStopsPhoneAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake moshi-hook never answers, like a phone nobody looks at. It
	// runs as a child of the ask, so only a signal to the ask's whole group
	// reaches it. It touches ready once it can catch the signal, and stopped
	// when it gets one.
	dir := t.TempDir()
	ready, stopped := filepath.Join(dir, "ready"), filepath.Join(dir, "stopped")
	approvaltest.FakeMoshiHook(t, `trap 'touch "`+stopped+`"; exit 0' TERM; touch "`+ready+`"; sleep 30 & wait`)
	approvaltest.Replace(t, &moshiProgram, func() (string, error) { return approvaltest.WriteScript(t, dir, "ask", `moshi-hook ask "$@"`), nil })
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

// moshi-hook cannot take back a question, so the phone question sent with
// every request left a card that showed "timed out" four hours later for
// each one the owner approved in the popup. A request from Herdr now waits
// first, and one answered meanwhile must never reach the phone. One still
// waiting must, or the owner could not answer away from Herdr. Outside
// Herdr no popup shows, so the phone must be asked without the wait.
func TestPhoneWaitsForPopupInHerdr(t *testing.T) {
	for _, tc := range []struct {
		name, herdrEnv string
		popupAnswers   bool
		wantAsked      bool
	}{
		{"popup answers first", "1", true, false},
		{"still pending", "1", false, true},
		// The fake wait declines here too; outside Herdr it must not run, so
		// the phone is still asked.
		{"no popup", "", true, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			t.Setenv("HERDR_ENV", tc.herdrEnv)
			// The fake moshi-hook logs each call and approves.
			log := approvaltest.FakeMoshiHook(t, `echo ask >> "$log"`)
			rec := approval.Record{ID: "new", State: approval.StatePending, Created: time.Now()}
			if err := approval.Save(rec); err != nil {
				t.Fatal(err)
			}
			// Stands in for the 30 seconds and records how long it would be; in
			// the popup cases the owner declines in the popup during it.
			var waited time.Duration
			approvaltest.Replace(t, &waitBeforePhone, func(d time.Duration) {
				waited = d
				if tc.popupAnswers {
					saveAnswer(rec.ID, approval.ChoiceNo)
				}
			})

			moshiAsk([]string{rec.ID})

			_, err := os.Stat(log)
			if asked := err == nil; asked != tc.wantAsked {
				t.Errorf("phone asked = %t, want %t", asked, tc.wantAsked)
			}
			// A new request waits nearly the whole 30 seconds.
			if tc.herdrEnv == "1" && waited < 25*time.Second {
				t.Errorf("waited %v before asking the phone, want about %v", waited, phoneDelay)
			}
		})
	}
}

// The phone question runs detached with its output thrown away. When
// moshi-hook could not reach Moshi, as when it is not paired, the question
// ended silently, and the agent kept telling the user to answer on the
// phone. The waiting command must say the question failed and how to check.
func TestUnreachablePhoneWarnsWaitingAgent(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// moshi-hook exits 4 when it cannot reach Moshi.
	approvaltest.FakeMoshiHook(t, `echo 'moshi-hook: not paired; run moshi-hook pair' >&2; exit 4`)
	// moshiAsk runs in this test process, so the saved question PID is the
	// test's own.
	rec := approval.Record{ID: "unpaired", State: approval.StatePending, MoshiPID: os.Getpid()}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	var messages strings.Builder
	approvaltest.Replace[io.Writer](t, &stderr, &messages)

	moshiAsk([]string{rec.ID})
	reportOrRun(rec.ID, "")

	for _, want := range []string{"phone question failed", "not paired", "moshi-hook status"} {
		if !strings.Contains(messages.String(), want) {
			t.Errorf("output lacks %q:\n%s", want, messages.String())
		}
	}
}

// Agents rerun a waiting command to learn what happened. While the first
// phone question is still waiting, a rerun must not send another, or the phone
// would show several questions for one request.
func TestMoshiRerunKeepsLiveAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	fake := approvaltest.WriteScript(t, t.TempDir(), "ask", "sleep 30")
	// The fake question program just sleeps, standing in for a phone question
	// still waiting for an answer.
	approvaltest.Replace(t, &moshiProgram, func() (string, error) { return fake, nil })
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
// pending forever as the agent reruns it. It must also clear the old
// failure, or the agent would be told that the new question failed.
func TestMoshiRerunRestartsDeadAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake question program just sleeps, standing in for a new question
	// waiting for an answer.
	fake := approvaltest.WriteScript(t, t.TempDir(), "ask", "sleep 30")
	approvaltest.Replace(t, &moshiProgram, func() (string, error) { return fake, nil })
	// The saved PID belongs to a process that has already exited, and the
	// request was made three hours ago.
	saved := time.Now().Add(-3 * time.Hour).Round(0)
	// It failed, so the record holds its reason.
	rec := approval.Record{ID: "dead", MoshiPID: deadPID(t), MoshiError: "exit status 4", State: approval.StatePending, Updated: saved}
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

// The phone answers from its own process, not through the popup or a
// terminal. Its answer must clear "Approval waiting" too, or the marker stays
// on the agent's pane after the request is decided.
func TestPhoneAnswerClearsMarker(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake moshi-hook approves at once; the fake herdr logs each call.
	approvaltest.FakeMoshiHook(t, "exit 0")
	log := approvaltest.FakeHerdr(t, `echo "$*" >> "$log"`)
	if err := approval.Save(approval.Record{ID: "phone", State: approval.StatePending, PaneID: "w1:p2"}); err != nil {
		t.Fatal(err)
	}

	moshiAsk([]string{"phone"})

	calls, _ := os.ReadFile(log)
	if !strings.Contains(string(calls), "pane report-metadata --source pde.approval w1:p2 --clear-title") {
		t.Fatalf("herdr calls = %q", calls)
	}
}
