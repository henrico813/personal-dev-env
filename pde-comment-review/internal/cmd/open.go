package cmd

import (
	"os"
	"slices"
	"strconv"
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
	return runReviewScreen(r, origin, in, out, screenOptions{settle: settle})
}

type screenOptions struct {
	settle time.Duration
	// In Herdr's popup, e and d open tabs, since nvim and less need more
	// room than the popup has.
	inPopup bool
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

// Shows review r one item per screen until the owner quits, or until e or
// d in the popup hands the screen to a Herdr tab. Each answer and note is
// saved when its key is pressed, so the owner can stop and come back later.
func runReviewScreen(
	r *review.Review, origin review.Origin, in, out *os.File, opts screenOptions,
) error {
	foundByName, err := readCommentsByName(r)
	v := &popup.View{
		Entries:     r.ListEntries(),
		Repo:        r.Repo(),
		Origin:      origin,
		Settle:      opts.settle,
		FoundByName: foundByName,
	}
	if err != nil {
		v.ShowNotice(err.Error())
	}
	term, err := popup.Start(in, out)
	if err != nil {
		return err
	}
	defer term.Restore()
	v.Cols, v.Rows = term.Size()
	v.ShowNextUndecided(0, time.Now())
	r, err = writeWhenAllAnswered(
		r,
		v,
	)
	if err != nil {
		return err
	}
	for {
		key, err := term.DrawAndReadKey(func(cols, rows int) []string {
			v.Cols, v.Rows = cols, rows
			return v.BuildScreen()
		})
		if err != nil {
			return err
		}
		if r, err = showReviewAsSaved(r, v); err != nil {
			return err
		}
		switch v.PressKey(key, time.Now()) {
		case popup.Quit:
			return nil
		case popup.Keep:
			r, err = saveAnswerAndShowNext(
				r,
				v,
				review.ActionKeep,
			)
		case popup.Cut:
			r, err = saveAnswerAndShowNext(
				r,
				v,
				review.ActionCut,
			)
		case popup.Accept:
			r, err = saveAnswerAndShowNext(
				r,
				v,
				review.ActionAccept,
			)
		case popup.SaveNote:
			err = r.SetNote(v.Entry().Item.ID, strings.TrimSpace(v.Typed))
			v.Entries = r.ListEntries()
		case popup.Edit:
			if !opts.inPopup {
				r, err = editInPlace(
					term,
					r,
					v,
					in,
					out,
				)
			} else if openEditTab(r, v) {
				return nil
			}
		case popup.ShowDiff:
			if !opts.inPopup {
				err = runOutsideScreen(term, v, func() error { return showDiff(r, out) })
			} else if openTabOrShowWhy(v, diffPaneID, map[string]string{reviewNameEnv: r.Name}) {
				return nil
			}
		case popup.ViewFile:
			item := v.Entry().Item
			err = runOutsideScreen(
				term,
				v,
				func() error {
					return viewFile(
						r.Repo(),
						item,
						v.FoundByName,
						in,
						out,
					)
				},
			)
		case popup.Search:
			text := v.Typed
			err = runOutsideScreen(term, v, func() error { return searchRepo(r.Repo(), text, out) })
		}
		if err != nil {
			return err
		}
	}
}

// Reads review r again and keeps the screen on its item, since the popup can
// save to the review while a tab stays open for hours.
func showReviewAsSaved(
	r *review.Review,
	v *popup.View,
) (*review.Review, error) {
	saved, err := review.OpenReview(r.Name)
	if err != nil {
		return r, err
	}
	shown := v.Entry()
	v.Entries = saved.ListEntries()
	for i, e := range v.Entries {
		if e.Item.ID != shown.Item.ID {
			continue
		}
		v.Index = i
		// Showing a new version of the item makes answer keys wait a moment.
		if e.Round != shown.Round {
			v.ShowNewVersion(i, time.Now())
		}
	}
	return saved, nil
}

// Returns each named item's comment as its file has it now, running the
// checker only when r has a named item.
func readCommentsByName(r *review.Review) (map[int]string, error) {
	isNamed := func(e review.Entry) bool { return e.Item.Name != "" }
	if !slices.ContainsFunc(r.ListEntries(), isNamed) {
		return nil, nil
	}
	checker, err := findCheckerPath()
	if err != nil {
		return nil, err
	}
	named, err := readReviewComments(r, checker)
	if err != nil {
		return nil, err
	}
	return findCommentsByName(r, named), nil
}

func openEditTab(r *review.Review, v *popup.View) bool {
	env := map[string]string{reviewNameEnv: r.Name, itemIDEnv: strconv.Itoa(v.Entry().Item.ID)}
	return openTabOrShowWhy(v, editPaneID, env)
}

func editInPlace(
	term *popup.Terminal,
	r *review.Review,
	v *popup.View,
	in *os.File,
	out *os.File,
) (*review.Review, error) {
	id := v.Entry().Item.ID
	err := runOutsideScreen(term, v, func() error { return editComment(r, id, in, out) })
	v.Entries = r.ListEntries()
	// Moves on even when nothing was saved, as the popup does when the edit
	// tab reopens it at the next unanswered item.
	v.ShowNextUndecided(v.Index+1, time.Now())
	if err != nil {
		return r, err
	}
	return writeWhenAllAnswered(
		r,
		v,
	)
}

// Returns true once Herdr opens the tab and moves every screen to it, so the
// popup can close.
func openTabOrShowWhy(v *popup.View, pane string, env map[string]string) bool {
	if _, err := openPluginPane(pane, "tab", env); err != nil {
		v.ShowNotice("cannot open the " + pane + " tab: " + err.Error())
		return false
	}
	return true
}

// Hands the terminal to run, such as nvim, then takes it back. A failure
// shows on the screen instead of closing it.
func runOutsideScreen(term *popup.Terminal, v *popup.View, run func() error) error {
	term.Restore()
	if err := run(); err != nil {
		v.ShowNotice(err.Error())
	}
	return term.Resume()
}

func saveAnswerAndShowNext(
	r *review.Review,
	v *popup.View,
	action string,
) (*review.Review, error) {
	if err := r.SetDecision(v.Entry().Item.ID, action, ""); err != nil {
		return r, err
	}
	v.Entries = r.ListEntries()
	v.ShowNextUndecided(v.Index+1, time.Now())
	return writeWhenAllAnswered(
		r,
		v,
	)
}

func writeWhenAllAnswered(
	r *review.Review,
	v *popup.View,
) (*review.Review, error) {
	if !v.IsAllAnswered() {
		return r, nil
	}
	return applyFromScreen(
		r.Name,
		v,
	)
}

// Applies the review and returns it as read again, for the screen to use
// from then on. A refusal stays on screen, so the owner can read why.
func applyFromScreen(name string, v *popup.View) (*review.Review, error) {
	// An AI may have added a round since the screen read the review.
	r, err := review.OpenReview(name)
	if err != nil {
		return nil, err
	}
	// A second screen, or a review reopened after its write, finds the
	// answers already in the code.
	if r.AppliedRound == len(r.Rounds) {
		v.ShowDone()
		return r, nil
	}
	// A missing checker is a refusal too, so its message stays on screen.
	checker, err := findCheckerPath()
	if err == nil {
		_, err = writeAndApproveDecisions(
			r,
			checker,
		)
	}
	v.Entries = r.ListEntries()
	switch {
	case err != nil && !v.IsAllAnswered():
		// A refusal for an item the new round added opens that item; the
		// answers are written once the owner answers it.
		v.ShowNextUndecided(
			0,
			time.Now(),
		)
	case err != nil:
		v.ShowNotWritten(err.Error())
	default:
		v.ShowDone()
	}
	return r, nil
}
