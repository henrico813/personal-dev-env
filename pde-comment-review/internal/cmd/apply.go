package cmd

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"pde-comment-review/internal/review"
)

const settingsFile = ".code-comments.json"

const checkerJSONVersion = 1

// Returns the installed comment checker, the one program that writes
// .code-comments.json.
func findCheckerPath() (string, error) {
	checker, err := exec.LookPath("pde-comment-checker")
	if err != nil {
		return "", fmt.Errorf(
			"%w; run pde-installer to install the comment checker",
			err,
		)
	}
	// A checker from another install may read this tool's requests differently.
	var version struct{}
	err = runCheckerCommand(
		checker,
		nil,
		&version,
		"version",
	)
	if err != nil {
		return "", fmt.Errorf(
			"%w; run pde-installer so pde-comment-review and the checker come from one install",
			err,
		)
	}
	return checker, nil
}

// Runs one of the checker's JSON commands, such as changes, and fills reply
// from what it prints. Exit 1 is a refusal whose reasons are in the reply.
func runCheckerCommand(checker string, request any, reply any, args ...string) error {
	run := exec.Command(
		checker,
		args...,
	)
	if request != nil {
		data, err := json.Marshal(request)
		if err != nil {
			return err
		}
		run.Stdin = bytes.NewReader(data)
	}
	var stderr strings.Builder
	run.Stderr = &stderr
	output, err := run.Output()
	var exit *exec.ExitError
	refused := errors.As(err, &exit) && exit.ExitCode() == 1 && len(output) > 0
	if refused {
		err = nil
	}
	var spoken struct {
		Interface int      `json:"interface"`
		Problems  []string `json:"problems"`
	}
	if err == nil {
		err = json.Unmarshal(
			output,
			&spoken,
		)
	}
	// A refusal with no reasons would read as nothing to review.
	if err == nil && refused && len(spoken.Problems) == 0 {
		err = errors.New("exit status 1 with no problems in its reply")
	}
	if err != nil {
		if detail := strings.TrimSpace(stderr.String()); detail != "" {
			err = fmt.Errorf(
				"%w: %s",
				err,
				detail,
			)
		}
		return fmt.Errorf(
			"comment checker: %w",
			err,
		)
	}
	if spoken.Interface != checkerJSONVersion {
		return fmt.Errorf(
			"the comment checker speaks interface %d, not %d",
			spoken.Interface,
			checkerJSONVersion,
		)
	}
	return json.Unmarshal(
		output,
		reply,
	)
}

// One answer sent to the checker, as if the owner accepted every proposal.
// Text is left out for a cut.
type checkerAnswer struct {
	ID          int    `json:"id"`
	File        string `json:"file"`
	Name        string `json:"name,omitempty"`
	Old         string `json:"old"`
	Current     string `json:"current,omitempty"`
	Text        string `json:"text,omitempty"`
	Cut         bool   `json:"cut,omitempty"`
	CodeRemoved bool   `json:"code_removed,omitempty"`
}

// Returns an error naming each problem the checker finds in r, as if the owner
// accepted every proposal.
func refuseUnwritableProposals(
	r *review.Review,
	named map[commentKey]string,
	checker string,
) error {
	var answers []checkerAnswer
	for _, e := range r.ListEntries() {
		answers = append(answers, buildAcceptedAnswer(e.Item, named))
	}
	reply, err := sendCommentsToChecker(r, answers, checker, true /* check */)
	if err != nil {
		return err
	}
	return joinProblems(reply.Problems)
}

// Holds the checker's report of changed files or reasons for refusing to
// write.
type applyReply struct {
	Changed  []string `json:"changed"`
	Problems []string `json:"problems"`
}

// Asks the checker to write and approve comments, moving approvals for
// renamed code. In check-only mode, reports what would prevent writing
// without changing files.
func sendCommentsToChecker(
	r *review.Review,
	answers []checkerAnswer,
	checker string,
	check bool,
) (applyReply, error) {
	request := struct {
		Interface int             `json:"interface"`
		Renames   []string        `json:"renames,omitempty"`
		Answers   []checkerAnswer `json:"answers"`
	}{
		Interface: checkerJSONVersion,
		Renames:   r.ListRenames(),
		Answers:   answers,
	}
	args := []string{"apply"}
	if check {
		args = append(args, "--check")
	}
	var reply applyReply
	err := runCheckerCommand(
		checker,
		request,
		&reply,
		append(
			args,
			"--interface",
			fmt.Sprint(checkerJSONVersion),
			"--repo",
			r.Repo(),
		)...,
	)
	return reply, err
}

