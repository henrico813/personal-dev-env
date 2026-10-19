package cmd

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"pde-comment-review/internal/review"
)

// Returns an item for each comment the owner must answer: an approved
// comment that changed, was renamed or lost its code.
func buildDraft(repo, checker string) (review.Proposals, error) {
	approvals, err := readApprovals(repo)
	if err != nil {
		return review.Proposals{}, err
	}
	named, checked, err := readCheckoutComments(
		repo,
		checker,
		approvals,
	)
	if err != nil {
		return review.Proposals{}, err
	}
	renamed := moveRenamedApprovals(
		approvals,
		named,
		checked,
	)
	var items []review.Item
	var problems []string
	for _, c := range named {
		key := commentKey{c.File, c.Name}
		old, isApproved := approvals[key]
		_, isRenamed := renamed[key]
		switch {
		case isApproved && old == "":
			problems = append(
				problems,
				c.File+" "+c.Name+": the owner cut this comment. Delete it from the code.",
			)
			continue
		// A renamed comment stays an item: the write moves its approval
		// only in a file that has an item.
		case isApproved && old == c.Text && !isRenamed:
			continue
		case !isApproved:
			continue
		}
		item := review.Item{
			File:     c.File,
			Name:     c.Name,
			Old:      old,
			Proposal: review.ProposeKeep,
		}
		if c.Text != old {
			item.Proposal, item.New = review.ProposeReword, c.Text
		}
		items = append(items, item)
	}
	for _, key := range listGoneApprovals(approvals, named) {
		place := key.file + " " + key.name
		if isNameShared(checked, key) {
			problems = append(
				problems,
				place+": the owner approved this comment, and now more than one comment in the file has its name, so none of them can be told apart. Keep a comment on only one of them.",
			)
			continue
		}
		if candidates := findRenameCandidates(checked, key); len(candidates) > 0 {
			problems = append(
				problems,
				place+": the owner approved this comment, and it is gone. It may now be "+strings.Join(candidates, " or ")+", but that is not certain, so none is guessed. Reword the first sentence of each comment that is not the renamed one.",
			)
			continue
		}
		items = append(items, review.Item{
			File:        key.file,
			Name:        key.name,
			Old:         approvals[key],
			Proposal:    review.ProposeCut,
			CodeRemoved: true,
		})
	}
	if len(problems) > 0 {
		sort.Strings(problems)
		return review.Proposals{}, fmt.Errorf(
			"fix these comments first:\n%s",
			strings.Join(problems, "\n"),
		)
	}
	var renames []string
	for key, from := range renamed {
		renames = append(renames, from+"="+key.name)
	}
	sort.Strings(renames)
	for i := range items {
		items[i].ID = i + 1
	}
	return review.Proposals{
		Version: review.Version,
		Repo:    repo,
		Items:   items,
		Renames: renames,
	}, nil
}

// Returns the comments in the checkout's files that have approvals, and
// what the checker printed when it checked them.
func readCheckoutComments(
	repo string,
	checker string,
	approvals map[commentKey]string,
) ([]namedComment, string, error) {
	files := map[string]bool{}
	for key := range approvals {
		files[key.file] = true
	}
	sources := map[string]string{}
	for file := range files {
		data, err := os.ReadFile(filepath.Join(repo, file))
		// A deleted file's approvals are all gone comments.
		if errors.Is(err, fs.ErrNotExist) {
			continue
		}
		if err != nil {
			return nil, "", err
		}
		sources[file] = string(data)
	}
	return readNamedComments(
		sources,
		approvals,
		checker,
	)
}

