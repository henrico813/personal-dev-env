package cmd

import (
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

// Saves the comments the owner must answer as the draft of the branch's
// review.
func saveBranchDraft(dir, checker string) (review.Draft, error) {
	name, d, err := buildBranchDraft(dir, checker)
	if err == nil {
		err = review.MoveFinishedReviewAside(name)
	}
	if err != nil {
		return review.Draft{}, fmt.Errorf("nothing was saved: %w", err)
	}
	// The branch's review now has no batch, so the draft is its first.
	d.BatchNumber = 1
	if err := review.SaveDraft(name, d); err != nil {
		return review.Draft{}, err
	}
	return d, nil
}

// Returns the top folder of the checkout at dir, and the name of its
// branch's review.
func findBranchReviewName(dir string) (string, string, error) {
	repo, err := runGit(
		dir,
		"rev-parse",
		"--show-toplevel",
	)
	if err != nil {
		return "", "", err
	}
	repo = strings.TrimSpace(repo)
	branch, err := runGit(
		repo,
		"branch",
		"--show-current",
	)
	if err != nil {
		return "", "", err
	}
	branch = strings.TrimSpace(branch)
	if branch == "" {
		return "", "", errors.New(
			"no branch is checked out, so the review has no name; check out a branch, such as with git switch -c NAME",
		)
	}
	name, err := review.ReviewNameForBranch(branch)
	return repo, name, err
}

// Returns the review name for the branch checked out at dir, and its
// draft.
func buildBranchDraft(dir, checker string) (string, review.Draft, error) {
	repo, name, err := findBranchReviewName(dir)
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

// A comment the checker says the owner must answer, and the reason it was
// listed.
type ownerChange struct {
	File    string `json:"file"`
	Name    string `json:"name"`
	Reason  string `json:"reason"`
	Old     string `json:"old"`
	Current string `json:"current"`
}

// Returns an item for each comment the owner must answer.
func buildDraft(repo, base, checker string) (review.Draft, error) {
	var changes struct {
		Items       []ownerChange `json:"items"`
		Renames     []string      `json:"renames"`
		Problems    []string      `json:"problems"`
		Unsupported []string      `json:"unsupported"`
	}
	err := runCheckerCommand(
		checker,
		nil,
		&changes,
		"changes",
		"--interface",
		fmt.Sprint(checkerJSONVersion),
		"--repo",
		repo,
		"--base",
		base,
	)
	if err != nil {
		return review.Draft{}, err
	}
	if len(changes.Problems) > 0 {
		return review.Draft{}, fmt.Errorf(
			"fix these comments first:\n%s",
			strings.Join(
				changes.Problems,
				"\n",
			),
		)
	}
	var requests []review.FeedbackRequest
	for i, c := range changes.Items {
		request := review.FeedbackRequest{
			ID:         i + 1,
			File:       c.File,
			Name:       c.Name,
			Old:        c.Old,
			Suggestion: review.SuggestKeep,
		}
		switch {
		case c.Reason == "code_removed":
			request.Suggestion, request.CodeRemoved = review.SuggestCut, true
		case c.Current != c.Old:
			request.Suggestion, request.New = review.SuggestReword, c.Current
		}
		requests = append(requests, request)
	}
	return review.Draft{
		Base: base,
		Batch: review.Batch{
			Version:  review.Version,
			Repo:     repo,
			Requests: requests,
			Renames:  changes.Renames,
		},
		// An empty list is kept as nil, which is how draft.json reads it back.
		Unsupported: append([]string(nil), changes.Unsupported...),
	}, nil
}

// Returns the text of each comment the checker names in sources. It
// approves them in a scratch copy, so the checkout's approvals never change.
func readCommentTexts(sources map[string]string, checker string) (map[commentKey]string, error) {
	// The checker exits 2 when it is given no file it can read.
	if len(sources) == 0 {
		return nil, nil
	}
	trial, err := os.MkdirTemp("", "pde-comment-review-")
	if err != nil {
		return nil, err
	}
	defer os.RemoveAll(trial)
	// The checker refuses null for either.
	err = os.WriteFile(
		filepath.Join(trial, settingsFile),
		[]byte(`{"approved": [], "avoid": {}}`),
		0o600,
	)
	if err != nil {
		return nil, err
	}
	var files []string
	for file, text := range sources {
		path := filepath.Join(trial, file)
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			return nil, err
		}
		err := os.WriteFile(
			path,
			[]byte(text),
			0o600,
		)
		if err != nil {
			return nil, err
		}
		files = append(files, file)
	}
	sort.Strings(files)
	err = approveComments(
		trial,
		checker,
		files,
	)
	if err != nil {
		return nil, err
	}
	return readApprovals(trial)
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