// Returns one error holding each of the checker's problems, or nil.
func joinProblems(problems []string) error {
	var errs []error
	for _, problem := range problems {
		errs = append(errs, errors.New(problem))
	}
	return errors.Join(errs...)
}

// Returns the answer the owner gives by accepting item's proposal.
func buildAcceptedAnswer(item review.Item, named map[commentKey]string) checkerAnswer {
	answer := checkerAnswer{
		ID:          item.ID,
		File:        item.File,
		Name:        item.Name,
		Old:         item.Old,
		CodeRemoved: item.CodeRemoved,
	}
	if !item.CodeRemoved {
		answer.Current = item.Old
		// A drafted item's old may be the approved text, which its file no
		// longer holds.
		if text, ok := named[commentKey{item.File, item.Name}]; ok {
			answer.Current = text
		}
	}
	switch item.Proposal {
	case review.ProposeCut:
		answer.Cut = true
	case review.ProposeReword:
		answer.Text = item.New
	default:
		// Keeping a comment restores the text the owner was shown.
		answer.Text = item.Old
	}
	return answer
}

// Converts the owner's decision about a comment into a request for the
// checker.
func convertDecisionForChecker(e review.Entry, named map[commentKey]string) (checkerAnswer, error) {
	answer := buildAcceptedAnswer(e.Item, named)
	switch action := e.Decision.Action; action {
	case review.ActionAccept:
	case review.ActionKeep:
		// Keeping a comment restores the text the owner was shown.
		answer.Text, answer.Cut = e.Item.Old, false
	case review.ActionCut:
		answer.Text, answer.Cut = "", true
	case review.ActionEdited:
		answer.Text, answer.Cut = e.Decision.Text, false
	default:
		return answer, fmt.Errorf(
			"item %d: decision %q must be keep, cut, accept or edited",
			e.Item.ID,
			action,
		)
	}
	return answer, nil
}

// Writes the owner's decisions into the code and approves them, so the
// comment checker fails if an agent changes them later. Writes nothing if
// any check fails first.
func writeAndApproveDecisions(r *review.Review, checker string) (string, error) {
	var undecided []string
	for _, e := range r.ListEntries() {
		if e.Decision == nil {
			undecided = append(undecided, fmt.Sprint(e.Item.ID))
		}
	}
	if len(undecided) > 0 {
		return "", fmt.Errorf(
			"nothing was written: not decided: items %s",
			strings.Join(undecided, ", "),
		)
	}
	named, err := readReviewComments(r, checker)
	if err != nil {
		return "", fmt.Errorf("nothing was written: %w", err)
	}
	var answers []checkerAnswer
	for _, e := range r.ListEntries() {
		answer, err := convertDecisionForChecker(e, named)
		if err != nil {
			return "", fmt.Errorf("nothing was written: %w", err)
		}
		answers = append(answers, answer)
	}
	reply, err := sendCommentsToChecker(r, answers, checker, false /* check */)
	// The checker may have started writing before this error, so some files
	// may have changed.
	if err != nil {
		return "", fmt.Errorf(
			"write did not finish; files may have changed: %w\nRun git -C %s status to check the files.",
			err,
			r.Repo(),
		)
	}
	if err := joinProblems(reply.Problems); err != nil {
		return "", fmt.Errorf("nothing was written: %w", err)
	}
	if err := r.SetApplied(); err != nil {
		return "", fmt.Errorf("applied, but the review still shows as waiting: %w", err)
	}
	renames := r.ListRenames()
	report := ""
	if len(renames) > 0 {
		report += "moved approvals to renamed code: " + strings.Join(renames, ", ") + "\n"
	}
	if len(reply.Changed) == 0 {
		return report + "no file changed: every comment was kept\n", nil
	}
	return report + "wrote " + strings.Join(reply.Changed, ", ") + "\nsee the changes with git -C " + r.Repo() + " diff\n", nil
}

