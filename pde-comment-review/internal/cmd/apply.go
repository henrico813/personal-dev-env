package cmd

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"sort"
	"strings"

	"pde-comment-review/internal/comment"
	"pde-comment-review/internal/review"
)

const settingsFile = ".code-comments.json"

// Returns the installed comment checker, the one program that writes
// .code-comments.json.
func findCheckerPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".agents", "skills", "code-documentation", "scripts", "check_comments.py"), nil
}

// Writes the owner's decisions into the code and approves them, so the
// comment checker fails if an agent changes them later. Writes nothing if
// any check fails first.
func writeAndApproveDecisions(r *review.Review, checker string) (string, error) {
	changed, err := r.BuildChangedFiles()
	if err != nil {
		return "", fmt.Errorf("nothing was written: %w", err)
	}
	repo, files, renames := r.Repo(), r.ListFiles(), r.ListRenames()
	var written []string
	for file := range changed {
		written = append(written, file)
	}
	sort.Strings(written)
	listed, err := runGofmt(repo, "-l", written)
	if err == nil && listed != "" {
		err = fmt.Errorf("gofmt would change %s, so the write would change more than comments; run gofmt -w on it first",
			strings.ReplaceAll(listed, "\n", ", "))
	}
	if err == nil {
		err = refuseUnwritableReview(
			r,
			checker,
		)
	}
	// Approve the text as it is now, so the run after the write records cuts.
	if err == nil {
		_, err = approveComments(repo, checker, files, renames)
	}
	if err != nil {
		return "", fmt.Errorf("nothing was written: %w", err)
	}
	for i, file := range written {
		if err := os.WriteFile(filepath.Join(repo, file), []byte(changed[file]), 0o644); err != nil {
			return "", explainAfterWrite(err, repo, checker, written[:i], files)
		}
	}
	// Run gofmt again after a write in case of misalignment.
	if _, err := runGofmt(repo, "-w", written); err != nil {
		return "", explainAfterWrite(err, repo, checker, written, files)
	}
	summary, err := approveComments(repo, checker, files, nil)
	if err != nil {
		return "", explainAfterWrite(err, repo, checker, written, files)
	}
	if err := r.SetApplied(); err != nil {
		return "", fmt.Errorf("applied, but the review still shows as waiting: %w", err)
	}
	report := ""
	if len(renames) > 0 {
		report += "moved approvals to renamed code: " + strings.Join(renames, ", ") + "\n"
	}
	if len(written) == 0 {
		return report + "no file changed: every comment was kept\n" + summary, nil
	}
	return report + "wrote " + strings.Join(written, ", ") + "\n" + summary + "see the changes with git -C " + repo + " diff\n", nil
}

type commentKey struct{ file, name string }

// Returns an error naming each problem in r that no answer can fix: items
// apply could not write, and comments it would approve unseen. Add runs it
// so the owner never answers a review that apply must refuse; apply runs it
// again, since the code can change after add.
func refuseUnwritableReview(
	r *review.Review,
	checker string,
) error {
	return errors.Join(
		refuseRepeatedItems(r.ListEntries()),
		refuseUnseenComments(
			r,
			checker,
		),
	)
}

// Returns an error naming each pair of items that are the same comment.
// Apply finds each item's comment by its old text, in item order: if the
// first answer changes the comment, apply refuses the second item, and if
// it keeps it, the second answer silently wins.
func refuseRepeatedItems(entries []review.Entry) error {
	type place struct {
		file string
		old  string
	}
	firstID := map[place]int{}
	var problems []error
	for _, e := range entries {
		at := place{
			file: e.Item.File,
			old:  e.Item.Old,
		}
		if id, ok := firstID[at]; ok {
			problems = append(
				problems,
				fmt.Errorf(
					"items %d and %d are the same comment in %s; keep only one of them",
					id,
					e.Item.ID,
					e.Item.File,
				),
			)
			continue
		}
		firstID[at] = e.Item.ID
	}
	return errors.Join(problems...)
}

