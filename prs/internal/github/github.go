// Package github reads pull requests through gh, never writing, starting
// at ListPullRequests for the PR list with its details in one query and
// FetchExtras for what the other screens show.
package github

import (
	"context"
	"encoding/json"
	"errors"
	"os/exec"
	"strconv"
	"strings"
	"time"
)

// Holds the details of one pull request, which the PR list screen's side
// pane and the top of the PR summary screen show.
type PullRequest struct {
	Number            int        `json:"number"`
	Title             string     `json:"title"`
	State             string     `json:"state"`
	IsDraft           bool       `json:"isDraft"`
	Author            Actor      `json:"author"`
	Repository        Repository `json:"repository"`
	BaseRefName       string     `json:"baseRefName"`
	HeadRefName       string     `json:"headRefName"`
	ReviewDecision    string     `json:"reviewDecision"`
	MergeStateStatus  string     `json:"mergeStateStatus"`
	Additions         int        `json:"additions"`
	Deletions         int        `json:"deletions"`
	ChangedFiles      int        `json:"changedFiles"`
	Labels            []Label    `json:"labels"`
	StatusCheckRollup []Check    `json:"statusCheckRollup"`
	// Changes with every push, comment and review, so it keys the cache.
	UpdatedAt time.Time `json:"updatedAt"`
}

type Actor struct {
	Login string `json:"login"`
}

type Repository struct {
	NameWithOwner string `json:"nameWithOwner"`
}

type Label struct {
	Name string `json:"name"`
}

// Holds one check run or commit status. GitHub reports the two kinds in
// different fields, and Result reads the right ones.
type Check struct {
	Kind       string `json:"__typename"`
	Name       string `json:"name"`
	Context    string `json:"context"`
	Status     string `json:"status"`
	Conclusion string `json:"conclusion"`
	State      string `json:"state"`
}

type ChangedFile struct {
	Path      string `json:"path"`
	Additions int    `json:"additions"`
	Deletions int    `json:"deletions"`
}

// Holds what the PR summary and comments screens show beyond the details.
// Files has at most 100 entries, and ChangedFiles says how many there are.
type Extras struct {
	Body     string        `json:"body"`
	Files    []ChangedFile `json:"files"`
	Feedback Feedback      `json:"feedback"`
}

// Holds a pull request's comments, reviews and code threads.
type Feedback struct {
	Comments []Comment `json:"comments"`
	Reviews  []Review  `json:"reviews"`
	Threads  []Thread  `json:"threads"`
}

type Comment struct {
	Author    Actor     `json:"author"`
	CreatedAt time.Time `json:"createdAt"`
	Body      string    `json:"body"`
	// The code a thread comment is on. Conversation comments have none.
	DiffHunk string `json:"diffHunk"`
}

// Holds one review. SubmittedAt is zero for a PENDING review, which is the
// gh user's own unsent draft.
type Review struct {
	Author      Actor     `json:"author"`
	State       string    `json:"state"`
	SubmittedAt time.Time `json:"submittedAt"`
	Body        string    `json:"body"`
}

// Holds a thread of comments on a line of code. Line is 0 when the line
// is gone from the latest diff, and OriginalLine then says where it was.
type Thread struct {
	IsResolved   bool      `json:"isResolved"`
	IsOutdated   bool      `json:"isOutdated"`
	Path         string    `json:"path"`
	Line         int       `json:"line"`
	OriginalLine int       `json:"originalLine"`
	Comments     []Comment `json:"comments"`
}

// Returns pass, fail or pending.
func (c Check) Result() string {
	outcome := c.Conclusion
	if c.Kind == "StatusContext" {
		outcome = c.State
	} else if c.Status != "COMPLETED" {
		outcome = "PENDING"
	}
	switch outcome {
	case "SUCCESS", "NEUTRAL", "SKIPPED":
		return "pass"
	case "PENDING", "EXPECTED":
		return "pending"
	}
	return "fail"
}

