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
	"slices"
	"strconv"
	"strings"
	"time"

	"pdev2/internal/approval"
)

// A stack is a chain of PRs, each based on the branch of the PR below it.
// Start with checkStackOrder, run before the owner is asked, and runMerge, run
// after approval.

// A request this program will not send for approval. pde-gh-write exits
// exitRejected for it.
type refusedError struct{ reason string }

func (e refusedError) Error() string { return e.reason }

func checkStackOrder(gh string, req approval.Request) error {
	switch req.Operation {
	case "create":
		return checkCreateBase(gh, req)
	case "merge":
		if req.TargetUnknown {
			return nil
		}
		pr, err := findMergePR(gh, req.Directory, req.Repo, req.Target)
		if err != nil {
			return nil
		}
		if err := checkStackMerge(pr); err != nil {
			return err
		}
		if pr.Stack != nil {
			if _, err := buildMergeAsyncBody(req.Args, req.Title, req.Body); err != nil {
				return refusedError{err.Error()}
			}
		}
	}
	return nil
}

// Runs an approved pr merge, using GitHub's merge-async API for a PR in a
// GitHub stack, since gh pr merge fails there. Once the PR has merged, the
// open PRs based on its branch move to its base.
func runMerge(rec approval.Record, gh string, output io.Writer) int {
	target, unknown := approval.FindCommandTarget(rec.Args)
	if unknown {
		return runApproved(rec, gh, output)
	}
	pr, err := findMergePR(gh, rec.Directory, rec.Repo, target)
	if err != nil {
		// This falls back to gh pr merge, which fails for a PR in a GitHub
		// stack.
		fmt.Fprintf(output, "pde-gh-write: cannot look up the pull request, so pull requests based on its branch keep their base: %v\n", err)
		return runApproved(rec, gh, output)
	}
	if err := checkStackMerge(pr); err != nil {
		fmt.Fprintf(output, "pde-gh-write: %v\n", err)
		return exitRejected
	}
	code := 0
	if pr.Stack != nil {
		code = mergeStacked(rec, gh, pr, output)
	} else {
		code = mergePlain(rec, gh, pr, output)
	}
	// With --auto, gh succeeds before the PR merges.
	if code == 0 && !slices.Contains(rec.Args, "--auto") {
		moveDependents(rec, gh, pr, output)
	}
	return code
}

const createBaseQuery = `query($owner: String!, $name: String!, $base: String!) {
  repository(owner: $owner, name: $name) {
    defaultBranchRef { name }
    pullRequests(headRefName: $base, states: OPEN) { totalCount }
  }
}`

// Refuses a PR based on a branch with no open PR, because it cannot merge in
// order. A failed lookup also fails the request, because nothing checks it
// later.
func checkCreateBase(gh string, req approval.Request) error {
	if req.Base == "" {
		return nil
	}
	var reply struct {
		Data struct {
			Repository struct {
				DefaultBranchRef struct{ Name string }
				PullRequests     struct{ TotalCount int }
			}
		}
	}
	err := readGHJSON(gh, req.Directory, req.Repo, &reply, "api", "graphql", "-F", "owner={owner}", "-F", "name={repo}",
		"-f", "base="+req.Base, "-f", "query="+createBaseQuery)
	if err != nil {
		return fmt.Errorf("cannot check that base branch %s has a pull request: %w", req.Base, err)
	}
	repo := reply.Data.Repository
	if req.Base == repo.DefaultBranchRef.Name || repo.PullRequests.TotalCount > 0 {
		return nil
	}
	return refusedError{fmt.Sprintf("refuses a pull request based on %s, which has no open pull request; open that branch's pull request first", req.Base)}
}

// The PR a merge names, and the open PRs based on its branch.
type mergePR struct {
	Number      int
	State       string
	HeadRefName string
	BaseRefName string
	// Set when the PR is in a GitHub stacked PR group.
	Stack      *struct{ BaseRefName string }
	Dependents []dependentPR
}

type dependentPR struct {
	Number int
	Stack  *struct{} // Set when the PR is in a GitHub stacked PR group.
}

const mergePRQuery = `query($owner: String!, $name: String!, $number: Int!, $head: String!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) { state baseRefName stack { baseRefName } }
    pullRequests(baseRefName: $head, states: OPEN, first: 100) { nodes { number stack { size } } }
  }
}`

