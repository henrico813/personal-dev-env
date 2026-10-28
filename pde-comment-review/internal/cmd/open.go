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
		Entries: r.ListEntries(),
		Repo:    r.Repo(),
		Origin:  origin,
		Settle:  opts.settle,
	}
	askCheckerForCommentLayout(r, v)
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

// Loads locations and wrapping from the checker. Errors become notices.
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

// Returns line ranges by comment ID, omitting missing or repeated matches.
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
		// A deleted file has no comments left to locate.
		if _, err := os.Stat(path); err == nil {
			args = append(
				args,
				path,
			)
			files++
		}
	}
	// The checker's list needs at least one file.
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
	for _, e := range r.ListEntries() {
		var found []review.CommentLines
		for _, file := range listed.Files {
			if file.File != e.Item.File || e.Item.CodeRemoved {
				continue
			}
			for _, c := range file.Comments {
				at := strings.Index(
					c.Text,
					e.Item.Old,
				)
				switch {
				case e.Item.Name != "" && c.Name == e.Item.Name:
					found = append(
						found,
						review.CommentLines{
							First: c.Line,
							Last:  c.EndLine,
							NoCut: !c.CanCut,
						},
					)
				// Comments with no code name may be part of a longer
				// comment, such as prose before or after a Go directive.
				case e.Item.Name == "" && (at == 0 || at > 0 && c.Text[at-1] == '\n'):
					first := c.Line + strings.Count(
						c.Text[:at],
						"\n",
					)
					found = append(
						found,
						review.CommentLines{
							First: first,
							Last: first + strings.Count(
								e.Item.Old,
								"\n",
							) - 1,
							NoCut: !c.CanCut && e.Item.Old == c.Text,
						},
					)
				}
			}
		}
		if len(found) == 1 {
			places[e.Item.ID] = found[0]
		}
	}
	return places, nil
}

// Requests comment layout, styling optional sentences like the old lines.
type showEntry struct {
	File  string `json:"file"`
	Text  string `json:"text"`
	Plain string `json:"plain,omitempty"`
}

// Holds comment lines, editable sentences and the checker's screen layout.
type shownComment struct {
	Text   string            `json:"text"`
	Plain  string            `json:"plain"`
	Screen []popup.ScreenRow `json:"screen"`
}

// Runs the checker's show command and returns replies in comment order.
func showComments(checker, repo string, entries []showEntry) ([]shownComment, error) {
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

// Returns layouts keyed by original, proposed or owner-edited wording.
func readCommentRows(r *review.Review, checker string) (map[string][]popup.ScreenRow, error) {
	var entries []showEntry
	seen := map[string]bool{}
	for _, e := range r.ListEntries() {
		texts := []string{e.Item.Old, e.Item.New}
		if e.Decision != nil {
			texts = append(texts, e.Decision.Text)
		}
		for _, text := range texts {
			if text != "" && !seen[text] {
				seen[text] = true
				entries = append(
					entries,
					showEntry{
						File: e.Item.File,
						Text: text,
					},
				)
			}
		}
	}
	shown, err := showComments(
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
	askCheckerForCommentLayout(r, v)
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

// Returns true once Herdr opens the tab, so the popup can close and leave
// the screen to it.
func openTabOrShowWhy(v *popup.View, pane string, env map[string]string) bool {
	if err := openPluginPane(pane, "tab", env); err != nil {
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
		// New comments arrived, so the screen opens the first undecided
		// one. The owner's choices reach the code once every comment is
		// decided.
		askCheckerForCommentLayout(r, v)
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
