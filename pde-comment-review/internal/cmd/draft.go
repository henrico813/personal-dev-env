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
	"slices"
	"sort"
	"strings"

	"pde-comment-review/internal/review"
)

// Saves the comments the owner must answer as the draft of the branch's
// review.
func saveBranchDraft(dir, checker string) (review.Draft, error) {
	name, d, err := buildBranchDraft(dir, checker)
	if err != nil {
		return review.Draft{}, fmt.Errorf("nothing was saved: %w", err)
	}
	if err := review.SaveDraft(name, d); err != nil {
		return review.Draft{}, err
	}
	return d, nil
}

// Returns the review name for the branch checked out at dir, and its
// draft.
func buildBranchDraft(dir, checker string) (string, review.Draft, error) {
	repo, err := runGit(
		dir,
		"rev-parse",
		"--show-toplevel",
	)
	if err != nil {
		return "", review.Draft{}, err
	}
	repo = strings.TrimSpace(repo)
	branch, err := runGit(
		repo,
		"branch",
		"--show-current",
	)
	if err != nil {
		return "", review.Draft{}, err
	}
	branch = strings.TrimSpace(branch)
	if branch == "" {
		return "", review.Draft{}, errors.New(
			"no branch is checked out, so the review has no name; check out a branch, such as with git switch -c NAME",
		)
	}
	name, err := review.ReviewNameForBranch(branch)
	if err != nil {
		return "", review.Draft{}, err
	}
	// A new worktree has none, and without it an edited approved comment
	// would look new, so the owner would never see the approved text.
	if _, err := os.Stat(filepath.Join(repo, settingsFile)); errors.Is(err, fs.ErrNotExist) {
		return "", review.Draft{}, fmt.Errorf(
			`%s is missing, so the draft cannot tell which comments the owner approved; copy it from the checkout where the owner last approved comments, such as one that git worktree list shows. Only if the owner confirms this repository never had approved comments, create an empty one with: echo '{}' > %s`,
			settingsFile,
			settingsFile,
		)
	}
	base, err := findDraftBase(repo)
	if err != nil {
		return "", review.Draft{}, err
	}
	d, err := buildDraft(
		repo,
		base,
		checker,
	)
	return name, d, err
}

// Returns the commit where the branch left the remote's default branch,
// such as origin/main.
func findDraftBase(repo string) (string, error) {
	listed, err := runGit(
		repo,
		"for-each-ref",
		"--format=%(refname)",
		"refs/remotes/*/HEAD",
	)
	if err != nil {
		return "", err
	}
	defaults := strings.Fields(listed)
	switch {
	case len(defaults) == 0:
		return "", errors.New(
			"no remote has a default branch to compare the branch with; set one with git remote set-head REMOTE --auto",
		)
	// Git 2.48 and later set a missing default again on each fetch, so
	// deleting one lasts only with followRemoteHEAD off.
	case len(defaults) > 1:
		remotes := strings.Join(defaults, " and ")
		remotes = strings.ReplaceAll(remotes, "refs/remotes/", "")
		remotes = strings.ReplaceAll(remotes, "/HEAD", "")
		return "", fmt.Errorf(
			"remotes %s each have a default branch, so which one to compare the branch with is unclear; for each remote but one, run git remote set-head NAME -d and git config remote.NAME.followRemoteHEAD never",
			remotes,
		)
	}
	base, err := runGit(
		repo,
		"merge-base",
		"HEAD",
		defaults[0],
	)
	// merge-base exits 1, printing nothing, when there is no shared commit.
	var exit *exec.ExitError
	if errors.As(err, &exit) && exit.ExitCode() == 1 {
		return "", fmt.Errorf(
			"the branch shares no commit with %s, so its changes are unknown; if this clone is shallow, fetch its history with git fetch --unshallow",
			strings.TrimPrefix(defaults[0], "refs/remotes/"),
		)
	}
	return strings.TrimSpace(base), err
}

