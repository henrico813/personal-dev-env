package view

import (
	"strings"
	"testing"

	"prs/internal/github"
)

// Forks often name their branch main, like the base branch of most pull
// requests. Reading a fork's main as a stack would make every other pull
// request wait for the fork's, and nothing would be bold.
func TestRenderStatusIgnoresForkBranches(t *testing.T) {
	prs := []github.OpenPullRequest{
		{
			PullRequest: github.PullRequest{
				Number:      5,
				Title:       "Add Dutch",
				BaseRefName: "main",
				HeadRefName: "main",
			},
			IsCrossRepository: true,
		},
		{
			PullRequest: github.PullRequest{
				Number:      6,
				Title:       "Fix typo",
				BaseRefName: "main",
				HeadRefName: "fix-typo",
			},
		},
	}

	output := RenderStatus(
		"example/repo",
		prs,
	)

	if strings.Contains(
		output,
		"waits for",
	) {
		t.Errorf(
			"a fork's branch started a stack:\n%s",
			output,
		)
	}
}

// Bold marks the one pull request to review first. With two in bold, the
// reviewer cannot tell which one to start with.
func TestRenderStatusBoldsOnlyFirstUnblocked(t *testing.T) {
	prs := []github.OpenPullRequest{
		{
			PullRequest: github.PullRequest{
				Number:      5,
				Title:       "Add Dutch",
				BaseRefName: "main",
				HeadRefName: "add-dutch",
			},
		},
		{
			PullRequest: github.PullRequest{
				Number:      6,
				Title:       "Fix typo",
				BaseRefName: "main",
				HeadRefName: "fix-typo",
			},
		},
	}

	output := RenderStatus(
		"example/repo",
		prs,
	)

	if !strings.Contains(
		output,
		"1. **",
	) || strings.Contains(
		output,
		"2. **",
	) {
		t.Errorf(
			"want only the first pull request in bold:\n%s",
			output,
		)
	}
}

// A blocked pull request in bold sends the reviewer to one that cannot merge
// yet. Each case reads a GitHub field whose expected value a typo would miss.
func TestRenderStatusShowsBlockersWithoutBold(t *testing.T) {
	tests := []struct {
		name    string
		pr      github.PullRequest
		blocker string
	}{
		{
			name: "merge-conflict",
			pr: github.PullRequest{
				Number:           7,
				Title:            "Add Dutch",
				MergeStateStatus: "DIRTY",
			},
			blocker: "Blocked by: merge conflict.",
		},
		{
			name: "changes-requested",
			pr: github.PullRequest{
				Number:         7,
				Title:          "Add Dutch",
				ReviewDecision: "CHANGES_REQUESTED",
			},
			blocker: "Blocked by: changes requested.",
		},
		{
			name: "checks-running",
			pr: github.PullRequest{
				Number: 7,
				Title:  "Add Dutch",
				StatusCheckRollup: []github.Check{
					{
						Kind:   "CheckRun",
						Name:   "test",
						Status: "IN_PROGRESS",
					},
				},
			},
			blocker: "Blocked by: checks running.",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			prs := []github.OpenPullRequest{
				{
					PullRequest: tt.pr,
				},
			}

			output := RenderStatus(
				"example/repo",
				prs,
			)

			if strings.Contains(
				output,
				"**",
			) || !strings.Contains(
				output,
				tt.blocker,
			) {
				t.Errorf(
					"want %q and no bold:\n%s",
					tt.blocker,
					output,
				)
			}
		})
	}
}
