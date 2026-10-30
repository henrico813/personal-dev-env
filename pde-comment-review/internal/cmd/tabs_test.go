package cmd

import (
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// The owner's wording replaces the comment when the review is applied, so
// the edit tab must save exactly what was meant. Closing nvim without a
// change must not turn the AI's wording into the owner's. Without the
// marker line, the code below it would become part of the comment.
func TestEditSavesOwnerWordingAsAnswer(t *testing.T) {
	// Copies the marker line and the code below it to the end of the new page.
	const keepMarkerAndCode = `sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"`
	tests := []struct {
		name       string
		script     string // runs as nvim, with the edit page as $1
		want       *review.Answer
		wantShown  string // a + line the request screen shows afterwards
		wantRefuse bool
	}{
		{
			name: "new wording",
			// The edit page starts with the AI's proposal as plain sentences,
			// without the // markers.
			script: `head -n 1 "$1" | grep -qx 'Returns the sum of prices, in cents.' || exit 1; ` +
				`printf 'Returns the total of every price in the cart, in cents, rounded down to whole cents.\n\n' > "$1.new"; ` + keepMarkerAndCode,
			want: &review.Answer{
				Action: review.ActionEdited,
				Text:   "// Returns the total of every price in the cart, in cents, rounded down to\n// whole cents.\n",
				Batch:  1,
			},
			// The screen wraps these lines at 37 columns, not where the saved
			// comment wraps, so it used the checker's paragraphs.
			wantShown: "+// in the cart, in cents, rounded\n+// down to whole cents.\n func Total(prices []int) int {",
		},
		{
			name:   "every word deleted",
			script: `printf '\n' > "$1.new"; ` + keepMarkerAndCode,
			want:   &review.Answer{Action: review.ActionCut, Batch: 1},
		},
		{name: "closed without a change", script: "true"},
		{name: "marker deleted", script: `printf 'Returns the total.\n' > "$1"`, wantRefuse: true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buildRepoCheckerOnPath(t)
			r := startReview(t, cartGo, `{"approved": [], "avoid": {}}`, nil)
			bin := t.TempDir()
			// nvim is run as: nvim -c COMMANDS PAGE.
			writeFile(t, filepath.Join(bin, "nvim"), "#!/bin/sh\nshift 2\n"+tt.script+"\n")
			if err := os.Chmod(filepath.Join(bin, "nvim"), 0o755); err != nil {
				t.Fatal(err)
			}
			t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))

			err := editComment(r, rewordTotal.ID, os.Stdin, os.Stdout)

			if (err != nil) != tt.wantRefuse {
				t.Fatalf("editComment() error = %v, want refused %v", err, tt.wantRefuse)
			}
			saved, err := review.OpenReview("cart")
			if err != nil {
				t.Fatal(err)
			}
			got, ok := saved.Answers[rewordTotal.ID]
			if tt.want == nil && ok {
				t.Errorf("saved %+v, want nothing saved", got)
			}
			if tt.want != nil && got != *tt.want {
				t.Errorf("saved %+v, want %+v", got, *tt.want)
			}
			// The owner checks an edit on the request screen, which must show
			// the owner's wording in place of the AI's.
			if tt.wantShown != "" {
				v := &popup.View{
					Requests: saved.ListFeedbackRequests(),
					Repo:     r.Repo(),
					Cols:     37,
					Rows:     40,
				}
				askCheckerForCommentLayout(saved, v)
				v.Show(1, time.Now())
				screen := regexp.MustCompile(`\x1b\[[0-9;]*m`).ReplaceAllString(strings.Join(v.BuildScreen(), "\n"), "")
				if !strings.Contains(screen, tt.wantShown) {
					t.Errorf("item screen does not show %q:\n%s", tt.wantShown, screen)
				}
			}
		})
	}
}

// Retyping on a phone is slow, so text the checker refuses opens again on the
// next edit.
func TestRefusedEditReopensOwnerText(t *testing.T) {
	keepTax := review.FeedbackRequest{
		ID:         1,
		File:       "tax.py",
		Name:       "tax",
		Suggestion: review.SuggestKeep,
		Old:        "    \"\"\"Return the tax.\"\"\"\n",
	}
	// The fake nvim types closing quotes the first time. The second time it
	// finds them in the file and fixes them.
	script := `if head -n 1 "$1" | grep -qF '"""tax"""'; then ` +
		`printf 'Return the whole tax.\n\n' > "$1.new"; ` +
		`else printf 'Return the """tax""".\n\n' > "$1.new"; fi; ` +
		`sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"`
	r := startTaxReview(
		t,
		"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n    return 0\n",
		keepTax,
		script,
	)

	refused := editComment(r, keepTax.ID, os.Stdin, os.Stdout)
	fixed := editComment(r, keepTax.ID, os.Stdin, os.Stdout)

	if refused == nil || !strings.Contains(refused.Error(), "your text is kept") {
		t.Fatalf("first editComment() error = %v, want the text kept", refused)
	}
	if fixed != nil {
		t.Errorf("second editComment() error = %v, want the fixed text saved", fixed)
	}
}

