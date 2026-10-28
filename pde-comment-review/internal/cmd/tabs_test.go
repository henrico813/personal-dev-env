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
		want       *review.Decision
		wantShown  string // a + line the item screen shows afterwards
		wantRefuse bool
	}{
		{
			name: "new wording",
			// The page starts from the AI's reword as plain text.
			script: `head -n 1 "$1" | grep -qx 'Returns the sum of prices, in cents.' || exit 1; ` +
				`printf 'Returns the total of every price in the cart, in cents, rounded down to whole cents.\n\n' > "$1.new"; ` + keepMarkerAndCode,
			want: &review.Decision{
				Action: review.ActionEdited,
				Text:   "// Returns the total of every price in the cart, in cents, rounded down to\n// whole cents.\n",
				Round:  1,
			},
			// The phone screen rewraps the two lines above the code.
			wantShown: "+// in the cart, in cents, rounded\n+// down to whole cents.\n func Total(prices []int) int {",
		},
		{
			name:   "every word deleted",
			script: `printf '\n' > "$1.new"; ` + keepMarkerAndCode,
			want:   &review.Decision{Action: review.ActionCut, Round: 1},
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
			got, ok := saved.Decisions[rewordTotal.ID]
			if tt.want == nil && ok {
				t.Errorf("saved %+v, want nothing saved", got)
			}
			if tt.want != nil && got != *tt.want {
				t.Errorf("saved %+v, want %+v", got, *tt.want)
			}
			// The owner checks an edit on the item screen, which must show
			// the owner's wording in place of the AI's.
			if tt.wantShown != "" {
				v := &popup.View{
					Entries: saved.ListEntries(),
					Repo:    r.Repo(),
					Cols:    37,
					Rows:    40,
				}
				prepareScreen(saved, v)
				v.Show(1, time.Now())
				screen := regexp.MustCompile(`\x1b\[[0-9;]*m`).ReplaceAllString(strings.Join(v.BuildScreen(), "\n"), "")
				if !strings.Contains(screen, tt.wantShown) {
					t.Errorf("item screen does not show %q:\n%s", tt.wantShown, screen)
				}
			}
		})
	}
}

// Retyping on a phone is slow, so text the checker refuses, such as closing
// quotes in a docstring, must open again. The edit must keep the r prefix of
// the wording shown, or the final write would refuse the owner's backslash.
func TestEditKeepsTypingAndShownQuotes(t *testing.T) {
	buildRepoCheckerOnPath(t)
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	repo := t.TempDir()
	writeFile(t, filepath.Join(repo, "tax.py"), "def tax(price):\n    \"\"\"Return the tax.\"\"\"\n    return 0\n")
	rewordTax := review.Item{
		ID:       1,
		File:     "tax.py",
		Name:     "tax",
		Proposal: review.ProposeReword,
		Old:      "    \"\"\"Return the tax.\"\"\"\n",
		New:      "    r\"\"\"Return the tax at C:\\rates.\"\"\"\n",
	}
	proposals := review.Proposals{
		Version: review.Version,
		Repo:    repo,
		Items:   []review.Item{rewordTax},
	}
	if _, err := review.AddRound("tax", proposals); err != nil {
		t.Fatal(err)
	}
	r, err := review.OpenReview("tax")
	if err != nil {
		t.Fatal(err)
	}
	bin := t.TempDir()
	// The first edit types closing quotes, and the second finds them on its
	// page and fixes them.
	script := `if head -n 1 "$1" | grep -qF '"""tax"""'; then ` +
		`printf 'Return the tax at C:\\rates, rounded down.\n\n' > "$1.new"; ` +
		`else printf 'Return the """tax""".\n\n' > "$1.new"; fi; ` +
		`sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"`
	writeFile(t, filepath.Join(bin, "nvim"), "#!/bin/sh\nshift 2\n"+script+"\n")
	if err := os.Chmod(filepath.Join(bin, "nvim"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))

	refused := editComment(r, rewordTax.ID, os.Stdin, os.Stdout)
	fixed := editComment(r, rewordTax.ID, os.Stdin, os.Stdout)

	if refused == nil || !strings.Contains(refused.Error(), "your text is kept") {
		t.Fatalf("first editComment() error = %v, want the text kept", refused)
	}
	if fixed != nil {
		t.Fatalf("second editComment() error = %v, want the fixed text saved", fixed)
	}
	saved, err := review.OpenReview("tax")
	if err != nil {
		t.Fatal(err)
	}
	want := review.Decision{
		Action: review.ActionEdited,
		Text:   "    r\"\"\"Return the tax at C:\\rates, rounded down.\"\"\"\n",
		Round:  1,
	}
	if got := saved.Decisions[rewordTax.ID]; got != want {
		t.Errorf("saved %+v, want %+v", got, want)
	}
}

// A docstring that is its function's whole body cannot be cut, or the file
// stops parsing. The write refuses that cut only after the last answer, when
// the owner can no longer change it, so the screen and the edit page refuse it.
func TestBodyOnlyDocstringCannotBeCut(t *testing.T) {
	tests := []struct {
		name     string
		itemName string // empty for a hand-written item found by its text
	}{
		{
			name:     "drafted item",
			itemName: "tax",
		},
		{
			name:     "hand-written item without a name",
			itemName: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buildRepoCheckerOnPath(t)
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			repo := t.TempDir()
			writeFile(t, filepath.Join(repo, "tax.py"), "def tax(price):\n    \"\"\"Return the tax.\"\"\"\n")
			keepTax := review.Item{
				ID:       1,
				File:     "tax.py",
				Name:     tt.itemName,
				Proposal: review.ProposeKeep,
				Old:      "    \"\"\"Return the tax.\"\"\"\n",
			}
			proposals := review.Proposals{
				Version: review.Version,
				Repo:    repo,
				Items:   []review.Item{keepTax},
			}
			if _, err := review.AddRound("tax", proposals); err != nil {
				t.Fatal(err)
			}
			r, err := review.OpenReview("tax")
			if err != nil {
				t.Fatal(err)
			}
			v := &popup.View{
				Entries: r.ListEntries(),
				Repo:    r.Repo(),
				Cols:    37,
				Rows:    40,
			}
			prepareScreen(r, v)
			bin := t.TempDir()
			// The first edit deletes every word, and the second must open on
			// the docstring's wording again and close unchanged.
			script := `if [ -e "$0.ran" ]; then head -n 1 "$1" | grep -qx 'Return the tax.' || exit 1; ` +
				`else touch "$0.ran"; printf '\n' > "$1.new"; sed -n '/^# ----/,$p' "$1" >> "$1.new"; mv "$1.new" "$1"; fi`
			writeFile(t, filepath.Join(bin, "nvim"), "#!/bin/sh\nshift 2\n"+script+"\n")
			if err := os.Chmod(filepath.Join(bin, "nvim"), 0o755); err != nil {
				t.Fatal(err)
			}
			t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))

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
			if got, ok := saved.Decisions[keepTax.ID]; ok {
				t.Errorf("saved %+v, want no answer", got)
			}
		})
	}
}
