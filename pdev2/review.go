package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const editHeader = "Changes from the current pull request\n"

type currentPR struct {
	Title string `json:"title"`
	Body  string `json:"body"`
}

func approvalText(gh string, req request) (string, error) {
	if req.Operation != "edit" {
		return requestText(req, ""), nil
	}
	if req.TargetUnknown {
		return requestText(req, editHeader+"Cannot show current values: the PR number or branch is unclear.\n"), nil
	}
	// With no target gh edits the current branch's PR, which is not looked up
	// here, so no diff is shown.
	if req.Target == "" {
		return requestText(req, ""), nil
	}
	current, err := fetchCurrent(gh, req)
	if err != nil {
		// A fetch failure is shown to the approver instead of blocking the write.
		return requestText(req, editHeader+"Fetch failed: "+err.Error()+"\n"), nil
	}
	titleDiff, err := fieldDiff("title", req.HasTitle, current.Title, req.Title)
	if err != nil {
		return "", err
	}
	bodyDiff, err := fieldDiff("body", req.HasBody, current.Body, req.Body)
	if err != nil {
		return "", err
	}
	return requestText(req, editHeader+"Title:\n"+titleDiff+"Body:\n"+bodyDiff), nil
}

// The caller holds the request lock, so a hung gh must not block other runs.
var fetchTimeout = 3 * time.Second

func fetchCurrent(gh string, req request) (currentPR, error) {
	args := []string{"pr", "view", req.Target, "--json", "title,body"}
	if req.Repo != "" {
		args = append(args, "--repo", req.Repo)
	}
	ctx, cancel := context.WithTimeout(context.Background(), fetchTimeout)
	defer cancel()
	var ghErr bytes.Buffer
	cmd := exec.CommandContext(ctx, gh, args...)
	cmd.Stderr = &ghErr
	cmd.WaitDelay = time.Second
	output, err := cmd.Output()
	if err != nil {
		message := strings.TrimSpace(ghErr.String())
		if ctx.Err() != nil {
			message = fmt.Sprintf("gh pr view timed out after %s", fetchTimeout)
		} else if message == "" {
			message = "unknown error"
		}
		return currentPR{}, errors.New(message)
	}
	var current currentPR
	if err := json.Unmarshal(output, &current); err != nil {
		return currentPR{}, fmt.Errorf("parse gh pr view output: %w", err)
	}
	return current, nil
}

func fieldDiff(name string, present bool, current, requested string) (string, error) {
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

func requestText(req request, edit string) string {
	if edit != "" {
		edit += "\n"
	}
	return fmt.Sprintf("%sid: %s\ntime: %d\ncommand: %s\ntitle:\n%s\nbody:\n%s\n",
		edit, req.ID, time.Now().Unix(), req.Command, req.Title, req.Body)
}
