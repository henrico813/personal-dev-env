package cmd

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
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

// Lets the approver see which PR a comment, merge, or review is for. Empty
// when the request names its own title or the lookup fails. pr edit reuses
// the title its review already fetched.
func currentTitle(gh string, req approval.Request, fetched string) string {
	if req.HasTitle || req.Target == "" {
		return ""
	}
	if req.Operation == "edit" {
		return fetched
	}
	args := []string{"pr", "view", req.Target, "--json", "title", "--jq", ".title"}
	if req.Repo != "" {
		args = append(args, "--repo", req.Repo)
	}
	return ghLookup(gh, req.Directory, args...)
}

// Read-only and short, because the request lock is held while it runs.
// Failure gives an empty result.
func ghLookup(gh, directory string, args ...string) string {
	ctx, cancel := context.WithTimeout(context.Background(), fetchTimeout)
	defer cancel()
	cmd := exec.CommandContext(ctx, gh, args...)
	cmd.Dir = directory
	output, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(output))
}

// The first gh on PATH is the guard ~/.local/bin/gh; it blocks PR writes unless
// PDE_GH_WRITE=1 is set. The real gh is the next executable gh on PATH that
// is neither the guard nor this program. It returns the PATH entry, not the
// symlink target: aqua's gh links to aqua-proxy, which only runs gh when
// started under that name. Empty and relative entries are skipped: they
// name the working directory, the agent's checkout, which may hold a gh.
func findRealGH() string {
	self, _ := os.Executable()
	self, _ = filepath.EvalSymlinks(self)
	guard := ""
	for _, dir := range strings.Split(os.Getenv("PATH"), ":") {
		if !filepath.IsAbs(dir) {
			continue
		}
		path := filepath.Join(dir, "gh")
		target, err := filepath.EvalSymlinks(path)
		if err != nil || target == self || !isExecutable(target) {
			continue
		}
		if guard == "" {
			guard = target
		} else if target != guard {
			return path
		}
	}
	return ""
}

func isExecutable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir() && info.Mode()&0o111 != 0
}

// Keeps a noisy gh command from bloating the record that reruns print.
const maxOutput = 64 << 10

// Keeps the first maxOutput bytes and accepts the rest unwritten, so gh never
// sees a short write.
type limitedBuffer struct{ bytes.Buffer }

func (b *limitedBuffer) Write(p []byte) (int, error) {
	kept := p[:min(len(p), max(0, maxOutput-b.Len()))]
	_, _ = b.Buffer.Write(kept)
	return len(p), nil
}

func runGH(rec approval.Record, gh string, output io.Writer) int {
	if gh == "" || !isExecutable(gh) {
		// The request stays approved, so rerunning the command runs it.
		fmt.Fprintf(stderr, "pde-gh-write: cannot find gh to run approved %s; rerun this exact command\n", rec.ID)
		return 1
	}
	rec.State = approval.StateRunning
	if err := approval.Save(rec); err != nil {
		return printError(err)
	}
	capture := &limitedBuffer{}
	cmd := exec.Command(gh, rec.Args...)
	cmd.Dir = rec.Directory
	cmd.Stdout, cmd.Stderr = capture, capture
	if rec.BodyFile {
		cmd.Stdin = strings.NewReader(rec.Body)
	}
	// Lets this one approved command through the gh guard.
	cmd.Env = append(os.Environ(), "PDE_GH_WRITE=1")
	rec.Exit = 0
	err := cmd.Run()
	rec.Output = capture.String()
	if err != nil {
		var exitErr *exec.ExitError
		rec.Exit = 1
		if errors.As(err, &exitErr) {
			rec.Exit = exitErr.ExitCode()
		} else {
			rec.Output += fmt.Sprintf("pde-gh-write: %v\n", err)
		}
	}
	rec.State = approval.StateRan
	if err := approval.Save(rec); err != nil {
		// The record stays running, so reruns report interrupted and never rerun.
		fmt.Fprintf(stderr, "pde-gh-write: gh exited %d but the result was not saved: %v\n", rec.Exit, err)
	}
	fmt.Fprint(output, rec.Output)
	return rec.Exit
}
