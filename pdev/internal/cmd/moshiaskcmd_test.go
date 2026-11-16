package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
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
	ask := approvaltest.WriteScript(t, dir, "ask", `moshi-hook ask "$@"`)
	// Start a detached ask for a saved request, as pde-gh does.
	pid, err := approval.StartDetached(ask)
	if err != nil {
		t.Fatal(err)
	}
	rec := approval.Record{ID: "popup", State: approval.StatePending, MoshiPID: pid}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
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

// The phone question runs detached, so when moshi-hook could not reach Moshi
// the agent kept telling the user to answer on the phone. The reason must be
// saved, so the waiting pde-gh-write can say it.
func TestUnreachablePhoneSavesItsReason(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// moshi-hook exits 4 when it cannot reach Moshi.
	approvaltest.FakeMoshiHook(t, `echo 'moshi-hook: not paired; run moshi-hook pair' >&2; exit 4`)
	// moshiAsk runs in this test process, so the saved question PID is the
	// test's own.
	rec := approval.Record{ID: "unpaired", State: approval.StatePending, MoshiPID: os.Getpid()}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}

	moshiAsk([]string{rec.ID})

	got, err := approval.Load(rec.ID)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(got.MoshiError, "not paired") {
		t.Errorf("saved failure = %q, want moshi-hook's reason", got.MoshiError)
	}
}

// A decision is final. A yes saved after a decline, such as from
// pde-pr-approve left open while the popup declined, must not approve the
// request and let gh post.
func TestLateYesKeepsDeclinedRequest(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	if err := approval.Save(approval.Record{ID: "declined", State: approval.StateDeclined}); err != nil {
		t.Fatal(err)
	}
	discardStderr(t)

	saveAnswer("declined", approval.ChoiceYes)

	if got, err := approval.Load("declined"); err != nil || got.State != approval.StateDeclined {
		t.Fatalf("state %q, %v; want still declined", got.State, err)
	}
}

// A runner killed or lost on reboot would leave a late approval unrun until
// the agent happened to rerun the command, so an approval must start a new
// one. The phone question used to save its answer with its own code, so the
// phone path is checked as well as the terminal path.
func TestApprovalRestartsDeadRunner(t *testing.T) {
	// The fake moshi-hook approves at once for the phone case.
	approvaltest.FakeMoshiHook(t, "exit 0")
	for _, tc := range []struct {
		name   string
		answer func(id string)
	}{
		{name: "terminal", answer: func(id string) { saveAnswer(id, approval.ChoiceYes) }},
		{name: "phone", answer: func(id string) { moshiAsk([]string{id}) }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			// Record runner starts instead of starting the test binary.
			var started []string
			approvaltest.Replace(t, &runnerStart, func(rec approval.Record) approval.Record { started = append(started, rec.ID); return rec })
			// The saved runner PID belongs to a process that has already exited.
			if err := approval.Save(approval.Record{ID: "late", State: approval.StatePending, RunnerPID: deadPID(t)}); err != nil {
				t.Fatal(err)
			}

			tc.answer("late")

			if len(started) != 1 || started[0] != "late" {
				t.Fatalf("runners started = %v", started)
			}
		})
	}
}
