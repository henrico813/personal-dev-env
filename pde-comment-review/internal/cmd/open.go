package cmd

import (
	"os"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// Shows review name one item per screen, read-only, until the owner quits.
func runReviewScreen(name string, in, out *os.File) error {
	r, err := review.OpenReview(name)
	if err != nil {
		return err
	}
	origin, err := r.ReadOrigin()
	if err != nil {
		return err
	}
	if origin.RepoName == "" {
		origin = findReviewOrigin(r.Repo())
	}
	v := &popup.View{Entries: r.ListEntries(), Repo: r.Repo(), Origin: origin}
	term, err := popup.Start(in, out)
	if err != nil {
		return err
	}
	defer term.Restore()
	v.Cols, v.Rows = term.Size()
	v.Show(0)
	redraw := true
	for {
		if cols, rows := term.Size(); redraw || cols != v.Cols || rows != v.Rows {
			v.Cols, v.Rows = cols, rows
			if err := term.Draw(v.Lines()); err != nil {
				return err
			}
		}
		key, err := term.ReadKey()
		if err != nil {
			return err
		}
		redraw = key != ""
		if !redraw {
			continue
		}
		if v.PressKey(key) == popup.Quit {
			return nil
		}
	}
}
