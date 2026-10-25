package cmd

import (
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"pde-comment-review/internal/review"
)

const cartGo = `package cart

const (
	Max = 50
	// Prices are in cents, so sums never round.
	Currency = "USD"
)

// Sums the prices.
func Total(prices []int) int { return 0 }

// Reports whether a price is too high.
func TooHigh(price int) bool { return price > 1000 }

// Reports whether every price is above zero.
func Valid(prices []int) bool { return true }
`

var (
	cutCurrency = review.Item{ID: 1, File: "cart.go", Proposal: review.ProposeCut,
		Old: "\t// Prices are in cents, so sums never round.\n"}
	rewordTotal = review.Item{ID: 2, File: "cart.go", Proposal: review.ProposeReword,
		Old: "// Sums the prices.\n", New: "// Returns the sum of prices, in cents.\n"}
	keepTooHigh = review.Item{ID: 3, File: "cart.go", Proposal: review.ProposeKeep,
		Old: "// Reports whether a price is too high.\n"}
)

// Builds this checkout's comment checker and puts it first on PATH, so the
// tests never run an installed copy. Returns the skill's script, which runs
// that checker.
func buildRepoCheckerOnPath(t *testing.T) string {
	t.Helper()
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 is not on PATH; the comment checker needs it")
	}
	bin := t.TempDir()
	checker := filepath.Join(
		bin,
		"pde-comment-checker",
	)
	build := exec.Command(
		"python3",
		"-I",
		"build.py",
		checker,
	)
	build.Dir = filepath.Join(
		"..",
		"..",
		"..",
		"pde-comment-checker",
	)
	if output, err := build.CombinedOutput(); err != nil {
		t.Fatalf(
			"build the comment checker: %v: %s",
			err,
			output,
		)
	}
	t.Setenv(
		"PATH",
		bin+string(os.PathListSeparator)+os.Getenv("PATH"),
	)
	return checker
}

