package cmd

import (
	"os"
	"strings"
	"time"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// Shows review name one item per screen until the owner quits. Each answer
// and note is saved when its key is pressed, so the owner can stop and come
// back later.
func runReviewScreen(name string, in, out *os.File, settle time.Duration) error {
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
	v := &popup.View{
		Entries: r.ListEntries(),
		Repo:    r.Repo(),
		Origin:  origin,
		Settle:  settle,
	}
	term, err := popup.Start(in, out)
	if err != nil {
		return err
	}
	defer term.Restore()
	v.Cols, v.Rows = term.Size()
	v.ShowNextUndecided(0, time.Now())
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
		switch v.PressKey(key, time.Now()) {
		case popup.Quit:
			return nil
		case popup.Keep:
			err = saveAnswerAndShowNext(r, v, review.ActionKeep)
		case popup.Cut:
			err = saveAnswerAndShowNext(r, v, review.ActionCut)
		case popup.Accept:
			err = saveAnswerAndShowNext(r, v, review.ActionAccept)
		case popup.SaveNote:
			err = r.SetNote(v.Entry().Item.ID, strings.TrimSpace(v.Typed))
			v.Entries = r.ListEntries()
		}
		if err != nil {
			return err
		}
	}
}

func saveAnswerAndShowNext(r *review.Review, v *popup.View, action string) error {
	if err := r.SetDecision(v.Entry().Item.ID, action, ""); err != nil {
		return err
	}
	v.Entries = r.ListEntries()
	v.ShowNextUndecided(v.Index+1, time.Now())
	return nil
}
