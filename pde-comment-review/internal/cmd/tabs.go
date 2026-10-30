package cmd

import (
	"bufio"
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"

	"pde-comment-review/internal/comment"
	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// Text above this line is the comment; the code below it is there to read.
const editMarker = "# ---- edits below are ignored ----"

// Runs in the edit tab: edits one comment in nvim, then reopens the popup
// at the next unanswered item and closes the tab.
func runEditTab() error {
	if os.Getenv("HERDR_PLUGIN_ENTRYPOINT_ID") != editPaneID {
		return errors.New("only Herdr's edit tab runs edit; press e in the review popup")
	}
	name := os.Getenv(reviewNameEnv)
	if err := editRequestByID(name, os.Getenv(requestIDEnv)); err != nil {
		waitForEnter(err)
	}
	if _, err := openPluginPane(popupPaneID, "popup", map[string]string{reviewNameEnv: name}); err != nil {
		waitForEnter(fmt.Errorf("cannot reopen the review: %w; press Ctrl+b e", err))
	}
	return closeOwnTab()
}

func editRequestByID(name, requestID string) error {
	r, err := review.OpenReview(name)
	if err != nil {
		return err
	}
	id, err := strconv.Atoi(requestID)
	if err != nil {
		return fmt.Errorf("item %q is not a number", requestID)
	}
	return editComment(r, id, os.Stdin, os.Stdout)
}

// Runs in the diff tab: shows the uncommitted changes in the review's
// files, then closes the tab.
func runDiffTab() error {
	if os.Getenv("HERDR_PLUGIN_ENTRYPOINT_ID") != diffPaneID {
		return errors.New("only Herdr's diff tab runs diff; press d in the review popup")
	}
	r, err := review.OpenReview(os.Getenv(reviewNameEnv))
	if err == nil {
		err = showDiff(r, os.Stdout)
	}
	if err != nil {
		waitForEnter(err)
	}
	return closeOwnTab()
}

// Keeps a tab open until Enter, so the owner can read why it failed.
func waitForEnter(err error) {
	fmt.Printf("pde-comment-review: %v\npress Enter to close\n", err)
	_, _ = bufio.NewReader(os.Stdin).ReadString('\n')
}

// Opens item id's comment as plain text in nvim and saves the owner's
// wording as the item's answer. A page left unchanged saves nothing, and a
// page with no words cuts the comment.
func editComment(r *review.Review, id int, in, out *os.File) error {
	e, err := findFeedbackRequest(r, id)
	if err != nil {
		return err
	}
	foundByName, err := readCommentsByName(r)
	if err != nil {
		return err
	}
	_, after, err := review.ReadCodeAround(
		r.Repo(),
		e.Request,
		foundByName,
		30,
	)
	if err != nil {
		return err
	}
	text := comment.ToPlainText(chooseStartingWording(e))
	page := filepath.Join(r.Dir, fmt.Sprintf("edit-%d.txt", id))
	content := text + "\n" + editMarker + "\n" + strings.Join(after, "\n") + "\n"
	if err := os.WriteFile(page, []byte(content), 0o600); err != nil {
		return err
	}
	defer os.Remove(page)
	nvim := exec.Command("nvim", "-c", "setlocal wrap linebreak nolist spell", page)
	if err := runOnTerminal(nvim, in, out); err != nil {
		return fmt.Errorf("nvim quit with an error, so nothing was saved: %w", err)
	}
	saved, err := os.ReadFile(page)
	if err != nil {
		return err
	}
	edited, _, hasMarker := strings.Cut(string(saved), editMarker)
	// Without the marker, the code below it would become part of the comment.
	if !hasMarker {
		return errors.New("the line starting with # ---- was deleted, so nothing was saved")
	}
	if strings.TrimSpace(edited) == strings.TrimSpace(text) {
		return nil
	}
	wording := comment.ToCommentLines(edited, e.Request.Old)
	if wording == "" {
		return r.SaveAnswer(id, e.Batch, review.ActionCut, "")
	}
	return r.SaveAnswer(id, e.Batch, review.ActionEdited, wording)
}

func findFeedbackRequest(r *review.Review, id int) (review.RequestWithFeedback, error) {
	for _, e := range r.ListFeedbackRequests() {
		if e.Request.ID == id {
			return e, nil
		}
	}
	return review.RequestWithFeedback{}, fmt.Errorf("review %s has no item %d", r.Name, id)
}

// Returns the newest wording: the owner's own, then the AI's reword, then
// the comment as it is.
func chooseStartingWording(e review.RequestWithFeedback) string {
	if e.Feedback.Answer != nil && e.Feedback.Answer.Action == review.ActionEdited {
		return e.Feedback.Answer.Text
	}
	if e.Request.Suggestion == review.SuggestReword {
		return e.Request.New
	}
	return e.Request.Old
}

// Shows the uncommitted changes in the review's files, through delta when
// it is installed, in less.
func showDiff(r *review.Review, out *os.File) error {
	args := append([]string{"-C", r.Repo(), "diff", "--"}, r.ListFiles()...)
	diff, err := exec.Command("git", args...).Output()
	if err != nil {
		return fmt.Errorf("git diff: %w", err)
	}
	if len(diff) == 0 {
		diff = []byte("no changes: every comment was kept\n")
	}
	cols, _ := popup.FindSize(out)
	delta := exec.Command("delta", "--paging=never", fmt.Sprintf("--width=%d", cols))
	delta.Stdin = bytes.NewReader(diff)
	if colored, err := delta.Output(); err == nil {
		diff = colored
	}
	return showInLess(diff, out)
}

// Opens the item's file read-only in nvim at its comment.
func viewFile(
	repo string,
	request review.FeedbackRequest,
	foundByName map[int]string,
	in *os.File,
	out *os.File,
) error {
	line, err := review.FindCommentLine(
		repo,
		request,
		foundByName,
	)
	if err != nil {
		return err
	}
	nvim := exec.Command("nvim", "-R", fmt.Sprintf("+%d", line), request.File)
	nvim.Dir = repo
	return runOnTerminal(nvim, in, out)
}

// Shows rg's matches for text, taken literally, in repo, in less.
func searchRepo(repo, text string, out *os.File) error {
	if strings.TrimSpace(text) == "" {
		return nil
	}
	rg := exec.Command("rg", "--color=always", "-n", "-S", "-F", "-C2", "--", text, ".")
	rg.Dir = repo
	found, err := rg.Output()
	// rg exits 1 with nothing on stderr when nothing matches; any other
	// failure, such as an aqua shim that cannot find rg, shows its reason.
	var exit *exec.ExitError
	switch {
	case errors.As(err, &exit) && exit.ExitCode() == 1 && len(exit.Stderr) == 0:
		found = []byte("no match for " + text + "\n")
	case errors.As(err, &exit) && len(exit.Stderr) > 0:
		return errors.New(strings.TrimSpace(string(exit.Stderr)))
	case err != nil:
		return err
	}
	return showInLess(found, out)
}

func showInLess(text []byte, out *os.File) error {
	less := exec.Command("less", "-R")
	less.Stdin = bytes.NewReader(text)
	less.Stdout, less.Stderr = out, out
	return less.Run()
}

func runOnTerminal(program *exec.Cmd, in, out *os.File) error {
	program.Stdin, program.Stdout, program.Stderr = in, out, out
	return program.Run()
}
