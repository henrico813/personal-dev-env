package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

// The Moshi phone-approval change deleted the bash diff helper while two calls
// remained, so every `pr edit` failed and no test noticed.
func TestEditReviewShowsCurrentAndRequested(t *testing.T) {
	gh, _ := approvaltest.FakeGH(t, `echo '{"title":"old","body":"same"}'`)
	cases := []struct {
		name  string
		title string
		want  []string
	}{
		{name: "changed title", title: "new", want: []string{"-old", "+new", "No changes (body not in this command)"}},
		{name: "unchanged title", title: "old", want: []string{"Title:\nNo changes\n"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			req := approval.Request{Operation: "edit", Target: "12", Title: tc.title, HasTitle: true}

			text, _, err := approvalText(gh, req)

			if err != nil {
				t.Fatal(err)
			}
			for _, want := range tc.want {
				if !strings.Contains(text, want) {
					t.Errorf("review is missing %q:\n%s", want, text)
				}
			}
		})
	}
}

// An empty PATH entry, as a stray colon leaves, names the working directory:
// the agent's checkout. A gh there must never be run with PDE_GH_WRITE=1
// after an approval, and must not hide the guard so the guard runs instead.
func TestRealGHIgnoresEmptyPATHEntry(t *testing.T) {
	dir := t.TempDir()
	for _, name := range []string{"checkout", "guard", "real"} {
		if err := os.Mkdir(filepath.Join(dir, name), 0o700); err != nil {
			t.Fatal(err)
		}
		approvaltest.WriteScript(t, filepath.Join(dir, name), "gh", "exit 0")
	}
	chdir(t, filepath.Join(dir, "checkout"))
	t.Setenv("PATH", ":"+filepath.Join(dir, "guard")+":"+filepath.Join(dir, "real"))

	gh := findRealGH()

	if want := filepath.Join(dir, "real", "gh"); gh != want {
		t.Errorf("findRealGH() = %q, want %q", gh, want)
	}
}

// A fetch failure must be shown to the approver instead of blocking the write.
func TestEditReviewShowsFetchFailure(t *testing.T) {
	gh, _ := approvaltest.FakeGH(t, "echo boom >&2; exit 1")
	req := approval.Request{Operation: "edit", Target: "12", Title: "new", HasTitle: true}

	text, _, err := approvalText(gh, req)

	if err != nil || !strings.Contains(text, "Fetch failed: boom\n") {
		t.Fatalf("review = %q, %v; want the fetch error in the text", text, err)
	}
}

// The fetch runs under the request lock; a hung gh would block every rerun.
func TestEditReviewStopsHungFetch(t *testing.T) {
	// The fake gh never answers; the shortened timeout keeps the test fast.
	gh, _ := approvaltest.FakeGH(t, "exec sleep 10")
	oldTimeout := fetchTimeout
	fetchTimeout = 100 * time.Millisecond
	t.Cleanup(func() { fetchTimeout = oldTimeout })
	req := approval.Request{Operation: "edit", Target: "12", Title: "new", HasTitle: true}

	text, _, err := approvalText(gh, req)

	if err != nil || !strings.Contains(text, "Fetch failed: gh pr view timed out") {
		t.Fatalf("review = %q, %v; want the timeout in the text", text, err)
	}
}
