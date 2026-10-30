package cmd

import (
	"os"
	"path/filepath"
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
			name:   "new wording",
			script: `printf 'Returns the total, in cents.\n\n' > "$1.new"; ` + keepMarkerAndCode,
			want: &review.Answer{
				Action: review.ActionEdited, Text: "// Returns the total, in cents.\n", Batch: 1,
			},
			wantShown: "+// Returns the total, in cents.",
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
				v := &popup.View{Requests: saved.ListFeedbackRequests(), Repo: r.Repo(), Cols: 80, Rows: 40}
				v.Show(1, time.Now())
				screen := strings.Join(v.BuildScreen(), "\n")
				if !strings.Contains(screen, tt.wantShown) {
					t.Errorf("item screen does not show %q:\n%s", tt.wantShown, screen)
				}
			}
		})
	}
}
