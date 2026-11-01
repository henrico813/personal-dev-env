package app

import (
	"bytes"
	"context"
	"os/exec"
	"strconv"
	"strings"

	"prs/internal/cache"
	"prs/internal/codewrap"
	"prs/internal/github"
	"prs/internal/view"
)

// The PR summary screen's own keys, besides h and q.
const summaryKeys = "c quit c\nd quit d\n? quit ?\n"

const summaryHelp = `j k      scroll
J K      half page down, up
c        comments screen
d        diff screen
h q      back
g G      top, bottom
/        find; Esc cancels it
`

// Shows the PR summary screen, as _summary REPO NUMBER.
func (a *prsApp) openSummary(args []string) int {
	repo, number := args[0], readNumber(args[1])
	pr, extras, err := a.loadPullRequest(
		repo,
		number,
	)
	if err != nil {
		a.printError(err.Error())
		a.waitForKey("Could not get #" + strconv.Itoa(number) + ".")
		return 0
	}
	a.showSummary(
		pr,
		extras,
	)
	return 0
}

// Shows one PR's summary screen without the PR list, as prs REPO NUMBER.
func (a *prsApp) openSummaryAlone(
	repo string,
	number int,
) int {
	pr, extras, err := a.loadPullRequest(
		repo,
		number,
	)
	if err != nil {
		a.printError(err.Error())
		a.printError("Could not get #" + strconv.Itoa(number) + ".")
		return 1
	}
	a.showSummary(
		pr,
		extras,
	)
	return 0
}

// Shows the PR summary screen until h or q, opening the comments or diff
// screen on c or d and coming back to it after.
func (a *prsApp) showSummary(
	pr github.PullRequest,
	extras github.Extras,
) {
	for {
		width := measureScreenWidth()
		key := a.openInLess(
			fitLessPrompt(
				width,
				"#"+strconv.Itoa(pr.Number),
				"c comments  d diff  J K page  h back  ? help",
				"c comments  d diff  h back  ? help",
			),
			codewrap.Wrap(
				a.renderSummary(
					pr,
					extras,
					width,
				),
				width,
				0,
			),
			summaryKeys,
		)
		switch key {
		case 'c':
			a.showComments(
				pr,
				extras.Feedback,
			)
		case 'd':
			a.showDiff(pr)
		case '?':
			a.showHelpPage(summaryHelp)
		default:
			return
		}
	}
}

// Returns the PR summary screen's text, which is the details, the comment
// count, the files and the description, with prose wrapped to width.
func (a *prsApp) renderSummary(
	pr github.PullRequest,
	extras github.Extras,
	width int,
) string {
	style := a.makeStyle(width)
	var out strings.Builder
	out.WriteString(view.RenderDetails(
		pr,
		view.Full,
		style,
	))
	count := view.FormatCommentCount(extras.Feedback)
	if count != view.NoComments {
		count += "  c to read"
	}
	out.WriteString(count + "\n")
	out.WriteString(view.RenderFileSummary(
		pr,
		extras,
		style,
	))
	out.WriteString(a.highlightMarkdown(view.WrapProse(
		extras.Body,
		width,
	) + "\n"))
	return out.String()
}

// Returns a pull request's details, description, files and comments from the
// cache, fetching and saving what is missing. A loading line shows while it
// fetches.
func (a *prsApp) loadPullRequest(
	repo string,
	number int,
) (
	github.PullRequest,
	github.Extras,
	error,
) {
	store, storeErr := cache.Open()
	var pr github.PullRequest
	listed := false
	if storeErr == nil {
		pr, listed = store.FindPullRequest(
			repo,
			number,
		)
	}
	if listed {
		if extras, ok := store.ReadExtras(pr); ok {
			return pr, extras, nil
		}
	}
	a.printLine(clearScreen + "Getting #" + strconv.Itoa(number))
	var extras github.Extras
	var err error
	if listed {
		extras, err = github.FetchExtras(
			context.Background(),
			repo,
			number,
		)
	} else {
		pr, extras, err = github.FetchPullRequest(
			context.Background(),
			repo,
			number,
		)
	}
	if err == nil && storeErr == nil {
		_ = store.WriteExtras(
			pr,
			extras,
		)
	}
	return pr, extras, err
}

// Returns Markdown with bat's colors, or as it is when colors are off or
// bat is missing. The ansi theme keeps to the 16 basic colors, where
// BAT_THEME may use 24-bit ones.
func (a *prsApp) highlightMarkdown(text string) string {
	if !a.color {
		return text
	}
	var out bytes.Buffer
	cmd := exec.Command(
		"bat",
		"--language=md",
		"--style=plain",
		"--theme=ansi",
		"--color=always",
		"--paging=never",
	)
	cmd.Stdin = strings.NewReader(text)
	cmd.Stdout = &out
	if err := cmd.Run(); err != nil {
		return text
	}
	return out.String()
}
