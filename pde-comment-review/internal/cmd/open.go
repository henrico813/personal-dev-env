package cmd

import (
	"os"
	"strings"
	"time"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

func answerReview(name string, in, out *os.File, settle time.Duration) error {
	r, origin, err := openReviewWithOrigin(name)
	if err != nil {
		return err
	}
	return runReviewScreen(r, origin, in, out, settle)
}

// Opens review name with the repository and branch it came from. Reviews
// saved before those were recorded use the checkout as it is now.
func openReviewWithOrigin(name string) (*review.Review, review.Origin, error) {
	r, err := review.OpenReview(name)
	if err != nil {
		return nil, review.Origin{}, err
	}
	origin, err := readReviewOrigin(r)
	if err != nil {
		return nil, review.Origin{}, err
	}
	return r, origin, nil
}

func readReviewOrigin(r *review.Review) (review.Origin, error) {
	origin, err := r.ReadOrigin()
	if err != nil {
		return review.Origin{}, err
	}
	if origin.RepoName == "" {
		origin = findReviewOrigin(r.Repo())
	}
	return origin, nil
}

// Shows review r one item per screen until the owner quits. Each answer
// and note is saved when its key is pressed, so the owner can stop and come
// back later.
func runReviewScreen(
	r *review.Review, origin review.Origin, in, out *os.File, settle time.Duration,
) error {
	v := &popup.View{
		Entries: r.ListEntries(),
		Repo:    r.Repo(),
		Origin:  origin,
		Settle:  settle,
		Renames: r.ListRenames(),
		Files:   r.ListFiles(),
	}
	term, err := popup.Start(in, out)
	if err != nil {
		return err
	}
	defer term.Restore()
	v.Cols, v.Rows = term.Size()
	v.ShowNextUndecided(0, time.Now())
	for {
		key, err := term.DrawAndReadKey(func(cols, rows int) []string {
			v.Cols, v.Rows = cols, rows
			return v.BuildScreen()
		})
		if err != nil {
			return err
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
		case popup.ApplyAll:
			r, err = applyFromScreen(r.Name, v)
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

// Applies the review and returns it as read again, for the screen to use
// from then on. A refusal stays on screen, so the owner can read why.
func applyFromScreen(name string, v *popup.View) (*review.Review, error) {
	checker, err := findCheckerPath()
	if err != nil {
		return nil, err
	}
	// An AI may have added a round since the screen read the review.
	r, err := review.OpenReview(name)
	if err != nil {
		return nil, err
	}
	report, err := writeAndApproveDecisions(r, checker)
	if err != nil {
		// A refusal for an item the new round added opens that item, so
		// the owner can answer it and apply again.
		v.Entries, v.Renames, v.Files = r.ListEntries(), r.ListRenames(), r.ListFiles()
		v.ShowNextUndecided(0, time.Now())
		v.ShowProblem(err.Error())
		return r, nil
	}
	v.ShowResult(report)
	return r, nil
}
