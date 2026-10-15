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

// Returns the comment checker in this checkout, not the installed copy,
// so the tests run against the checker from the same commit.
func findRepoChecker(t *testing.T) string {
	t.Helper()
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 is not on PATH; the comment checker needs it")
	}
	checker, err := filepath.Abs(filepath.Join("..", "..", "..", "ai", "skills", "code-documentation", "scripts", "check_comments.py"))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(checker); err != nil {
		t.Fatal(err)
	}
	return checker
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

// Apply must approve the owner's answers so agents cannot change them.
// Agents have put back cut comments, so a cut is recorded as "" even
// when never committed, and the renamed Check keeps its approval.
func TestApplyWritesAndApprovesOwnerDecisions(t *testing.T) {
	checker := findRepoChecker(t)
	r := startReview(t, cartGo, `{"approved": [{"file": "cart.go", "name": "Check", "text": "// Reports whether every price is above zero.\n"}], "avoid": {}}`, []string{"Check=Valid"})
	setDecisions(t, r, map[int]review.Decision{
		1: {Action: review.ActionAccept},
		2: {Action: review.ActionAccept},
		3: {Action: review.ActionEdited, Text: "// Reports whether price is over the 1000 cent limit.\n"},
	})

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
		t.Errorf("cart.go =\n%s\nwant\n%s", got, want)
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
		t.Errorf(".code-comments.json approves %q, want %q", got, wantApproved)
	}
}

// applyDecisions writes the owner's answers into the code. It cannot run
// again on comments it already changed, so a failed check must say why
// and change nothing. An early draft approved comments the owner never saw.
func TestApplyChangesNothingWhenChecksFail(t *testing.T) {
	approveValid := `{"approved": [{"file": "cart.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"},
		{"file": "cart.go", "name": "Reset", "text": ""}], "avoid": {}}`
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
			want:   "item 3: cart.go: comment found 0 times",
		},
		{
			// A file with Windows line endings is refused this way too.
			name:   "file gofmt would reformat",
			onDisk: strings.Replace(cartGo, "Max = 50", "Max  =  50", 1),
			want:   "gofmt would change cart.go",
		},
		{
			name:   "approved comment changed",
			onDisk: strings.Replace(cartGo, "every price is above zero", "no price is negative", 1),
			want:   "cart.go Valid: differs from the text the owner approved",
		},
		{
			// Only a renamed comment may pass by matching an item.
			name:   "approved comment changed, in review",
			onDisk: strings.Replace(cartGo, "every price is above zero", "no price is negative", 1),
			extra: []review.Item{{ID: 4, File: "cart.go", Proposal: review.ProposeKeep,
				Old: "// Reports whether no price is negative.\n"}},
			want: "cart.go Valid: differs from the text the owner approved",
		},
		{
			name:   "approved comment deleted",
			onDisk: strings.Replace(cartGo, "// Reports whether every price is above zero.\n", "", 1),
			want:   "cart.go Valid: the owner's approved comment is gone",
		},
		{
			name:    "renamed approved comment rewritten",
			onDisk:  strings.Replace(cartGo, "// Reports whether every price is above zero.\nfunc Valid", "// Always returns true; safe to skip.\nfunc Check", 1),
			renames: []string{"Valid=Check"},
			want:    "cart.go Check: differs from the text the owner approved",
		},
		{
			name:   "unreviewed comment added",
			onDisk: cartGo + "\n// Deletes every order.\nfunc Clear() {}\n",
			want:   "cart.go Clear: not in the review",
		},
		{
			name:   "cut comment put back",
			onDisk: cartGo + "\n// Empties the cart.\nfunc Reset() {}\n",
			want:   "cart.go Reset: the owner cut this comment",
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
			checker := findRepoChecker(t)
			if tt.crash {
				checker = filepath.Join(t.TempDir(), "crash.py")
				writeFile(t, checker, "raise RuntimeError('bug')\n")
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
