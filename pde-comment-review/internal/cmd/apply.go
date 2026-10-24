package cmd

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"pde-comment-review/internal/review"
)

const settingsFile = ".code-comments.json"

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
	return checker, nil
}

// Writes the owner's decisions into the code and approves them, so the
// comment checker fails if an agent changes them later. Writes nothing if
// any check fails first.
func writeAndApproveDecisions(r *review.Review, checker string) (string, error) {
	named, err := readReviewComments(r, checker)
	if err == nil {
		err = refuseUnwritableReview(r, named)
	}
	var changed map[string]string
	if err == nil {
		changed, err = r.BuildChangedFiles(findCommentsByName(r, named))
	}
	if err != nil {
		return "", fmt.Errorf("nothing was written: %w", err)
	}
	repo, renames, answered := r.Repo(), r.ListRenames(), listAnsweredComments(r, named)
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
	if err != nil {
		return "", fmt.Errorf("nothing was written: %w", err)
	}
	for i, file := range written {
		if err := os.WriteFile(filepath.Join(repo, file), []byte(changed[file]), 0o644); err != nil {
			return "", explainAfterWrite(err, written[:i])
		}
	}
	// Run gofmt again after a write in case of misalignment.
	if _, err := runGofmt(repo, "-w", written); err != nil {
		return "", explainAfterWrite(err, written)
	}
	for _, key := range answered {
		err := approveComment(
			repo,
			checker,
			key,
			renames,
		)
		if err != nil {
			return "", explainAfterWrite(err, written)
		}
	}
	if err := r.SetApplied(); err != nil {
		return "", fmt.Errorf("applied, but the review still shows as waiting: %w", err)
	}
	report := ""
	if len(renames) > 0 {
		report += "moved approvals to renamed code: " + strings.Join(renames, ", ") + "\n"
	}
	if len(written) == 0 {
		return report + "no file changed: every comment was kept\n", nil
	}
	return report + "wrote " + strings.Join(written, ", ") + "\nsee the changes with git -C " + repo + " diff\n", nil
}

type commentKey struct{ file, name string }

// Returns an error naming each problem in r that no answer can fix: items
// apply could not write, and comments it would approve unseen. Add runs it
// so the owner never answers a review that apply must refuse; apply runs it
// again, since the code can change after add.
func refuseUnwritableReview(
	r *review.Review,
	named map[commentKey]string,
) error {
	approved, err := readApprovals(r.Repo())
	if err != nil {
		return err
	}
	answeredFiles := map[string]bool{}
	for _, key := range listAnsweredComments(r, named) {
		answeredFiles[key.file] = true
	}
	moved, used := moveApprovals(
		approved,
		named,
		r.ListRenames(),
		answeredFiles,
	)
	problems := []error{
		refuseRepeatedItems(r.ListEntries(), named),
		refuseUnwritableNamedItems(r, named),
		refuseChangedApprovals(
			r,
			named,
			moved,
		),
	}
	removed := map[commentKey]bool{}
	for _, e := range r.ListEntries() {
		removed[commentKey{e.Item.File, e.Item.Name}] = e.Item.CodeRemoved
	}
	// Without its rename, an edited approved comment could pass as a new one.
	for key, text := range moved {
		if _, isComment := named[key]; text != "" && answeredFiles[key.file] && !isComment && !removed[key] {
			problems = append(
				problems,
				fmt.Errorf(
					"%s %s: the owner's approved comment is gone. If you renamed it, add the rename to renames; if you deleted its code, send it as an item with code_removed and this as old:\n    %s",
					key.file,
					key.name,
					strings.ReplaceAll(
						strings.TrimSuffix(text, "\n"),
						"\n",
						"\n    ",
					),
				),
			)
		}
	}
	for _, pair := range r.ListRenames() {
		if used[pair] {
			continue
		}
		before, _, _ := strings.Cut(pair, "=")
		problems = append(
			problems,
			fmt.Errorf(
				"rename %s would move no approved comment, because its file has no item. Add an item for that comment in its file, or remove the rename if the comment was never approved",
				before,
			),
		)
	}
	return errors.Join(problems...)
}

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

