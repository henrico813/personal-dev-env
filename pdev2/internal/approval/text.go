package approval

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const EditHeader = "Changes from the current pull request\n"

func FieldDiff(name string, present bool, current, requested string) (string, error) {
	if !present {
		return "No changes (" + name + " not in this command)\n", nil
	}
	return unifiedDiff(current, requested)
}

// A diff error is returned instead of being reported as "No changes".
func unifiedDiff(current, requested string) (string, error) {
	dir, err := os.MkdirTemp("", "pde-review-")
	if err != nil {
		return "", err
	}
	defer os.RemoveAll(dir)
	currentPath := filepath.Join(dir, "current")
	requestedPath := filepath.Join(dir, "requested")
	if err := os.WriteFile(currentPath, []byte(current+"\n"), 0o600); err != nil {
		return "", err
	}
	if err := os.WriteFile(requestedPath, []byte(requested+"\n"), 0o600); err != nil {
		return "", err
	}
	cmd := exec.Command("diff", "-u", "--label", "current", currentPath, "--label", "requested", requestedPath)
	output, err := cmd.Output()
	var exitErr *exec.ExitError
	switch {
	case err == nil:
		return "No changes\n", nil
	case errors.As(err, &exitErr) && exitErr.ExitCode() == 1:
		return string(output), nil
	default:
		return "", fmt.Errorf("compare pull request values: %w", err)
	}
}

func RequestText(req Request, edit string) string {
	if edit != "" {
		edit += "\n"
	}
	return fmt.Sprintf("%sid: %s\ntime: %d\ncommand: %s\ntitle:\n%s\nbody:\n%s\n",
		edit, req.ID, time.Now().Unix(), req.Command, req.Title, req.Body)
}

// The popup has room for a few lines, so it shows where the request came from,
// the changes of a pr edit, and the body. The full approval text starts with
// ID, time, and command lines that would fill the popup; v shows it.
func Summary(rec Record) []string {
	summary := ContextLines(rec)
	if len(rec.Args) > 1 && rec.Args[1] == "edit" {
		summary = append(summary, editChanges(rec.ApprovalText)...)
	}
	if rec.Body != "" {
		summary = append(summary, strings.Split(rec.Body, "\n")...)
	}
	return summary
}

// Lines that say what is asked and where, in the order a reader needs them:
// operation and PR, repository, Herdr workspace and session, PR title. The
// popup and the phone question both start with these, so they cannot drift.
func ContextLines(rec Record) []string {
	what := "PR approval"
	if len(rec.Args) > 1 {
		what = rec.Args[1] + " PR"
		if target, _ := findTarget(rec.Args[1], rec.Args[2:]); target != "" {
			what = rec.Args[1] + " #" + target
		}
	}
	lines := []string{what}
	if rec.Repo != "" {
		lines = append(lines, rec.Repo)
	} else if rec.Directory != "" {
		lines = append(lines, "dir: "+filepath.Base(rec.Directory))
	}
	session := strings.Trim(rec.Workspace+" / "+rec.SessionTitle, " /")
	if session == "" {
		session = rec.SessionID
	}
	if session != "" {
		lines = append(lines, session)
	}
	if rec.Title != "" {
		lines = append(lines, rec.Title)
	} else if rec.PRTitle != "" {
		lines = append(lines, rec.PRTitle)
	}
	return lines
}

// editChanges returns the "-old" and "+new" lines of a pr edit. It stops at the
// blank line before the body, so Markdown bullets ("- item") in the body are
// not shown as removed lines.
func editChanges(approvalText string) []string {
	_, changes, found := strings.Cut(approvalText, EditHeader)
	if !found {
		return nil
	}
	changes, _, _ = strings.Cut(changes, "\n\n")
	var lines []string
	for _, line := range strings.Split(changes, "\n") {
		if (strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++")) ||
			(strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---")) {
			lines = append(lines, line)
		}
	}
	return lines
}
