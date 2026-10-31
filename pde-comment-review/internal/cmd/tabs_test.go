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
				buildNvimTypingScript(`Returns the total of every price in the cart, in cents, rounded down to whole cents.\n\n`),
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
			script: buildNvimTypingScript(`\n`),
			want:   &review.Answer{Action: review.ActionCut, Batch: 1},
		},
		{name: "closed without a change", script: "true"},
		{name: "marker deleted", script: `printf 'Returns the total.\n' > "$1"`, wantRefuse: true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buildRepoCheckerOnPath(t)
			r := startReview(t, cartGo, `{"approved": [], "avoid": {}}`, nil)
			installFakeNvim(
				t,
				tt.script,
			)

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
	r := startTaxReview(
		t,
		"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n    return 0\n",
		keepTax,
		buildNvimTypingScript(`Return the """tax""".\n\n`),
	)
	refused := editComment(
		r,
		keepTax.ID,
		os.Stdin,
		os.Stdout,
	)
	if refused == nil || !strings.Contains(refused.Error(), "your text is kept") {
		t.Fatalf("first editComment() error = %v, want the text kept", refused)
	}
	// The second nvim fails unless the file nvim opens starts with the refused
	// text.
	installFakeNvim(
		t,
		`head -n 1 "$1" | grep -qxF 'Return the """tax""".' || exit 1; `+
			buildNvimTypingScript(`Return the whole tax.\n\n`),
	)

	fixed := editComment(
		r,
		keepTax.ID,
		os.Stdin,
		os.Stdout,
	)

	if fixed != nil {
		t.Errorf(
			"second editComment() error = %v, want the refused text reopened and the fix saved",
			fixed,
		)
	}
}