// Returns an item for each comment the owner must answer.
func buildDraft(repo, base, checker string) (review.Draft, error) {
	changed, err := listChangedGoFiles(repo, base)
	if err != nil {
		return review.Draft{}, err
	}
	approvals, renames, err := readMovedApprovals(repo, changed)
	if err != nil {
		return review.Draft{}, err
	}
	named, checked, err := readCheckoutComments(
		repo,
		checker,
		changed,
		approvals,
	)
	if err != nil {
		return review.Draft{}, err
	}
	textAtBase, err := readBaseComments(
		repo,
		base,
		checker,
		changed,
	)
	if err != nil {
		return review.Draft{}, err
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
		_, inChangedFile := changed[c.File]
		before, wasAtBase := textAtBase[key]
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
		case !isApproved && (!inChangedFile || wasAtBase && before == c.Text):
			continue
		case !isApproved:
			old = c.Text
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
		return review.Draft{}, fmt.Errorf(
			"fix these comments first:\n%s",
			strings.Join(problems, "\n"),
		)
	}
	for key, from := range renamed {
		renames = append(renames, from+"="+key.name)
	}
	sort.Strings(renames)
	for i := range items {
		items[i].ID = i + 1
	}
	return review.Draft{
		Base: base,
		Proposals: review.Proposals{
			Version: review.Version,
			Repo:    repo,
			Items:   items,
			Renames: renames,
		},
	}, nil
}

// Returns each Go file in the checkout that changed since base, with its
// path at base, or "" for a new file.
func listChangedGoFiles(repo, base string) (map[string]string, error) {
	diff, err := runGit(
		repo,
		"diff",
		"-z",
		"--name-status",
		"-M",
		base,
		"--",
	)
	if err != nil {
		return nil, err
	}
	untracked, err := runGit(
		repo,
		"ls-files",
		"-z",
		"--others",
		"--exclude-standard",
	)
	if err != nil {
		return nil, err
	}
	isNUL := func(r rune) bool { return r == 0 }
	changed := map[string]string{}
	// Each change is a status and a path; a move has the old path too.
	fields := strings.FieldsFunc(diff, isNUL)
	for i := 0; i+1 < len(fields); i += 2 {
		status, path, from := fields[i], fields[i+1], fields[i+1]
		if strings.HasPrefix(status, "R") {
			i++
			path = fields[i+1]
		}
		switch {
		case status == "D" || !strings.HasSuffix(path, ".go"):
		case status == "A":
			changed[path] = ""
		default:
			changed[path] = from
		}
	}
	for _, path := range strings.FieldsFunc(untracked, isNUL) {
		if strings.HasSuffix(path, ".go") {
			changed[path] = ""
		}
	}
	return changed, nil
}

// Returns the approvals with moved files under their new paths, and an
// OLD=NEW pair for each moved file that has approvals.
func readMovedApprovals(repo string, changed map[string]string) (map[commentKey]string, []string, error) {
	approved, err := readApprovals(repo)
	if err != nil {
		return nil, nil, err
	}
	movedTo := map[string]string{}
	for path, from := range changed {
		if from != "" && from != path {
			movedTo[from] = path
		}
	}
	approvals := map[commentKey]string{}
	var renames []string
	for key, text := range approved {
		if to, ok := movedTo[key.file]; ok {
			if pair := key.file + "=" + to; !slices.Contains(renames, pair) {
				renames = append(renames, pair)
			}
			key.file = to
		}
		approvals[key] = text
	}
	return approvals, renames, nil
}

// Returns the comments in the checkout's changed files and files with
// approvals, and what the checker printed for them.
func readCheckoutComments(
	repo string,
	checker string,
	changed map[string]string,
	approvals map[commentKey]string,
) ([]namedComment, string, error) {
	files := map[string]bool{}
	for file := range changed {
		files[file] = true
	}
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

// Returns the text each changed file's comments had at base.
func readBaseComments(
	repo string,
	base string,
	checker string,
	changed map[string]string,
) (map[commentKey]string, error) {
	sources := map[string]string{}
	for file, from := range changed {
		if from == "" {
			continue
		}
		text, err := runGit(
			repo,
			"show",
			base+":"+from,
		)
		if err != nil {
			return nil, err
		}
		sources[file] = text
	}
	return readCommentTexts(sources, checker)
}

// Returns the text of each comment the checker names in sources.
func readCommentTexts(sources map[string]string, checker string) (map[commentKey]string, error) {
	named, _, err := readNamedComments(
		sources,
		nil,
		checker,
	)
	if err != nil {
		return nil, err
	}
	texts := map[commentKey]string{}
	for _, c := range named {
		texts[commentKey{c.File, c.Name}] = c.Text
	}
	return texts, nil
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
	err = approveComments(
		trial,
		checker,
		files,
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

func runGit(repo string, args ...string) (string, error) {
	gitArgs := append(
		[]string{"-C", repo},
		args...,
	)
	run := exec.Command("git", gitArgs...)
	var stderr strings.Builder
	run.Stderr = &stderr
	output, err := run.Output()
	if err != nil {
		return "", fmt.Errorf(
			"git %s: %w: %s",
			strings.Join(args, " "),
			err,
			strings.TrimSpace(stderr.String()),
		)
	}
	return string(output), nil
}
