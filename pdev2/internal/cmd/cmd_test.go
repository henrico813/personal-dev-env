package cmd

import (
	"io"
	"os"
	"strings"
	"testing"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

// One binary does every job: the installer links pde-pr-approve to
// pde-gh-write, and the program starts its runner and phone question as
// copies of itself. If the program name or first argument picked the wrong
// mode, pde-pr-approve would print the pde-gh-write usage, or a late
// approval would never run.
func TestMainPicksEntryMode(t *testing.T) {
	cases := []struct {
		name   string
		args   []string
		code   int
		stderr string
		call   string // A call the mode makes to the fake herdr or moshi-hook.
	}{
		{name: "write", args: []string{"pde-gh-write"}, code: 2,
			stderr: "pde-gh-write: usage: pde-gh-write pr {create|edit|comment|review|merge|ready|close|reopen} [gh arguments...]\n"},
		{name: "approve by link name", args: []string{"/home/me/.local/bin/pde-pr-approve"}, code: 2,
			stderr: "usage: pde-pr-approve REQUEST_ID\n"},
		{name: "runner", args: []string{"pde-gh-write", "--pde-runner", "ran"}, code: 5,
			stderr: "pde-gh-write: already ran ran, exit 5\n"},
		{name: "phone question", args: []string{"pde-gh-write", "--moshi-ask", "waiting"}, code: 0,
			call: "ask --require-remote"},
		{name: "popup without request ID", args: []string{"pde-pr-approve", "--herdr-popup"}, code: 1},
		{name: "inbox", args: []string{"pde-pr-approve", "--herdr-inbox"}, code: 1, call: "status server"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// The runner reports the finished request; the phone question and
			// the inbox find the waiting one.
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			t.Setenv("PDE_REQUEST_ID", "")
			for _, rec := range []approval.Record{
				{ID: "ran", State: approval.StateRan, Exit: 5},
				{ID: "waiting", State: approval.StatePending},
			} {
				if err := approval.Save(rec); err != nil {
					t.Fatal(err)
				}
			}
			// The fake herdr reports no socket, so the inbox cannot open the
			// popup and exits 1. The fake moshi-hook declines.
			herdrLog := approvaltest.FakeHerdr(t, `echo "$*" >> "$log"`)
			moshiLog := approvaltest.FakeMoshiHook(t, `echo "$*" >> "$log"; exit 1`)
			var output strings.Builder
			approvaltest.Replace[io.Writer](t, &stderr, &output)

			code := Main(tc.args)

			if code != tc.code || output.String() != tc.stderr {
				t.Errorf("exit %d with stderr %q, want %d with %q", code, output.String(), tc.code, tc.stderr)
			}
			herdrCalls, _ := os.ReadFile(herdrLog)
			moshiCalls, _ := os.ReadFile(moshiLog)
			if calls := string(herdrCalls) + string(moshiCalls); !strings.Contains(calls, tc.call) {
				t.Errorf("calls %q do not include %q", calls, tc.call)
			}
		})
	}
}