// Returns an error naming each comment the owner has not seen that
// approving would record. Without it, apply would also approve any
// comment an agent changed outside the review.
func refuseUnseenComments(r *review.Review, checker string) error {
	repo, files := r.Repo(), r.ListFiles()
	approved, err := readApprovals(repo)
	if err != nil {
		return err
	}
	trial, err := os.MkdirTemp("", "pde-comment-review-")
	if err != nil {
		return err
	}
	defer os.RemoveAll(trial)
	for _, file := range append([]string{settingsFile}, files...) {
		data, err := os.ReadFile(filepath.Join(repo, file))
		if file == settingsFile && errors.Is(err, fs.ErrNotExist) {
			continue
		}
		if err == nil {
			err = os.MkdirAll(filepath.Dir(filepath.Join(trial, file)), 0o700)
		}
		if err == nil {
			err = os.WriteFile(filepath.Join(trial, file), data, 0o600)
		}
		if err != nil {
			return err
		}
	}
	printed, err := approveComments(
		trial,
		checker,
		files,
		r.ListRenames(),
	)
	if err != nil {
		return err
	}
	// The checker says "--rename OLD: matches no entry" when a rename moved
	// nothing, as it does in a file the review does not check.
	for _, line := range strings.Split(
		printed,
		"\n",
	) {
		old, isUnused := strings.CutPrefix(
			line,
			"--rename ",
		)
		if !isUnused {
			continue
		}
		renamed, _, _ := strings.Cut(
			old,
			":",
		)
		return fmt.Errorf(
			"rename %s would move no approved comment, because its file has no item. Add an item for that comment in its file, or remove the rename if the comment was never approved",
			renamed,
		)
	}
	wouldApprove, err := readApprovals(trial)
	if err != nil {
		return err
	}
	type reviewedText struct{ file, text string }
	inReview := map[reviewedText]bool{}
	for _, e := range r.ListEntries() {
		inReview[reviewedText{e.Item.File, e.Item.Old}] = true
	}
	renamedFrom := map[string]string{}
	for _, pair := range r.ListRenames() {
		before, after, _ := strings.Cut(pair, "=")
		renamedFrom[after] = before
	}
	var unseen []string
	for key, text := range wouldApprove {
		if !slices.Contains(files, key.file) {
			continue
		}
		old, ok := approved[key]
		renamed := false
		if before, isNew := renamedFrom[key.name]; !ok && isNew {
			old, ok = approved[commentKey{key.file, before}]
			renamed = ok
		}
		place := key.file + " " + key.name
		seen := ok && text == old || (!ok || renamed) && inReview[reviewedText{key.file, text}]
		if seen {
			continue
		}
		asItem := " To change it, list it as an item with this as old:\n" + strings.ReplaceAll(
			"    "+strings.TrimSuffix(
				old,
				"\n",
			),
			"\n",
			"\n    ",
		)
		var problem string
		switch {
		case ok && old == "":
			problem = "the owner cut this comment. Delete it from the code."
		case ok && text == "":
			problem = "the owner's approved comment is gone. If you renamed it, add the " +
				"rename to renames; otherwise put it back." + asItem
		case ok:
			problem = "differs from the text the owner approved. Put it back." + asItem
		// The checker's text is whole // lines, so only a line with no
		// space after //, such as //go:generate, fails this.
		case comment.CheckWholeLines(text) != nil:
			problem = "a line has no space after //, so Go may read it as a directive. " +
				"Add the space if it is prose, or leave " + key.file + " out of the review."
		default:
			problem = "not in the review, and the owner never approved it. " +
				"Add it as an item, with old copied from the file."
		}
		unseen = append(
			unseen,
			place+": "+problem,
		)
	}
	if len(unseen) == 0 {
		return nil
	}
	sort.Strings(unseen)
	return fmt.Errorf(
		"these comments would be approved, but the owner has not seen them:\n%s",
		strings.Join(
			unseen,
			"\n",
		),
	)
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
func approveComments(repo, checker string, files, renames []string) (string, error) {
	args := []string{checker, "--repo", repo, "--approve"}
	for _, pair := range renames {
		args = append(args, "--rename="+pair)
	}
	run := exec.Command("python3", append(append(args, "--"), files...)...)
	run.Dir = repo
	output, err := run.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("comment checker: %w: %s", err, strings.TrimSpace(string(output)))
	}
	return string(output), nil
}

// Runs gofmt for the check before the write and the reformat after it.
// Returns what gofmt printed, such as the files -l lists.
func runGofmt(repo, flag string, files []string) (string, error) {
	// Skip gofmt when no file changes: with no files, gofmt -w fails.
	if len(files) == 0 {
		return "", nil
	}
	gofmt := exec.Command("gofmt", append([]string{flag, "--"}, files...)...)
	gofmt.Dir = repo
	output, err := gofmt.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("gofmt: %w: %s", err, strings.TrimSpace(string(output)))
	}
	return strings.TrimSpace(string(output)), nil
}

// Adds to err how to finish by hand, since once a file is written the
// review cannot write it again.
func explainAfterWrite(err error, repo, checker string, written, files []string) error {
	if len(written) == 0 {
		return fmt.Errorf(
			"no Go file was written; fix this and open the review again: %w",
			err,
		)
	}
	return fmt.Errorf("%w\n"+
		"The owner's decisions are written into %s, but .code-comments.json still\n"+
		"approves the old comments, and the review cannot write them again.\n"+
		"Check the change with git diff, then in %s run:\n"+
		"  gofmt -w %s\n"+
		"  python3 %s --repo . --approve %s",
		err, strings.Join(written, ", "), repo, strings.Join(written, " "), checker, strings.Join(files, " "))
}