// An owner who edits the AI's suggested wording keeps its r prefix. Without
// it, the checker would refuse the owner's backslash as soon as the owner
// closes nvim.
func TestEditKeepsRawPrefixOfReword(t *testing.T) {
	rewordTax := review.FeedbackRequest{
		ID:         1,
		File:       "tax.py",
		Name:       "tax",
		Suggestion: review.SuggestReword,
		Old:        "    \"\"\"Return the tax.\"\"\"\n",
		New:        "    r\"\"\"Return the tax at C:\\rates.\"\"\"\n",
	}
	r := startTaxReview(
		t,
		"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n    return 0\n",
		rewordTax,
		buildNvimTypingScript(`Return the tax at C:\\rates, rounded down.\n\n`),
	)

	err := editComment(
		r,
		rewordTax.ID,
		os.Stdin,
		os.Stdout,
	)

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

// The edit page showed only the code after a docstring, so the owner could
// not see which function it describes. The def lines sit below the marker,
// where the page's text is never read as the owner's wording.
func TestEditPageShowsDefBelowMarker(t *testing.T) {
	keepTax := review.FeedbackRequest{
		ID:         1,
		File:       "tax.py",
		Name:       "tax",
		Suggestion: review.SuggestKeep,
		Old:        "    \"\"\"Return the tax.\"\"\"\n",
	}
	page := filepath.Join(t.TempDir(), "page.txt")
	r := startTaxReview(
		t,
		"def tax(\n    price,\n):\n    \"\"\"Return the tax.\"\"\"\n    return 0\n",
		keepTax,
		`cp "$1" '`+page+`'`,
	)

	err := editComment(
		r,
		keepTax.ID,
		os.Stdin,
		os.Stdout,
	)

	if err != nil {
		t.Fatalf("editComment() error = %v", err)
	}
	opened, err := os.ReadFile(page)
	if err != nil {
		t.Fatal(err)
	}
	wantCode := editMarker + "\ndef tax(\n    price,\n):\n    return 0\n"
	if !strings.HasSuffix(string(opened), wantCode) {
		t.Errorf(
			"page ends %q, want %q",
			opened,
			wantCode,
		)
	}
}

// Deleting a docstring that is its function's only statement breaks the file.
// The tool would find this only when it writes the files, so pressing c or
// emptying the text in nvim refuses it at once.
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
			r := startTaxReview(
				t,
				"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n",
				keepTax,
				buildNvimTypingScript(`\n`),
			)
			v := &popup.View{
				Requests: r.ListFeedbackRequests(),
				Repo:     r.Repo(),
				Cols:     37,
				Rows:     40,
			}
			askCheckerForCommentLayout(
				r,
				v,
			)

			pressed := v.PressKey("c", time.Now())
			emptied := editComment(
				r,
				keepTax.ID,
				os.Stdin,
				os.Stdout,
			)

			if pressed != popup.None {
				t.Errorf("c gave action %v, want none", pressed)
			}
			if emptied == nil || !strings.Contains(emptied.Error(), "only statement") {
				t.Errorf("editComment() error = %v, want the cut refused", emptied)
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

// After the owner empties a docstring that cannot be cut, the next edit opens
// its wording again. An empty file would make the owner retype it on a phone.
func TestRefusedCutReopensDocstringWording(t *testing.T) {
	keepTax := review.FeedbackRequest{
		ID:         1,
		File:       "tax.py",
		Name:       "tax",
		Suggestion: review.SuggestKeep,
		Old:        "    \"\"\"Return the tax.\"\"\"\n",
	}
	r := startTaxReview(
		t,
		"def tax(price):\n    \"\"\"Return the tax.\"\"\"\n",
		keepTax,
		buildNvimTypingScript(`\n`),
	)
	emptied := editComment(
		r,
		keepTax.ID,
		os.Stdin,
		os.Stdout,
	)
	if emptied == nil {
		t.Fatal("first editComment() saved the empty docstring, want it refused")
	}
	// The second nvim fails unless the file nvim opens starts with the
	// wording.
	installFakeNvim(
		t,
		`head -n 1 "$1" | grep -qx 'Return the tax.'`,
	)

	reopened := editComment(
		r,
		keepTax.ID,
		os.Stdin,
		os.Stdout,
	)

	if reopened != nil {
		t.Errorf(
			"second editComment() error = %v, want the docstring's wording",
			reopened,
		)
	}
}

// Returns a fake nvim script that types the given printf text, such as
// 'Sum.\n', in place of the comment. The script keeps the "# ----" line and
// the code under it.
func buildNvimTypingScript(text string) string {
	return `printf '` + text + `' > "$1.new"; ` +
		`sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"`
}

// Puts a fake nvim first on PATH, which runs the given shell script with the
// opened file as $1. A later call replaces the earlier script.
func installFakeNvim(t *testing.T, script string) {
	t.Helper()
	bin := t.TempDir()
	// nvim is run as nvim -c COMMANDS PAGE.
	nvim := filepath.Join(bin, "nvim")
	writeFile(
		t,
		nvim,
		"#!/bin/sh\nshift 2\n"+script+"\n",
	)
	if err := os.Chmod(nvim, 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv(
		"PATH",
		bin+string(os.PathListSeparator)+os.Getenv("PATH"),
	)
}

// Saves a one-comment review of tax.py in a new repository, with a fake nvim
// that runs the given shell script.
func startTaxReview(
	t *testing.T,
	code string,
	request review.FeedbackRequest,
	script string,
) *review.Review {
	t.Helper()
	buildRepoCheckerOnPath(t)
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	repo := t.TempDir()
	writeFile(
		t,
		filepath.Join(repo, "tax.py"),
		code,
	)
	batch := review.Batch{
		Version:  review.Version,
		Repo:     repo,
		Requests: []review.FeedbackRequest{request},
	}
	_, err := review.AddBatch(
		"tax",
		batch,
	)
	if err != nil {
		t.Fatal(err)
	}
	r, err := review.OpenReview("tax")
	if err != nil {
		t.Fatal(err)
	}
	installFakeNvim(
		t,
		script,
	)
	return r
}