// Approves every comment in a copy of sources, with approvals as its
// .code-comments.json, to learn each comment's name and text without
// touching the checkout. Returns them in file order, and what the
// checker printed before approving.
func readNamedComments(
	sources map[string]string,
	approvals map[commentKey]string,
	checker string,
) ([]namedComment, string, error) {
	// The checker exits 2 when given no Go file.
	if len(sources) == 0 {
		return nil, "", nil
	}
	trial, err := os.MkdirTemp("", "pde-comment-review-")
	if err != nil {
		return nil, "", err
	}
	defer os.RemoveAll(trial)
	settings := struct {
		Approved []namedComment    `json:"approved"`
		Avoid    map[string]string `json:"avoid"`
	}{
		// The checker refuses null for either.
		Approved: []namedComment{},
		Avoid:    map[string]string{},
	}
	for key, text := range approvals {
		settings.Approved = append(settings.Approved, namedComment{
			File: key.file,
			Name: key.name,
			Text: text,
		})
	}
	data, err := json.Marshal(settings)
	if err != nil {
		return nil, "", err
	}
	err = os.WriteFile(
		filepath.Join(trial, settingsFile),
		data,
		0o600,
	)
	if err != nil {
		return nil, "", err
	}
	var files []string
	for file, text := range sources {
		path := filepath.Join(trial, file)
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			return nil, "", err
		}
		err := os.WriteFile(
			path,
			[]byte(text),
			0o600,
		)
		if err != nil {
			return nil, "", err
		}
		files = append(files, file)
	}
	sort.Strings(files)
	checked, err := readCheckerFindings(
		trial,
		checker,
		files,
	)
	if err != nil {
		return nil, "", err
	}
	_, err = approveComments(
		trial,
		checker,
		files,
		nil,
	)
	if err != nil {
		return nil, "", err
	}
	entries, err := readApprovalEntries(trial)
	if err != nil {
		return nil, "", err
	}
	var named []namedComment
	for _, entry := range entries {
		// Approving records each gone comment as "".
		if entry.Text != "" {
			named = append(named, entry)
		}
	}
	return named, checked, nil
}

// Runs the checker on files in dir without approving, and returns what it
// printed. Exit 1 means an approved comment changed, which a draft
// expects, but Python also exits 1 when it crashes, so only a run that
// quoted the approved rule passes.
func readCheckerFindings(dir, checker string, files []string) (string, error) {
	args := append(
		[]string{checker, "--repo", dir, "--"},
		files...,
	)
	run := exec.Command("python3", args...)
	run.Dir = dir
	output, err := run.CombinedOutput()
	var exit *exec.ExitError
	if errors.As(err, &exit) && exit.ExitCode() == 1 && strings.Contains(string(output), "approved rule: ") {
		err = nil
	}
	if err != nil {
		return "", fmt.Errorf(
			"comment checker: %w: %s",
			err,
			strings.TrimSpace(string(output)),
		)
	}
	return string(output), nil
}

// Moves each approval whose comment is gone to the checker's one
// suggested new name, unless that name has its own approval. Returns
// each moved approval's old name, keyed by its new one.
func moveRenamedApprovals(
	approvals map[commentKey]string,
	named []namedComment,
	checked string,
) map[commentKey]string {
	renamed := map[commentKey]string{}
	for _, key := range listGoneApprovals(approvals, named) {
		candidates := findRenameCandidates(checked, key)
		if len(candidates) != 1 {
			continue
		}
		to := commentKey{key.file, candidates[0]}
		if _, taken := approvals[to]; taken {
			continue
		}
		approvals[to] = approvals[key]
		delete(approvals, key)
		renamed[to] = key.name
	}
	return renamed
}

// Returns each approval with text whose comment is not among named,
// sorted by file and name.
func listGoneApprovals(approvals map[commentKey]string, named []namedComment) []commentKey {
	current := map[commentKey]bool{}
	for _, c := range named {
		current[commentKey{c.File, c.Name}] = true
	}
	var gone []commentKey
	for key, text := range approvals {
		if text != "" && !current[key] {
			gone = append(gone, key)
		}
	}
	sort.Slice(gone, func(i, j int) bool {
		return gone[i].file < gone[j].file || gone[i].file == gone[j].file && gone[i].name < gone[j].name
	})
	return gone
}

var renameCandidate = regexp.MustCompile(`; possibly now (.+?) at \d+ \(`)

// Returns the names the checker printed as possible new names for the
// gone approved comment, from its "possibly now NAME at LINE" hints.
func findRenameCandidates(checked string, gone commentKey) []string {
	prefix := gone.file + " " + gone.name + ": approved: approved comment is gone"
	for _, line := range strings.Split(checked, "\n") {
		rest, ok := strings.CutPrefix(line, prefix)
		if !ok {
			continue
		}
		var names []string
		for _, match := range renameCandidate.FindAllStringSubmatch(rest, -1) {
			names = append(names, match[1])
		}
		return names
	}
	return nil
}

// Reports whether the checker found more than one comment in the file
// with key's name, so key's comment looks gone.
func isNameShared(checked string, key commentKey) bool {
	shared := regexp.MustCompile(
		`(?m)^` + regexp.QuoteMeta(key.file) + `:\d+ ` + regexp.QuoteMeta(key.name) + `: cannot be approved;`,
	)
	return shared.MatchString(checked)
}
