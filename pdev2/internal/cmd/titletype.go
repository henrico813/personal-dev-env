package cmd

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"pdev2/internal/approval"
)

// The git-messages check that refuses a docs title when skill or prompt
// files change. Tests replace it with a fake script.
var msgCommand = func() []string {
	home, _ := os.UserHomeDir()
	script := filepath.Join(home, ".agents/skills/git-messages/scripts/msg")
	return []string{"python3", script, "check-type"}
}()

// Returns the line msg printed when it refuses a PR title, or "" when the
// title may go to the approver. A check that cannot run only warns, so PR
// writes keep working on a machine without the git-messages skill.
func findTitleRefusal(req approval.Request, gh string) string {
	if !req.HasTitle {
		return ""
	}
	var scope []string
	switch req.Operation {
	case "create":
		// A stacked PR's base is its parent branch, whose files are not
		// part of this PR.
		if base := findBaseBranch(req.Args); base != "" {
			scope = []string{"--base", base}
		}
	case "edit":
		files := listPRFiles(gh, req)
		if len(files) == 0 {
			// The checkout may be on another branch, so its files could
			// refuse a title for changes this PR does not make.
			fmt.Fprintln(stderr, "pde-gh-write: skipped the title type check: cannot list the PR's files")
			return ""
		}
		for _, path := range files {
			scope = append(scope, "--path", path)
		}
	default:
		return ""
	}
	message, err := os.CreateTemp("", "pde-gh-write-title-")
	if err != nil {
		fmt.Fprintf(stderr, "pde-gh-write: skipped the title type check: %v\n", err)
		return ""
	}
	defer os.Remove(message.Name())
	_, err = message.WriteString(req.Title + "\n")
	if closeErr := message.Close(); err == nil {
		err = closeErr
	}
	if err != nil {
		fmt.Fprintf(stderr, "pde-gh-write: skipped the title type check: %v\n", err)
		return ""
	}
	args := append(append([]string(nil), msgCommand[1:]...), message.Name())
	cmd := exec.Command(msgCommand[0], append(args, scope...)...)
	cmd.Dir = req.Directory
	output, err := cmd.Output()
	// msg exits 1 only when it refuses the title.
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) && exitErr.ExitCode() == 1 {
		return strings.TrimSpace(string(output))
	}
	if err != nil {
		fmt.Fprintf(stderr, "pde-gh-write: skipped the title type check: %v\n", err)
	}
	return ""
}

// Returns the value of --base or -B, or "" when the PR targets the default
// branch. Parse has already refused joined short options such as -Bmain.
func findBaseBranch(args []string) string {
	for index, arg := range args {
		if value, ok := strings.CutPrefix(arg, "--base="); ok {
			return value
		}
		if (arg == "--base" || arg == "-B") && index+1 < len(args) {
			return args[index+1]
		}
	}
	return ""
}

// Returns the files of the PR that pr edit changes, from GitHub, or nil when
// the PR is unclear or the lookup fails. Without a target, gh picks the
// checkout's branch's PR, as gh pr edit does.
func listPRFiles(gh string, req approval.Request) []string {
	if req.TargetUnknown {
		return nil
	}
	args := []string{"pr", "diff", "--name-only"}
	if req.Target != "" {
		args = append(args, req.Target)
	}
	if req.Repo != "" {
		args = append(args, "--repo", req.Repo)
	}
	output := ghLookup(gh, req.Directory, args...)
	return strings.FieldsFunc(output, func(r rune) bool { return r == '\n' })
}
