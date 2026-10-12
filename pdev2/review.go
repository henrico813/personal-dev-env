package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"pdev2/internal/approval"
)

type currentPR struct {
	Title string `json:"title"`
	Body  string `json:"body"`
}

// Also returns the PR's current title when pr edit fetched it, so creation
// needs no second lookup.
func approvalText(gh string, req approval.Request) (string, string, error) {
	if req.Operation != "edit" {
		return approval.RequestText(req, ""), "", nil
	}
	if req.TargetUnknown {
		return approval.RequestText(req, approval.EditHeader+"Cannot show current values: the PR number or branch is unclear.\n"), "", nil
	}
	// With no target gh edits the current branch's PR, which is not looked up
	// here, so no diff is shown.
	if req.Target == "" {
		return approval.RequestText(req, ""), "", nil
	}
	current, err := fetchCurrent(gh, req)
	if err != nil {
		// A fetch failure is shown to the approver instead of blocking the write.
		return approval.RequestText(req, approval.EditHeader+"Fetch failed: "+err.Error()+"\n"), "", nil
	}
	titleDiff, err := approval.FieldDiff("title", req.HasTitle, current.Title, req.Title)
	if err != nil {
		return "", "", err
	}
	bodyDiff, err := approval.FieldDiff("body", req.HasBody, current.Body, req.Body)
	if err != nil {
		return "", "", err
	}
	return approval.RequestText(req, approval.EditHeader+"Title:\n"+titleDiff+"Body:\n"+bodyDiff), current.Title, nil
}

// The caller holds the request lock, so a hung gh must not block other runs.
var fetchTimeout = 3 * time.Second

func fetchCurrent(gh string, req approval.Request) (currentPR, error) {
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