// Returns the check's name, which a commit status calls its context.
func (c Check) Label() string {
	if c.Name != "" {
		return c.Name
	}
	return c.Context
}

// Returns OWNER/NAME.
func (pr PullRequest) Repo() string {
	return pr.Repository.NameWithOwner
}

const detailsFragment = `
fragment details on PullRequest {
  number title state isDraft author { login } repository { nameWithOwner }
  baseRefName headRefName reviewDecision mergeStateStatus
  additions deletions changedFiles updatedAt
  labels(first: 20) { nodes { name } }
  commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 100) { nodes {
    __typename
    ... on CheckRun { name status conclusion }
    ... on StatusContext { context state }
  } } } } } }
}`

// GitHub returns the oldest 100 comments, reviews and threads, and the
// oldest 50 comments of each thread, so newer ones past those are not shown.
const extrasFragment = `
fragment extras on PullRequest {
  body
  files(first: 100) { nodes { path additions deletions } }
  comments(first: 100) { nodes { author { login } createdAt body } }
  reviews(first: 100) { nodes { author { login } state submittedAt body } }
  reviewThreads(first: 100) {
    nodes {
      isResolved isOutdated path line originalLine
      comments(first: 50) { nodes { author { login } createdAt body diffHunk } }
    }
  }
}`

const listQuery = `
query {
  search(query: "is:open is:pr involves:@me sort:updated-desc", type: ISSUE, first: 50) {
    nodes { ...details }
  }
}` + detailsFragment

const extrasQuery = `
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) { pullRequest(number: $number) { ...extras } }
}` + extrasFragment

const pullRequestQuery = `
query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) { pullRequest(number: $number) { ...details ...extras } }
}` + detailsFragment + extrasFragment

// Holds a pull request as GraphQL nests it, with lists under nodes.
type pullRequestNode struct {
	PullRequest
	Labels struct {
		Nodes []Label `json:"nodes"`
	} `json:"labels"`
	Commits struct {
		Nodes []struct {
			Commit struct {
				StatusCheckRollup *struct {
					Contexts struct {
						Nodes []Check `json:"nodes"`
					} `json:"contexts"`
				} `json:"statusCheckRollup"`
			} `json:"commit"`
		} `json:"nodes"`
	} `json:"commits"`
	Body  string `json:"body"`
	Files struct {
		Nodes []ChangedFile `json:"nodes"`
	} `json:"files"`
	Comments struct {
		Nodes []Comment `json:"nodes"`
	} `json:"comments"`
	Reviews struct {
		Nodes []Review `json:"nodes"`
	} `json:"reviews"`
	ReviewThreads struct {
		Nodes []struct {
			Thread
			// A thread's comments are nested under nodes too. The tag
			// matches Thread's, so this field wins when GitHub's JSON decodes.
			Comments struct {
				Nodes []Comment `json:"nodes"`
			} `json:"comments"`
		} `json:"nodes"`
	} `json:"reviewThreads"`
}

func (n pullRequestNode) toPullRequest() PullRequest {
	pr := n.PullRequest
	pr.Labels = n.Labels.Nodes
	for _, commit := range n.Commits.Nodes {
		if rollup := commit.Commit.StatusCheckRollup; rollup != nil {
			pr.StatusCheckRollup = rollup.Contexts.Nodes
		}
	}
	return pr
}

func (n pullRequestNode) toExtras() Extras {
	extras := Extras{
		Body:  n.Body,
		Files: n.Files.Nodes,
		Feedback: Feedback{
			Comments: n.Comments.Nodes,
			Reviews:  n.Reviews.Nodes,
		},
	}
	for _, node := range n.ReviewThreads.Nodes {
		thread := node.Thread
		thread.Comments = node.Comments.Nodes
		extras.Feedback.Threads = append(
			extras.Feedback.Threads,
			thread,
		)
	}
	return extras
}

