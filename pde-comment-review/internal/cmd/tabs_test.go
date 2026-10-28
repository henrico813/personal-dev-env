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