// An owner who edits the AI's reword keeps its r prefix. Without it, the tool
// would refuse the owner's backslash when it writes the files.
func TestEditKeepsRawPrefixOfReword(t *testing.T) {
	rewordTax := review.FeedbackRequest{
		ID:         1,
		File:       "tax.py",
		Name:       "tax",
		Suggestion: review.SuggestReword,
		Old:        "    \"\"\"Return the tax.\"\"\"\n",
		New:        "    r\"\"\"Return the tax at C:\\rates.\"\"\"\n",
	}
	script := `printf 'Return the tax at C:\\rates, rounded down.\n\n' > "$1.new"; ` +
		`sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"`
	r := startTaxReview(
		t,
		"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n    return 0\n",
		rewordTax,
		script,
	)

	err := editComment(r, rewordTax.ID, os.Stdin, os.Stdout)

	if err != nil {
		t.Fatalf("editComment() error = %v, want the text saved", err)
	}
	saved, err := review.OpenReview("tax")
	if err != nil {
		t.Fatal(err)
	}
	want := review.Answer{
		Action: review.ActionEdited,
		Text:   "    r\"\"\"Return the tax at C:\\rates, rounded down.\"\"\"\n",
		Batch:  1,
	}
	if got := saved.Answers[rewordTax.ID]; got != want {
		t.Errorf("saved %+v, want %+v", got, want)
	}
}

// Deleting a docstring that is its function's only statement breaks the file.
// The tool would find this only when it writes the files after the last
// comment, so c and an empty edit refuse it at once.
func TestBodyOnlyDocstringCannotBeCut(t *testing.T) {
	tests := []struct {
		name        string
		requestName string // empty when the comment is found by its text
	}{
		{
			name:        "drafted request",
			requestName: "tax",
		},
		{
			name:        "hand-written request without a name",
			requestName: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			keepTax := review.FeedbackRequest{
				ID:         1,
				File:       "tax.py",
				Name:       tt.requestName,
				Suggestion: review.SuggestKeep,
				Old:        "    \"\"\"Return the tax.\"\"\"\n",
			}
			// The fake nvim deletes every word the first time. The second time
			// it fails unless the file starts with the docstring's text again.
			script := `if [ -e "$0.ran" ]; then head -n 1 "$1" | grep -qx 'Return the tax.' || exit 1; ` +
				`else touch "$0.ran"; printf '\n' > "$1.new"; sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"; fi`
			r := startTaxReview(
				t,
				"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n",
				keepTax,
				script,
			)
			v := &popup.View{
				Requests: r.ListFeedbackRequests(),
				Repo:     r.Repo(),
				Cols:     37,
				Rows:     40,
			}
			askCheckerForCommentLayout(r, v)

			pressed := v.PressKey("c", time.Now())
			emptied := editComment(r, keepTax.ID, os.Stdin, os.Stdout)
			reopened := editComment(r, keepTax.ID, os.Stdin, os.Stdout)

			if pressed != popup.None {
				t.Errorf("c gave action %v, want none", pressed)
			}
			if emptied == nil || !strings.Contains(emptied.Error(), "only statement") {
				t.Errorf("editComment() error = %v, want the cut refused", emptied)
			}
			if reopened != nil {
				t.Errorf("second editComment() error = %v, want the docstring's wording back", reopened)
			}
			saved, err := review.OpenReview("tax")
			if err != nil {
				t.Fatal(err)
			}
			if got, ok := saved.Answers[keepTax.ID]; ok {
				t.Errorf("saved %+v, want no answer", got)
			}
		})
	}
}

// Saves a one-comment review of tax.py in a new repository. A fake nvim
// first on PATH runs the given shell script on the file it opens.
func startTaxReview(t *testing.T, code string, request review.FeedbackRequest, script string) *review.Review {
	t.Helper()
	buildRepoCheckerOnPath(t)
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	repo := t.TempDir()
	writeFile(t, filepath.Join(repo, "tax.py"), code)
	batch := review.Batch{
		Version:  review.Version,
		Repo:     repo,
		Requests: []review.FeedbackRequest{request},
	}
	if _, err := review.AddBatch("tax", batch); err != nil {
		t.Fatal(err)
	}
	r, err := review.OpenReview("tax")
	if err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	writeFile(t, filepath.Join(bin, "nvim"), "#!/bin/sh\nshift 2\n"+script+"\n")
	if err := os.Chmod(filepath.Join(bin, "nvim"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	return r
}