// Returns the comment of each of r's items that the checker names.
func listAnsweredComments(r *review.Review, named map[commentKey]string) []commentKey {
	var answered []commentKey
	for _, e := range r.ListEntries() {
		if name := findItemName(e.Item, named); name != "" {
			answered = append(answered, commentKey{e.Item.File, name})
		}
	}
	return answered
}

// Returns an error naming each item whose old is not its named comment's
// text, or not the text the owner approved, since approving it would
// approve another comment or an agent's edit.
func refuseChangedApprovals(
	r *review.Review,
	named map[commentKey]string,
	approved map[commentKey]string,
) error {
	var problems []error
	for _, e := range r.ListEntries() {
		key := commentKey{e.Item.File, findItemName(e.Item, named)}
		text, isApproved := approved[key]
		var problem string
		switch {
		case key.name == "" || isApproved && text == e.Item.Old:
			continue
		case named[key] != e.Item.Old:
			problem = "no comment by this name has the item's old. Fix or remove the item's name."
		case !isApproved:
			continue
		case text == "":
			problem = "the owner cut this comment. Delete it from the code."
		default:
			problem = "differs from the text the owner approved. To change it, list it as an item with this as old:\n    " + strings.ReplaceAll(
				strings.TrimSuffix(text, "\n"),
				"\n",
				"\n    ",
			)
		}
		problems = append(
			problems,
			fmt.Errorf(
				"item %d: %s %s: %s",
				e.Item.ID,
				key.file,
				key.name,
				problem,
			),
		)
	}
	return errors.Join(problems...)
}

// Returns the owner's approvals after the renames, as apply's approvals
// will move them, and each rename that moved one.
func moveApprovals(
	approved map[commentKey]string,
	named map[commentKey]string,
	renames []string,
	answeredFiles map[string]bool,
) (map[commentKey]string, map[string]bool) {
	moved, used := map[commentKey]string{}, map[string]bool{}
	for key, text := range approved {
		to := key
		// Each answer's approval moves every renamed file first.
		for _, pair := range renames {
			if before, after, _ := strings.Cut(pair, "="); to.file == before && len(answeredFiles) > 0 {
				to.file = after
				used[pair] = true
			}
		}
		// A name moves only in an answered file where it is no longer a comment.
		for _, pair := range renames {
			before, after, _ := strings.Cut(pair, "=")
			_, isStillComment := named[commentKey{to.file, before}]
			if to.name == before && answeredFiles[to.file] && !isStillComment {
				to.name = after
				used[pair] = true
			}
		}
		// An approval already under the new name wins.
		if _, taken := approved[to]; !taken || to == key {
			moved[to] = text
		}
	}
	return moved, used
}

// Returns an error naming each pair of items that are the same comment.
// Apply finds each item's comment by its old text, in item order: if the
// first answer changes the comment, apply refuses the second item, and if
// it keeps it, the second answer silently wins.
func refuseRepeatedItems(entries []review.Entry, named map[commentKey]string) error {
	type place struct {
		file string
		name string
		old  string
	}
	firstID := map[place]int{}
	var problems []error
	for _, e := range entries {
		at := place{
			file: e.Item.File,
			name: findItemName(e.Item, named),
		}
		if at.name == "" {
			at.old = e.Item.Old
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

// Returns an error naming each named item whose removed code is back, or
// whose comment the write cannot find exactly once.
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
	args := []string{checker, "--repo", repo, "--approve", "--"}
	run := exec.Command("python3", append(args, files...)...)
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

// Approves one answered comment through the checker's --approve-comment,
// after renames move the owner's earlier approvals.
func approveComment(
	repo string,
	checker string,
	key commentKey,
	renames []string,
) error {
	args := []string{checker, "--repo", repo, "--approve-comment", key.file, key.name}
	for _, pair := range renames {
		args = append(args, "--rename="+pair)
	}
	run := exec.Command("python3", args...)
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
func explainAfterWrite(err error, written []string) error {
	if len(written) == 0 {
		return fmt.Errorf(
			"no Go file was written; fix this and open the review again: %w",
			err,
		)
	}
	return fmt.Errorf(
		"%w\nThe owner's decisions are written into %s, but some may not be approved yet, and the review cannot write them again.\nRun gofmt -w on them, check the change with git diff, and have an agent approve each answered comment with the comment checker's --approve-comment",
		err,
		strings.Join(written, ", "),
	)
}