// Returns the PR a merge names and the open PRs based on its branch. An empty
// target means the checked-out branch's PR.
func findMergePR(gh, directory, repo, target string) (mergePR, error) {
	var pr mergePR
	view := []string{"pr", "view"}
	if target != "" {
		view = append(view, target)
	}
	if err := readGHJSON(gh, directory, repo, &pr, append(view, "--json", "number,headRefName")...); err != nil {
		return pr, err
	}
	var reply struct {
		Data struct {
			Repository struct {
				PullRequest struct {
					State       string
					BaseRefName string
					Stack       *struct{ BaseRefName string }
				}
				PullRequests struct{ Nodes []dependentPR }
			}
		}
	}
	err := readGHJSON(gh, directory, repo, &reply, "api", "graphql", "-F", "owner={owner}", "-F", "name={repo}",
		"-F", "number="+strconv.Itoa(pr.Number), "-f", "head="+pr.HeadRefName, "-f", "query="+mergePRQuery)
	if err != nil {
		return pr, err
	}
	found := reply.Data.Repository.PullRequest
	pr.State, pr.BaseRefName, pr.Stack = found.State, found.BaseRefName, found.Stack
	pr.Dependents = reply.Data.Repository.PullRequests.Nodes
	return pr, nil
}

// GitHub merges a stacked PR together with every open PR below it, so only
// the lowest one, based on the stack's base branch, may merge on one approval.
func checkStackMerge(pr mergePR) error {
	if pr.Stack == nil || pr.State != "OPEN" || pr.BaseRefName == pr.Stack.BaseRefName {
		return nil
	}
	return refusedError{fmt.Sprintf("refuses to merge #%d, because GitHub would also merge the open pull requests below it in its stack; merge those first, one request each", pr.Number)}
}

// The body of GitHub's merge-async request.
type asyncMerge struct {
	MergeMethod   string `json:"merge_method"`
	MergeAction   string `json:"merge_action"`
	CommitTitle   string `json:"commit_title,omitempty"`
	CommitMessage string `json:"commit_message,omitempty"`
	SHA           string `json:"sha,omitempty"`
}

// Turns gh pr merge options into the merge-async body. An option the API
// has no field for is refused, so nothing approved is dropped.
func buildMergeAsyncBody(args []string, title, body string) (asyncMerge, error) {
	request := asyncMerge{MergeAction: "direct_merge", CommitTitle: title, CommitMessage: body}
	for index := 2; index < len(args); index++ {
		name, value, inline := strings.Cut(args[index], "=")
		switch name {
		case "--squash", "-s":
			request.MergeMethod = "squash"
		case "--merge", "-m":
			request.MergeMethod = "merge"
		case "--rebase", "-r":
			request.MergeMethod = "rebase"
		case "--match-head-commit", "--subject", "-t", "--body", "-b", "--body-file", "-F", "--repo", "-R":
			// Parse has checked that each of these has a value.
			if !inline {
				index++
				value = args[index]
			}
			if name == "--match-head-commit" {
				request.SHA = value
			}
		default:
			if strings.HasPrefix(name, "-") {
				return request, fmt.Errorf("refuses %s for a pull request in a GitHub stack, because its merge API has no such option", name)
			}
		}
	}
	if request.MergeMethod == "" {
		return request, errors.New("a pull request in a GitHub stack needs --squash, --merge or --rebase")
	}
	return request, nil
}

// How long GitHub may take to merge a stacked PR before this program stops
// waiting. An agent command approved late in its 90-second wait may still be
// killed first, which leaves the record running.
const mergeWait = 25 * time.Second

// How often the result of a stacked PR merge is checked.
var mergePoll = time.Second

type mergeResult struct {
	Status  string
	Details struct {
		Message string
		UUID    string
	}
}