// Real reviews run the checker pde-installer put on PATH, not a copy in
// the skills folder, so a fixed path would miss the installed checker.
func TestCheckerFoundOnPath(t *testing.T) {
	installed := t.TempDir()
	checker := filepath.Join(
		installed,
		"pde-comment-checker",
	)
	err := os.WriteFile(
		checker,
		[]byte("#!/bin/sh\necho '{\"interface\": 1}'\n"),
		0o755,
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv(
		"PATH",
		installed,
	)

	got, err := findCheckerPath()

	if err != nil || got != checker {
		t.Fatalf(
			"findCheckerPath() = %q, %v, want %q",
			got,
			err,
			checker,
		)
	}
}

// The screen shows this error after the owner's last answer, so it must
// say how to install the checker.
func TestMissingCheckerNamesInstaller(t *testing.T) {
	t.Setenv(
		"PATH",
		t.TempDir(),
	)

	_, err := findCheckerPath()

	if err == nil || !strings.Contains(err.Error(), "run pde-installer") {
		t.Fatalf(
			"findCheckerPath() error = %v, want one naming pde-installer",
			err,
		)
	}
}

// A checker from another install may read the tool's requests differently,
// so the tool must stop before asking it anything and say how to fix it.
func TestCheckerOfOtherInterfaceIsRefused(t *testing.T) {
	installed := t.TempDir()
	err := os.WriteFile(
		filepath.Join(
			installed,
			"pde-comment-checker",
		),
		[]byte("#!/bin/sh\necho '{\"interface\": 2, \"version\": \"3.0.0\"}'\n"),
		0o755,
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv(
		"PATH",
		installed,
	)

	_, err = findCheckerPath()

	if err == nil || !strings.Contains(err.Error(), "speaks interface 2, not 1") || !strings.Contains(err.Error(), "run pde-installer") {
		t.Fatalf(
			"findCheckerPath() error = %v, want one naming both interfaces and pde-installer",
			err,
		)
	}
}

// A broken checker that fails with an empty problems list must not pass as a
// reply, or draft would save a review with nothing for the owner to answer.
func TestCheckerFailureWithoutReasonsIsRefused(t *testing.T) {
	checker := filepath.Join(
		t.TempDir(),
		"pde-comment-checker",
	)
	err := os.WriteFile(
		checker,
		[]byte("#!/bin/sh\necho '{\"interface\": 1, \"items\": [], \"problems\": []}'\nexit 1\n"),
		0o755,
	)
	if err != nil {
		t.Fatal(err)
	}
	var reply struct{}

	err = runCheckerCommand(
		checker,
		&reply,
		"changes",
	)

	if err == nil || !strings.Contains(err.Error(), "no problems in its reply") {
		t.Fatalf(
			"runCheckerCommand() error = %v, want the empty refusal refused",
			err,
		)
	}
}

func startReview(t *testing.T, code, settings string, renames []string, extra ...review.Item) *review.Review {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	repo := t.TempDir()
	writeFile(t, filepath.Join(repo, "cart.go"), code)
	writeFile(t, filepath.Join(repo, ".code-comments.json"), settings)
	p := review.Proposals{Version: review.Version, Repo: repo, Renames: renames,
		Items: append([]review.Item{cutCurrency, rewordTotal, keepTooHigh}, extra...)}
	if _, err := review.AddRound("cart", p); err != nil {
		t.Fatal(err)
	}
	r, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	return r
}

func writeFile(t *testing.T, path, text string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
}

func setDecisions(t *testing.T, r *review.Review, decisions map[int]review.Decision) {
	t.Helper()
	for id, d := range decisions {
		if err := r.SetDecision(id, d.Action, d.Text); err != nil {
			t.Fatal(err)
		}
	}
}

// Verifies that only answered comments are approved, so a comment the
// owner never saw stays unlocked.
func TestApplyApprovesOnlyAnsweredComments(t *testing.T) {
	checker := buildRepoCheckerOnPath(t)
	// No item shows Clear's comment, so it must stay unapproved.
	unanswered := "\n// Deletes every order.\nfunc Clear() {}\n"
	r := startReview(
		t,
		cartGo+unanswered,
		validApproval,
		nil,
	)
	setDecisions(
		t,
		r,
		map[int]review.Decision{
			1: {Action: review.ActionAccept},
			2: {Action: review.ActionAccept},
			3: {Action: review.ActionKeep},
		},
	)

	_, err := writeAndApproveDecisions(r, checker)

	if err != nil {
		t.Fatalf("writeAndApproveDecisions() error = %v", err)
	}
	approved, err := readApprovals(r.Repo())
	if err != nil {
		t.Fatal(err)
	}
	if text, ok := approved[commentKey{"cart.go", "Clear"}]; ok {
		t.Errorf("Clear's comment, which no item showed, is approved as %q", text)
	}
}

// Verifies that the owner's answers reach the code and are approved, cuts
// and renames included.
func TestApplyWritesOwnerAnswers(t *testing.T) {
	checker := buildRepoCheckerOnPath(t)
	r := startReview(
		t,
		cartGo,
		`{"approved": [{"file": "cart.go", "name": "Check", "text": "// Reports whether every price is above zero.\n"}], "avoid": {}}`,
		[]string{"Check=Valid"},
	)
	setDecisions(
		t,
		r,
		map[int]review.Decision{
			1: {Action: review.ActionAccept},
			2: {Action: review.ActionAccept},
			3: {Action: review.ActionEdited, Text: "// Reports whether price is over the 1000 cent limit.\n"},
		},
	)

	_, err := writeAndApproveDecisions(r, checker)

	if err != nil {
		t.Fatalf("writeAndApproveDecisions() error = %v", err)
	}
	// gofmt realigns the const block once the comment inside it is cut.
	want := `package cart

const (
	Max      = 50
	Currency = "USD"
)

// Returns the sum of prices, in cents.
func Total(prices []int) int { return 0 }

// Reports whether price is over the 1000 cent limit.
func TooHigh(price int) bool { return price > 1000 }

// Reports whether every price is above zero.
func Valid(prices []int) bool { return true }
`
	if got, _ := os.ReadFile(filepath.Join(r.Repo(), "cart.go")); string(got) != want {
		t.Errorf(
			"cart.go =\n%s\nwant\n%s",
			got,
			want,
		)
	}
	got, err := readApprovals(r.Repo())
	if err != nil {
		t.Fatal(err)
	}
	wantApproved := map[commentKey]string{
		{"cart.go", "Currency"}: "",
		{"cart.go", "Total"}:    "// Returns the sum of prices, in cents.\n",
		{"cart.go", "TooHigh"}:  "// Reports whether price is over the 1000 cent limit.\n",
		{"cart.go", "Valid"}:    "// Reports whether every price is above zero.\n",
	}
	if !reflect.DeepEqual(got, wantApproved) {
		t.Errorf(
			".code-comments.json approves %q, want %q",
			got,
			wantApproved,
		)
	}
}

// Verifies that a failed check refuses the whole review and writes
// nothing, so nothing unseen is approved.
func TestApplyRefusesReviewOnFailedCheck(t *testing.T) {
	approveValid := `{"approved": [{"file": "cart.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"},
		{"file": "cart.go", "name": "Reset", "text": ""},
		{"file": "tax.go", "name": "Check", "text": "// Checks the tax.\n"}], "avoid": {}}`
	tests := []struct {
		name       string
		onDisk     string // cart.go when apply runs
		renames    []string
		extra      []review.Item // items after the three every row has; the owner keeps them
		crash      bool          // a checker that raises an exception, as Python does on a bug
		reproposed bool          // the AI sends item 2 again after the owner decided it
		want       string
	}{
		{
			name:       "item proposed again",
			onDisk:     cartGo,
			reproposed: true,
			want:       "not decided: items 2",
		},
		{
			// Approving it would approve wording the owner never saw.
			name:   "kept comment edited after review",
			onDisk: strings.Replace(cartGo, "a price is too high", "a price is over the limit", 1),
			want:   "cart.go: item 3 changed in the file after it was added",
		},
		{
			// A file with Windows line endings is refused this way too.
			name:   "file gofmt would reformat",
			onDisk: strings.Replace(cartGo, "Max = 50", "Max  =  50", 1),
			want:   "gofmt would change cart.go",
		},
		{
			// Listing an edited approved comment must not approve the edit.
			name:   "approved comment changed, in review",
			onDisk: strings.Replace(cartGo, "every price is above zero", "no price is negative", 1),
			extra: []review.Item{{ID: 4, File: "cart.go", Proposal: review.ProposeKeep,
				Old: "// Reports whether no price is negative.\n"}},
			want: "cart.go Valid: differs from the text the owner approved",
		},
		{
			// Without its rename, an edit to it would pass as a new comment.
			name:   "approved comment deleted",
			onDisk: strings.Replace(cartGo, "// Reports whether every price is above zero.\n", "", 1),
			want:   "cart.go Valid: the owner's approved comment is gone",
		},
		{
			name:    "renamed approved comment rewritten, in review",
			onDisk:  strings.Replace(cartGo, "// Reports whether every price is above zero.\nfunc Valid", "// Always returns true; safe to skip.\nfunc Check", 1),
			renames: []string{"Valid=Check"},
			extra: []review.Item{{
				ID:       4,
				File:     "cart.go",
				Proposal: review.ProposeKeep,
				Old:      "// Always returns true; safe to skip.\n",
			}},
			want: "cart.go Check: differs from the text the owner approved",
		},
		{
			// A rename once reported success without moving an approved comment,
			// leaving the checker unable to find it later.
			name:    "rename that moves nothing",
			onDisk:  cartGo,
			renames: []string{"Check=Sum"},
			want:    "rename Check would move no approved comment",
		},
		{
			// The write would approve the named comment, not the one shown.
			name:   "item named after another comment",
			onDisk: cartGo,
			extra: []review.Item{{
				ID:       4,
				File:     "cart.go",
				Name:     "Total",
				Proposal: review.ProposeKeep,
				Old:      "// Reports whether every price is above zero.\n",
			}},
			want: "item 4: cart.go Total: no comment by this name has the item's old",
		},
		{
			name:   "cut comment put back, in review",
			onDisk: cartGo + "\n// Empties the cart.\nfunc Reset() {}\n",
			extra: []review.Item{{
				ID:       4,
				File:     "cart.go",
				Proposal: review.ProposeKeep,
				Old:      "// Empties the cart.\n",
			}},
			want: "cart.go Reset: the owner cut this comment",
		},
		{
			// Python exits 1 on an exception, as it once did on an
			// unreadable .code-comments.json; that is not success.
			name:   "checker crashes",
			onDisk: cartGo,
			crash:  true,
			want:   "comment checker: exit status 1",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			checker := buildRepoCheckerOnPath(t)
			if tt.crash {
				checker = filepath.Join(t.TempDir(), "crash.py")
				err := os.WriteFile(
					checker,
					[]byte("#!/usr/bin/env python3\nraise RuntimeError('bug')\n"),
					0o755,
				)
				if err != nil {
					t.Fatal(err)
				}
			}
			r := startReview(t, tt.onDisk, approveValid, tt.renames, tt.extra...)
			decisions := map[int]review.Decision{
				1: {Action: review.ActionAccept},
				2: {Action: review.ActionAccept},
				3: {Action: review.ActionKeep},
			}
			for _, item := range tt.extra {
				decisions[item.ID] = review.Decision{Action: review.ActionKeep}
			}
			setDecisions(t, r, decisions)
			if tt.reproposed {
				shorter := rewordTotal
				shorter.New = "// Returns the sum.\n"
				if _, err := review.AddRound("cart", review.Proposals{Version: review.Version, Repo: r.Repo(), Items: []review.Item{shorter}}); err != nil {
					t.Fatal(err)
				}
				var err error
				if r, err = review.OpenReview("cart"); err != nil {
					t.Fatal(err)
				}
			}

			_, err := writeAndApproveDecisions(r, checker)

			if err == nil || !strings.Contains(err.Error(), tt.want) || !strings.Contains(err.Error(), "nothing was written") {
				t.Fatalf("writeAndApproveDecisions() error = %v, want it to contain %q and say nothing was written", err, tt.want)
			}
			if got, _ := os.ReadFile(filepath.Join(r.Repo(), "cart.go")); string(got) != tt.onDisk {
				t.Errorf("cart.go changed:\n%s", got)
			}
			if got, _ := os.ReadFile(filepath.Join(r.Repo(), ".code-comments.json")); string(got) != approveValid {
				t.Errorf(".code-comments.json changed:\n%s", got)
			}
		})
	}
}
