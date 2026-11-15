package installer

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-installer/internal/run"
)

// Returns extra_skills_repo from config.json with a leading ~/ expanded, or ""
// when the file or the key is missing. Only a PC with extra skills sets it.
func loadExtraSkillsRepo(home string) (string, error) {
	data, err := os.ReadFile(filepath.Join(
		home,
		".config",
		"pde",
		"config.json",
	))
	if os.IsNotExist(err) || err == nil && len(bytes.TrimSpace(data)) == 0 {
		return "", nil
	}
	if err != nil {
		return "", fmt.Errorf(
			"read PDE config: %w",
			err,
		)
	}
	var values map[string]any
	if err := json.Unmarshal(
		data,
		&values,
	); err != nil {
		return "", fmt.Errorf(
			"read extra_skills_repo in PDE config: %w",
			err,
		)
	}
	// Install reads this before the migration saves renamed keys, so an
	// old key name must still count here.
	renameOldConfigKeys(values)
	repo, _ := values["extra_skills_repo"].(string)
	if rest, found := strings.CutPrefix(
		repo,
		"~/",
	); found {
		return filepath.Join(
			home,
			rest,
		), nil
	}
	return repo, nil
}

// Fast-forwards the extra_skills_repo checkout and returns whether it moved.
// A failure only warns, because the personal update does not need it.
func updateExtraSkills(home string, runner run.Runner) bool {
	root, err := loadExtraSkillsRepo(home)
	if err == nil && root == "" {
		return false
	}
	moved := false
	if err == nil {
		moved, err = fastForwardExtraSkills(
			root,
			runner,
		)
	}
	if err != nil {
		fmt.Fprintf(
			runner.Err(),
			"warning: extra skills not updated: %v\n",
			err,
		)
	}
	return moved
}

// Fast-forwards root to the main of the remote its main tracks, and returns
// whether HEAD moved. Like the personal checkout, a checkout off main or with
// changes to tracked files stays where it is.
func fastForwardExtraSkills(root string, runner run.Runner) (bool, error) {
	reason, err := findSkipReason(
		root,
		runner,
	)
	if err != nil {
		return false, err
	}
	if reason != "" {
		return false, errors.New(reason)
	}
	// The extra skills repository is private, so git fetches it by remote name with
	// that remote's own credentials, not over a public HTTPS address.
	commit, err := fetchMain(
		root,
		readMainRemote(
			root,
			runner,
		),
		runner,
	)
	if err != nil {
		return false, err
	}
	if readCheckoutRevision(
		root,
		runner,
		"HEAD",
	) == commit {
		return false, nil
	}
	if _, err := runner.Query(
		"fast-forward the extra skills to main",
		makeGitCommand(
			root,
			"merge",
			"--ff-only",
			"--quiet",
			commit,
		),
	); err != nil {
		return false, err
	}
	return true, nil
}