type commentKey struct{ file, name string }

// Returns the text of each comment the checker names in r's files.
func readReviewComments(r *review.Review, checker string) (map[commentKey]string, error) {
	sources := map[string]string{}
	for _, file := range r.ListFiles() {
		data, err := os.ReadFile(filepath.Join(r.Repo(), file))
		// A deleted file's comments are all gone.
		if errors.Is(err, fs.ErrNotExist) {
			continue
		}
		if err != nil {
			return nil, err
		}
		sources[file] = string(data)
	}
	return readCommentTexts(sources, checker)
}

// Returns the checker's name for the item's comment, or "" for one it
// never names, such as a comment inside a function.
func findItemName(item review.Item, named map[commentKey]string) string {
	if item.Name != "" {
		return item.Name
	}
	for key, text := range named {
		if key.file == item.File && text == item.Old {
			return key.name
		}
	}
	return ""
}

// Returns each named item's comment as its file has it now, by item ID.
func findCommentsByName(r *review.Review, named map[commentKey]string) map[int]string {
	foundByName := map[int]string{}
	for _, e := range r.ListEntries() {
		if text, ok := named[commentKey{e.Item.File, e.Item.Name}]; ok {
			foundByName[e.Item.ID] = text
		}
	}
	return foundByName
}

// Returns an error for each comment, found by its function or type name,
// whose deleted code is back or whose text is not in its file exactly once.
// The screen opens nvim at that text when the owner views the file.
func refuseUnwritableNamedItems(r *review.Review, named map[commentKey]string) error {
	foundByName := findCommentsByName(r, named)
	var problems []error
	for _, e := range r.ListEntries() {
		if _, found := foundByName[e.Item.ID]; !found {
			continue
		}
		var err error
		if e.Item.CodeRemoved {
			err = fmt.Errorf(
				"%s %s: its removed code is back. Send the item again without code_removed.",
				e.Item.File,
				e.Item.Name,
			)
		} else {
			_, err = review.FindCommentLine(
				r.Repo(),
				e.Item,
				foundByName,
			)
		}
		if err != nil {
			problems = append(
				problems,
				fmt.Errorf(
					"item %d: %w",
					e.Item.ID,
					err,
				),
			)
		}
	}
	return errors.Join(problems...)
}

// Returns the approved text per comment in dir's .code-comments.json, ""
// for a cut one, to compare what approving would change.
func readApprovals(dir string) (map[commentKey]string, error) {
	entries, err := readApprovalEntries(dir)
	if err != nil {
		return nil, err
	}
	approvals := map[commentKey]string{}
	for _, entry := range entries {
		approvals[commentKey{entry.File, entry.Name}] = entry.Text
	}
	return approvals, nil
}

// One entry of .code-comments.json: a comment's approved text, or "" when
// the owner cut it.
type namedComment struct {
	File string `json:"file"`
	Name string `json:"name"`
	Text string `json:"text"`
}

// Returns the entries of dir's .code-comments.json in the order the
// checker wrote them, which keeps each file's comments in file order.
func readApprovalEntries(dir string) ([]namedComment, error) {
	data, err := os.ReadFile(filepath.Join(dir, settingsFile))
	if errors.Is(err, fs.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var settings struct {
		Approved []namedComment `json:"approved"`
	}
	if err := json.Unmarshal(data, &settings); err != nil {
		return nil, fmt.Errorf("%s: %w", settingsFile, err)
	}
	return settings.Approved, nil
}

// Approves through the checker's --approve, since only the checker
// writes .code-comments.json.
func approveComments(repo, checker string, files []string) error {
	args := []string{
		"--repo",
		repo,
		"--approve",
		"--",
	}
	run := exec.Command(
		checker,
		append(args, files...)...,
	)
	run.Dir = repo
	output, err := run.CombinedOutput()
	if err != nil {
		return fmt.Errorf(
			"comment checker: %w: %s",
			err,
			strings.TrimSpace(string(output)),
		)
	}
	return nil
}