// Asks GitHub to merge the PR and waits up to mergeWait for the result.
func mergeStacked(rec approval.Record, gh string, pr mergePR, output io.Writer) int {
	request, err := buildMergeAsyncBody(rec.Args, rec.Title, rec.Body)
	if err != nil {
		fmt.Fprintf(output, "pde-gh-write: %v\n", err)
		return exitRejected
	}
	body, err := json.Marshal(request)
	if err != nil {
		fmt.Fprintf(output, "pde-gh-write: %v\n", err)
		return 1
	}
	path := fmt.Sprintf("repos/{owner}/{repo}/pulls/%d/merge-async", pr.Number)
	reply, err := runGHWrite(gh, rec, bytes.NewReader(body), "api", "--method", "PUT", path, "--input", "-")
	// GitHub answers HTTP 409 Conflict when a merge was already asked for,
	// with that merge's ID. gh treats 409 as a failure, so the reply is read
	// either way.
	var result mergeResult
	if json.Unmarshal(reply, &result) != nil {
		fmt.Fprintf(output, "pde-gh-write: GitHub did not accept the merge of #%d: %v %s\n", pr.Number, err, reply)
		return 1
	}
	for deadline := time.Now().Add(mergeWait); result.Status == "pending"; {
		if time.Now().After(deadline) {
			fmt.Fprintf(output, "pde-gh-write: GitHub is still merging #%d after %s; check it with gh pr view %d, then move the pull requests based on %s yourself\n", pr.Number, mergeWait, pr.Number, pr.HeadRefName)
			return 1
		}
		time.Sleep(mergePoll)
		var polled mergeResult
		if readGHJSON(gh, rec.Directory, rec.Repo, &polled, "api", path+"/"+result.Details.UUID) == nil {
			result = polled
		}
	}
	if result.Status != "merged" {
		fmt.Fprintf(output, "pde-gh-write: GitHub did not merge #%d: %s: %s\n", pr.Number, result.Status, result.Details.Message)
		return 1
	}
	fmt.Fprintf(output, "pde-gh-write: merged #%d with GitHub's stack merge\n", pr.Number)
	return 0
}

// GitHub moves the stacked PRs above a merged PR to the stack's base, and
// refuses to let anyone else, so only the other PRs are moved here.
func moveDependents(rec approval.Record, gh string, pr mergePR, output io.Writer) {
	for _, dependent := range pr.Dependents {
		if dependent.Stack != nil {
			continue
		}
		number := strconv.Itoa(dependent.Number)
		if _, err := runGHWrite(gh, rec, nil, "pr", "edit", number, "--base", pr.BaseRefName); err != nil {
			fmt.Fprintf(output, "pde-gh-write: merged, but could not move #%s to base %s: %v\n", number, pr.BaseRefName, err)
			continue
		}
		fmt.Fprintf(output, "pde-gh-write: moved #%s to base %s\n", number, pr.BaseRefName)
	}
}

// Stack lookups run under the request lock, so a hung gh must not hold it
// long. GitHub's GraphQL API can take several seconds.
const lookupTimeout = 10 * time.Second

// Runs a read-only gh command, stopped after lookupTimeout, and decodes the
// JSON it prints.
func readGHJSON(gh, directory, repo string, out any, args ...string) error {
	ctx, cancel := context.WithTimeout(context.Background(), lookupTimeout)
	defer cancel()
	cmd := makeGHCommand(ctx, gh, directory, repo, args...)
	var ghErr bytes.Buffer
	cmd.Stderr = &ghErr
	cmd.WaitDelay = time.Second
	output, err := cmd.Output()
	if err != nil {
		return fmt.Errorf("gh %s %s: %v: %s", args[0], args[1], err, strings.TrimSpace(ghErr.String()))
	}
	return json.Unmarshal(output, out)
}

// Runs a gh write that the request's approval covers and returns what gh
// printed on standard output. A failure's error holds standard error.
func runGHWrite(gh string, rec approval.Record, stdin io.Reader, args ...string) ([]byte, error) {
	cmd := makeGHCommand(context.Background(), gh, rec.Directory, rec.Repo, args...)
	cmd.Env = append(cmd.Env, "PDE_GH_WRITE=1")
	cmd.Stdin = stdin
	var ghErr bytes.Buffer
	cmd.Stderr = &ghErr
	output, err := cmd.Output()
	if err != nil {
		err = fmt.Errorf("%v: %s", err, strings.TrimSpace(ghErr.String()))
	}
	return output, err
}

// Returns a gh command that, when repo is set, uses it for {owner} and {repo}
// instead of the checkout's remote.
func makeGHCommand(ctx context.Context, gh, directory, repo string, args ...string) *exec.Cmd {
	cmd := exec.CommandContext(ctx, gh, args...)
	cmd.Dir = directory
	cmd.Env = os.Environ()
	if repo != "" {
		cmd.Env = append(cmd.Env, "GH_REPO="+repo)
	}
	return cmd
}

// Runs gh pr merge and reports the merge, since gh prints nothing after one
// when its output is not a terminal.
func mergePlain(rec approval.Record, gh string, pr mergePR, output io.Writer) int {
	code := runApproved(rec, gh, output)
	// With --auto, gh succeeds once the merge is queued.
	if code == 0 && !slices.Contains(rec.Args, "--auto") {
		fmt.Fprintf(output, "pde-gh-write: merged #%d with gh pr merge\n", pr.Number)
	}
	return code
}
