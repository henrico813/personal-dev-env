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
		Requests:    r.ListFeedbackRequests(),
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
	v.ShowNextUnanswered(0, time.Now())
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
		// Quitting reads nothing, so it works even when the review is gone.
		if v.IsQuitKey(key) {
			return nil
		}
		// A Herdr tab and the popup can be open on the same review, and the
		// AI can send new suggestions while the owner reviews.
		r, err = review.OpenReview(r.Name)
		if err != nil {
			return err
		}
		v.ReloadRequests(r.ListFeedbackRequests(), time.Now())
		switch v.PressKey(key, time.Now()) {
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
			err = r.SaveNote(v.Current().Request.ID, strings.TrimSpace(v.Typed))
			v.Requests = r.ListFeedbackRequests()
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
			request := v.Current().Request
			err = runOutsideScreen(
				term,
				v,
				func() error {
					return viewFile(
						r.Repo(),
						request,
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

// Returns each named request's comment as its file has it now. Runs the
// checker only when the review has a named request.
func readCommentsByName(r *review.Review) (map[int]string, error) {
	isNamed := func(e review.RequestWithFeedback) bool { return e.Request.Name != "" }
	if !slices.ContainsFunc(r.ListFeedbackRequests(), isNamed) {
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
	env := map[string]string{reviewNameEnv: r.Name, requestIDEnv: strconv.Itoa(v.Current().Request.ID)}
	return openTabOrShowWhy(v, editPaneID, env)
}

func editInPlace(
	term *popup.Terminal,
	r *review.Review,
	v *popup.View,
	in *os.File,
	out *os.File,
) (*review.Review, error) {
	id := v.Current().Request.ID
	err := runOutsideScreen(term, v, func() error { return editComment(r, id, in, out) })
	v.Requests = r.ListFeedbackRequests()
	// Moves on even when nothing was saved, as the popup does when the edit
	// tab reopens it at the next unanswered request.
	v.ShowNextUnanswered(v.Index+1, time.Now())
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
	shown := v.Current()
	if err := r.SaveAnswer(shown.Request.ID, shown.Batch, action, ""); err != nil {
		return r, err
	}
	v.Requests = r.ListFeedbackRequests()
	v.ShowNextUnanswered(v.Index+1, time.Now())
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
	// An AI may have added a batch since the screen read the review.
	r, err := review.OpenReview(name)
	if err != nil {
		return nil, err
	}
	// A second screen, or a review reopened after its write, finds the
	// answers already in the code.
	if r.AppliedBatch == len(r.Batches) {
		v.ShowDone()
		return r, nil
	}
	// A missing checker is a refusal too, so its message stays on screen.
	checker, err := findCheckerPath()
	if err == nil {
		_, err = writeAndApproveAnswers(
			r,
			checker,
		)
	}
	v.Requests = r.ListFeedbackRequests()
	switch {
	case err != nil && !v.IsAllAnswered():
		// A refused request from the new batch opens on screen. The answers are
		// written once the owner answers it.
		v.ShowNextUnanswered(
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
