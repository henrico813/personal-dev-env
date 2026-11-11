package view

import (
	"cmp"
	"slices"
	"strconv"
	"strings"

	"prs/internal/github"
)

// The words of prs status. The owner may still rename them, so they stay
// together here.
const (
	statusHeading = "Open pull requests in REPO, in merge order:"
	statusNone    = "No open pull requests in REPO."
	blockedBy     = "Blocked by: "
	noBlocker     = "nothing"
)

// Returns a Markdown list of repo's open pull requests in merge order, for
// agents to paste into a status update. The first one that nothing blocks
// is in bold, as the one to review first.
func RenderStatus(
	repo string,
	prs []github.OpenPullRequest,
) string {
	if len(prs) == 0 {
		return strings.Replace(
			statusNone,
			"REPO",
			repo,
			1,
		) + "\n"
	}
	ordered, parents := orderForMerge(prs)
	var out strings.Builder
	out.WriteString(strings.Replace(
		statusHeading,
		"REPO",
		repo,
		1,
	) + "\n\n")
	marked := false
	for i, pr := range ordered {
		blockers := listBlockers(
			pr,
			parents,
		)
		blocked := strings.Join(
			blockers,
			", ",
		)
		if blocked == "" {
			blocked = noBlocker
		}
		item := pr.Repo() + " #" + strconv.Itoa(pr.Number) + ": " +
			findProblemLine(pr) + " " +
			describeChecks(pr.StatusCheckRollup) + " " +
			blockedBy + blocked + "."
		if len(blockers) == 0 && !marked {
			item = "**" + item + "**"
			marked = true
		}
		out.WriteString(strconv.Itoa(i+1) + ". " + item + "\n")
	}
	return out.String()
}

// Returns prs in merge order and the number each stacked one waits for. A
// pull request based on another's head branch comes right after it, and
// the rest come by number.
func orderForMerge(prs []github.OpenPullRequest) (
	[]github.OpenPullRequest,
	map[int]int,
) {
	sorted := slices.Clone(prs)
	slices.SortFunc(
		sorted,
		func(a, b github.OpenPullRequest) int {
			return cmp.Compare(
				a.Number,
				b.Number,
			)
		},
	)
	heads := map[string]int{}
	for _, pr := range sorted {
		// A fork's branch can share a name such as main with this
		// repository's, so only this repository's branches start a stack.
		if _, taken := heads[pr.HeadRefName]; !taken && !pr.IsCrossRepository {
			heads[pr.HeadRefName] = pr.Number
		}
	}
	parents := map[int]int{}
	children := map[int][]github.OpenPullRequest{}
	for _, pr := range sorted {
		if parent, ok := heads[pr.BaseRefName]; ok && parent != pr.Number {
			parents[pr.Number] = parent
			children[parent] = append(
				children[parent],
				pr,
			)
		}
	}
	var ordered []github.OpenPullRequest
	placed := map[int]bool{}
	var place func(github.OpenPullRequest)
	place = func(pr github.OpenPullRequest) {
		if placed[pr.Number] {
			return
		}
		placed[pr.Number] = true
		ordered = append(
			ordered,
			pr,
		)
		for _, child := range children[pr.Number] {
			place(child)
		}
	}
	for _, pr := range sorted {
		if _, stacked := parents[pr.Number]; !stacked {
			place(pr)
		}
	}
	// Pull requests based on each other in a loop have no first one, so
	// they come last rather than not at all.
	for _, pr := range sorted {
		place(pr)
	}
	return ordered, parents
}

// Returns what keeps pr from merging, such as "waits for #12", in the
// order a reviewer deals with them.
func listBlockers(
	pr github.OpenPullRequest,
	parents map[int]int,
) []string {
	var blockers []string
	if pr.IsDraft {
		blockers = append(
			blockers,
			"draft",
		)
	}
	if parent, ok := parents[pr.Number]; ok {
		blockers = append(
			blockers,
			"waits for #"+strconv.Itoa(parent),
		)
	}
	if pr.MergeStateStatus == "DIRTY" {
		blockers = append(
			blockers,
			"merge conflict",
		)
	}
	failed, running := countUnfinishedChecks(pr.StatusCheckRollup)
	if len(failed) > 0 {
		blockers = append(
			blockers,
			"failing checks",
		)
	}
	if running > 0 {
		blockers = append(
			blockers,
			"checks running",
		)
	}
	if pr.ReviewDecision == "CHANGES_REQUESTED" {
		blockers = append(
			blockers,
			"changes requested",
		)
	}
	return blockers
}

// Returns a sentence on the checks that names each failed check once. A
// failed check outweighs one still running.
func describeChecks(checks []github.Check) string {
	failed, running := countUnfinishedChecks(checks)
	total := strconv.Itoa(len(checks))
	switch {
	case len(checks) == 0:
		return "No checks."
	case len(failed) > 0:
		names := slices.Clone(failed)
		slices.Sort(names)
		return strconv.Itoa(len(failed)) + " of " + total + " checks fail: " +
			strings.Join(
				slices.Compact(names),
				", ",
			) + "."
	case running > 0:
		return strconv.Itoa(running) + " of " + total + " checks running."
	}
	return "Checks pass."
}

// Returns the names of the failed checks and how many still run.
func countUnfinishedChecks(checks []github.Check) (
	[]string,
	int,
) {
	var failed []string
	running := 0
	for _, check := range checks {
		switch check.Result() {
		case "fail":
			failed = append(
				failed,
				check.Label(),
			)
		case "pending":
			running++
		}
	}
	return failed, running
}

// Returns the first sentence of the description's Overview section, or of
// its first paragraph when it has none, or else the title.
func findProblemLine(pr github.OpenPullRequest) string {
	paragraphs := splitParagraphs(pr.Body)
	for i, paragraph := range paragraphs {
		if strings.EqualFold(
			paragraph,
			"## Overview",
		) && i+1 < len(paragraphs) && !strings.HasPrefix(
			paragraphs[i+1],
			"#",
		) {
			return cutFirstSentence(paragraphs[i+1])
		}
	}
	for _, paragraph := range paragraphs {
		if !strings.HasPrefix(
			paragraph,
			"#",
		) {
			return cutFirstSentence(paragraph)
		}
	}
	return cutFirstSentence(pr.Title)
}

// Returns the paragraphs of a Markdown text with each one's lines joined by
// spaces. A heading line is a paragraph of its own.
func splitParagraphs(text string) []string {
	var paragraphs, lines []string
	endParagraph := func() {
		if len(lines) > 0 {
			paragraphs = append(
				paragraphs,
				strings.Join(
					lines,
					" ",
				),
			)
			lines = nil
		}
	}
	for _, line := range strings.Split(
		text,
		"\n",
	) {
		line = strings.TrimSpace(line)
		switch {
		case line == "":
			endParagraph()
		case strings.HasPrefix(
			line,
			"#",
		):
			endParagraph()
			paragraphs = append(
				paragraphs,
				line,
			)
		default:
			lines = append(
				lines,
				line,
			)
		}
	}
	endParagraph()
	return paragraphs
}

// Returns text up to the end of its first sentence, ending with a period
// when it had no closing mark.
func cutFirstSentence(text string) string {
	if end := strings.Index(
		text,
		". ",
	); end >= 0 {
		return text[:end+1]
	}
	if strings.TrimRight(
		text,
		".?!",
	) != text {
		return text
	}
	return text + "."
}
