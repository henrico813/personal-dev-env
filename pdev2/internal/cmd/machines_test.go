package cmd

import (
	"os"
	"slices"
	"strings"
	"testing"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

// An approval waiting on Wallace was invisible from this machine, and merges
// stayed blocked. The machine that asks marks the agent's pane, and the inbox
// on another machine reads that mark through herdr --machine. Both sides must
// use the same mark, and an answer must remove it, or the inbox would send
// the user to a machine where nothing waits. Dismissing is not an answer: the
// inbox there still lists a dismissed request, so it must still count here,
// while the sidebar title clears as it did before.
func TestOtherMachineSeesMarkerUntilAnswered(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	t.Setenv("HERDR_ENV", "1")
	// One fake herdr plays both machines: it keeps the tokens that
	// report-metadata sets on pane w1:p1 and lists them as Wallace's panes.
	// It logs every call.
	log := approvaltest.FakeHerdr(t, `echo "$*" >> "$log"; tokens="$log.tokens"; mkdir -p "$tokens"
case "$*" in
"machine list --json") echo '[{"id":"id-wallace","label":"Wallace","enabled":true}]' ;;
"pane report-metadata"*)
	while [ $# -gt 0 ]; do
		case "$1" in
		--token) echo "${2#*=}" > "$tokens/${2%%=*}" ;;
		--clear-token) rm -f "$tokens/$2" ;;
		esac
		shift
	done ;;
"--machine id-wallace pane list")
	set_tokens=$(for f in "$tokens"/*; do [ -e "$f" ] && printf '"%s":"%s",' "${f##*/}" "$(cat "$f")"; done)
	echo "{\"result\":{\"panes\":[{\"pane_id\":\"w1:p1\",\"workspace_id\":\"w1\",\"tokens\":{${set_tokens%,}}},{\"pane_id\":\"w1:p2\",\"workspace_id\":\"w1\"}]}}" ;;
"--machine id-wallace workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"notes"}]}}' ;;
esac`)
	rec := approval.Record{ID: "merge", State: approval.StatePending, PaneID: "w1:p1"}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}

	notifyRequest(rec)
	waiting := otherMachineLines()
	dismissRequest(rec.ID)
	dismissed := otherMachineLines()
	callsAfterDismiss, _ := os.ReadFile(log)
	saveAnswer(rec.ID, approval.ChoiceNo)
	answered := otherMachineLines()

	if want := []string{"Wallace: 1 waiting in", "  notes"}; !slices.Equal(waiting, want) {
		t.Errorf("while waiting: %q, want %q", waiting, want)
	}
	if want := []string{"Wallace: 1 waiting in", "  notes"}; !slices.Equal(dismissed, want) {
		t.Errorf("after dismissing: %q, want %q", dismissed, want)
	}
	if !strings.Contains(string(callsAfterDismiss), "report-metadata --source pde.approval w1:p1 --clear-title") {
		t.Errorf("dismissing kept the sidebar title: %q", callsAfterDismiss)
	}
	if want := []string{"Wallace: none waiting"}; !slices.Equal(answered, want) {
		t.Errorf("after the answer: %q, want %q", answered, want)
	}
}

// Wallace can be asleep or off the network, and an SSH connection to it can
// hang. The inbox must then name it as not reachable within its time limit
// instead of saying "checking" forever, and must not list a machine the user
// disabled.
func TestHungMachineIsNamedNotReachable(t *testing.T) {
	approvaltest.Replace(t, &machineTimeout, 200*time.Millisecond)
	// sleep stands in for a hung ssh: killing herdr leaves it holding the
	// output open.
	approvaltest.FakeHerdr(t, `case "$*" in
"machine list --json") echo '[{"id":"id-lab","label":"Lab","enabled":true},{"id":"id-off","label":"Off","enabled":false}]' ;;
"--machine id-lab"*) sleep 10 ;;
esac`)

	start := time.Now()
	lines := otherMachineLines()
	elapsed := time.Since(start)

	if want := []string{"Lab: not reachable"}; !slices.Equal(lines, want) {
		t.Errorf("lines = %q, want %q", lines, want)
	}
	if elapsed > 5*time.Second {
		t.Errorf("check took %v", elapsed)
	}
}