// Returns up to 50 open pull requests that involve the gh user, most
// recently updated first, with every field the details show.
func ListPullRequests(ctx context.Context) (
	[]PullRequest,
	error,
) {
	var reply struct {
		Data struct {
			Search struct {
				Nodes []pullRequestNode `json:"nodes"`
			} `json:"search"`
		} `json:"data"`
	}
	err := runGraphQL(
		ctx,
		&reply,
		listQuery,
	)
	var prs []PullRequest
	for _, node := range reply.Data.Search.Nodes {
		prs = append(
			prs,
			node.toPullRequest(),
		)
	}
	return prs, err
}

// Returns the description, files, comments, reviews and code threads.
func FetchExtras(
	ctx context.Context,
	repo string,
	number int,
) (
	Extras,
	error,
) {
	node, err := fetchPullRequestNode(
		ctx,
		extrasQuery,
		repo,
		number,
	)
	return node.toExtras(), err
}

// Returns one pull request's details, description, files and comments in
// one query, for a pull request that is not in the PR list.
func FetchPullRequest(
	ctx context.Context,
	repo string,
	number int,
) (
	PullRequest,
	Extras,
	error,
) {
	node, err := fetchPullRequestNode(
		ctx,
		pullRequestQuery,
		repo,
		number,
	)
	return node.toPullRequest(), node.toExtras(), err
}

// Returns the pull request's whole diff as git prints it.
func FetchDiff(
	ctx context.Context,
	repo string,
	number int,
) (
	[]byte,
	error,
) {
	return runGH(
		ctx,
		"pr",
		"diff",
		strconv.Itoa(number),
		"--repo="+repo,
		"--color=never",
	)
}

// Returns OWNER/NAME of the repository in the current folder. The error
// holds gh's reason, such as a folder outside any GitHub repository.
func FindCurrentRepo() (
	string,
	error,
) {
	out, err := runGH(
		context.Background(),
		"repo",
		"view",
		"--json=nameWithOwner",
		"--jq=.nameWithOwner",
	)
	return strings.TrimSpace(string(out)), err
}

func fetchPullRequestNode(
	ctx context.Context,
	query,
	repo string,
	number int,
) (
	pullRequestNode,
	error,
) {
	var reply struct {
		Data struct {
			Repository struct {
				PullRequest pullRequestNode `json:"pullRequest"`
			} `json:"repository"`
		} `json:"data"`
	}
	owner, name, _ := strings.Cut(
		repo,
		"/",
	)
	err := runGraphQL(
		ctx,
		&reply,
		query,
		"--field=owner="+owner,
		"--field=name="+name,
		"--field=number="+strconv.Itoa(number),
	)
	return reply.Data.Repository.PullRequest, err
}

func runGraphQL(
	ctx context.Context,
	into any,
	query string,
	fields ...string,
) error {
	args := append(
		[]string{
			"api",
			"graphql",
			"--raw-field=query=" + query,
		},
		fields...,
	)
	out, err := runGH(
		ctx,
		args...,
	)
	if err != nil {
		return err
	}
	return json.Unmarshal(
		out,
		into,
	)
}

// Returns what gh printed, and kills gh when ctx ends. A failure's error
// holds gh's own message, such as the HTTP status GitHub sent.
func runGH(
	ctx context.Context,
	args ...string,
) (
	[]byte,
	error,
) {
	cmd := exec.CommandContext(
		ctx,
		"gh",
		args...,
	)
	// Once ctx ends and gh is killed, a child of gh that still holds its
	// output open cannot keep Output waiting past this.
	cmd.WaitDelay = time.Second
	out, err := cmd.Output()
	var exit *exec.ExitError
	if errors.As(
		err,
		&exit,
	) {
		if text := strings.TrimSpace(string(exit.Stderr)); text != "" {
			return nil, errors.New(text)
		}
	}
	if err != nil {
		return nil, err
	}
	return out, nil
}
