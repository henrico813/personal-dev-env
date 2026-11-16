package cmd

import (
	"io"
	"os"
	"strings"
	"testing"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
)

// Herdr runs pde-pr-approve for the popup and the inbox, and pde-gh starts
// it for the phone question. If the first argument picked the wrong mode,
// the popup would not open or the phone would never be asked.
func TestMainPicksEntryMode(t *testing.T) {
	cases := []struct {
		name   string
		args   []string
		code   int
		stderr string
		call   string // A call the mode makes to the fake herdr or moshi-hook.
	}{
		{name: "approve", args: []string{"/home/me/.local/bin/pde-pr-approve"}, code: 2,
			stderr: "usage: pde-pr-approve REQUEST_ID\n"},
		{name: "phone question", args: []string{"pde-pr-approve", "--moshi-ask", "waiting"}, code: 0,
			call: "ask --require-remote"},
		{name: "popup without request ID", args: []string{"pde-pr-approve", "--herdr-popup"}, code: 1},
		{name: "inbox", args: []string{"pde-pr-approve", "--herdr-inbox"}, code: 1, call: "status server",
			stderr: "pde-pr-approve: cannot open the approval popup: herdr socket unavailable\n"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// The phone question and the inbox find the waiting request.
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			t.Setenv("PDE_REQUEST_ID", "")
			for _, rec := range []approval.Record{
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
