package cmd

import (
	"fmt"
	"os"
	"path/filepath"
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
	v := &popup.View{
		Requests: r.ListFeedbackRequests(),
		Repo:     r.Repo(),
		Origin:   origin,
		Settle:   opts.settle,
	}
	askCheckerForCommentLayout(r, v)
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
		if v.IsQuitKey(key) {
			return nil
		}
		// Reads the review again, since the popup, a Herdr tab or the AI may
		// have changed it.
		batches := len(r.Batches)
		r, err = review.OpenReview(r.Name)
		if err != nil {
			return err
		}
		// Comments from a new batch need the checker to find and wrap them.
		if len(r.Batches) != batches {
			askCheckerForCommentLayout(r, v)
		}
		v.ReplaceRequests(r.ListFeedbackRequests(), time.Now())
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
						v.CommentLinesByID,
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

// Asks the checker where each comment sits in its file and how to wrap it on
// screen. A failure shows as a message on the screen instead of closing it.
func askCheckerForCommentLayout(r *review.Review, v *popup.View) {
	checker, err := findCheckerPath()
	if err == nil {
		v.CommentLinesByID, err = findCommentLineRanges(r, checker)
	}
	if err == nil {
		v.CommentRows, err = readCommentRows(r, checker)
	}
	if err != nil {
		v.ShowNotice(err.Error())
	}
}

// Returns the lines of each comment under review.
func findCommentLineRanges(r *review.Review, checker string) (map[int]review.CommentLines, error) {
	args := []string{
		"list",
		"--interface",
		fmt.Sprint(checkerJSONVersion),
		"--repo",
		r.Repo(),
	}
	files := 0
	for _, file := range r.ListFiles() {
		path := filepath.Join(
			r.Repo(),
			file,
		)
		// The checker fails on a file that is gone, such as one the branch
		// deleted.
		if _, err := os.Stat(path); err == nil {
			args = append(
				args,
				path,
			)
			files++
		}
	}
	if files == 0 {
		return nil, nil
	}
	var listed struct {
		Files []struct {
			File     string `json:"file"`
			Comments []struct {
				Name    string `json:"name"`
				Line    int    `json:"line"`
				EndLine int    `json:"end_line"`
				Text    string `json:"text"`
				CanCut  bool   `json:"can_cut"`
			} `json:"comments"`
		} `json:"files"`
		Problems []string `json:"problems"`
	}
	err := runCheckerCommand(
		checker,
		nil,
		&listed,
		args...,
	)
	if err == nil {
		err = joinProblems(listed.Problems)
	}
	if err != nil {
		return nil, err
	}
	places := map[int]review.CommentLines{}
	for _, e := range r.ListFeedbackRequests() {
		var found []review.CommentLines
		for _, file := range listed.Files {
			if file.File != e.Request.File || e.Request.CodeRemoved {
				continue
			}
			for _, c := range file.Comments {
				switch {
				case e.Request.Name != "" && c.Name == e.Request.Name:
					found = append(
						found,
						review.CommentLines{
							First: c.Line,
							Last:  c.EndLine,
							NoCut: !c.CanCut,
						},
					)
				case e.Request.Name == "":
					if lines, ok := findTextInComment(c.Text, e.Request.Old, c.Line); ok {
						lines.NoCut = !c.CanCut && e.Request.Old == c.Text
						found = append(found, lines)
					}
				}
			}
		}
		if len(found) == 1 {
			places[e.Request.ID] = found[0]
		}
	}
	return places, nil
}

// Returns the lines of a comment the checker could not name.
func findTextInComment(text, old string, line int) (review.CommentLines, bool) {
	at := strings.Index(text, old)
	// The comment under review may be only part of the checker's comment, such
	// as the prose above a Go directive. It counts only when it starts a line.
	if at < 0 || at > 0 && text[at-1] != '\n' {
		return review.CommentLines{}, false
	}
	first := line + strings.Count(text[:at], "\n")
	return review.CommentLines{
		First: first,
		Last:  first + strings.Count(old, "\n") - 1,
	}, true
}

// Holds one comment to send to the checker, with its file so the checker can
// tell the language.
type showEntry struct {
	File string `json:"file"`
	Text string `json:"text"`
}

// Holds what the checker returns for one comment. It gives the comment as file
// lines, as plain sentences to edit, and as paragraphs the screen can wrap.
type shownComment struct {
	Text   string            `json:"text"`
	Plain  string            `json:"plain"`
	Screen []popup.ScreenRow `json:"screen"`
}

// Asks the checker to turn each comment into plain sentences the owner edits
// and into paragraphs the screen can wrap to its width.
func prepareCommentsForScreen(checker, repo string, entries []showEntry) ([]shownComment, error) {
	var reply struct {
		Shown    []shownComment `json:"shown"`
		Problems []string       `json:"problems"`
	}
	err := runCheckerCommand(
		checker,
		entries,
		&reply,
		"show",
		"--interface",
		fmt.Sprint(checkerJSONVersion),
		"--repo",
		repo,
	)
	if err == nil {
		err = joinProblems(reply.Problems)
	}
	return reply.Shown, err
}

// Asks the checker to turn plain sentences back into comment lines. The new
// lines keep the old comment's markers, such as //, and its indentation.
func convertSentencesToComment(checker, repo, file, old, plain string) (string, error) {
	entries := []struct {
		File  string `json:"file"`
		Text  string `json:"text"`
		Plain string `json:"plain"`
	}{{
		File:  file,
		Text:  old,
		Plain: plain,
	}}
	var reply struct {
		Shown []struct {
			Text string `json:"text"`
		} `json:"shown"`
		Problems []string `json:"problems"`
	}
	err := runCheckerCommand(
		checker,
		entries,
		&reply,
		"show",
		"--interface",
		fmt.Sprint(checkerJSONVersion),
		"--repo",
		repo,
	)
	if err == nil {
		err = joinProblems(reply.Problems)
	}
	if err != nil {
		return "", err
	}
	return reply.Shown[0].Text, nil
}

// Asks the checker to wrap every version of each comment for the screen.
func readCommentRows(r *review.Review, checker string) (map[string][]popup.ScreenRow, error) {
	var entries []showEntry
	seen := map[string]bool{}
	for _, e := range r.ListFeedbackRequests() {
		texts := []string{e.Request.Old, e.Request.New}
		if e.Feedback.Answer != nil {
			texts = append(texts, e.Feedback.Answer.Text)
		}
		for _, text := range texts {
			if text != "" && !seen[text] {
				seen[text] = true
				entries = append(
					entries,
					showEntry{
						File: e.Request.File,
						Text: text,
					},
				)
			}
		}
	}
	shown, err := prepareCommentsForScreen(
		checker,
		r.Repo(),
		entries,
	)
	if err != nil {
		return nil, err
	}
	rows := map[string][]popup.ScreenRow{}
	for i, s := range shown {
		rows[entries[i].Text] = s.Screen
	}
	return rows, nil
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
	askCheckerForCommentLayout(r, v)
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
		askCheckerForCommentLayout(r, v)
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
