package app

import (
	"context"

	"prs/internal/github"
	"prs/internal/view"
)

// Prints the current folder's repository's open pull requests as a
// Markdown list, as prs status.
func (a *prsApp) printStatus() int {
	repo, err := github.FindCurrentRepo()
	if err != nil {
		a.printError(err.Error())
		a.printError("Could not find this folder's GitHub repository.")
		return 1
	}
	prs, err := github.ListOpenPullRequests(
		context.Background(),
		repo,
	)
	if err != nil {
		a.printError(err.Error())
		a.printError("Could not get the open pull requests of " + repo + ".")
		return 1
	}
	a.printText(view.RenderStatus(
		repo,
		prs,
	))
	return 0
}
