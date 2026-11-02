package view

import (
	"strconv"
	"strings"
	"unicode/utf8"

	"prs/internal/github"
)

// The color of each check result.
var resultColors = map[string]string{
	"pass":    green,
	"fail":    red,
	"pending": yellow,
}

// GitHub's merge states in plain words. Others read as merge unknown.
var mergeWords = map[string]coloredWords{
	"CLEAN": {
		words: "ready",
		color: green,
	},
	"HAS_HOOKS": {
		words: "ready",
		color: green,
	},
	"BLOCKED": {
		words: "blocked",
		color: yellow,
	},
	"BEHIND": {
		words: "behind",
		color: yellow,
	},
	"DIRTY": {
		words: "conflicts",
		color: red,
	},
	"UNSTABLE": {
		words: "unstable",
		color: yellow,
	},
}

// Says how much of a pull request RenderDetails shows.
type Detail int

const (
	// The 3-row status strip, for a side pane of 3 rows or fewer. The zero
	// Detail is no length, so a Detail left unset never passes as one.
	Strip Detail = iota + 1
	// The details for a side pane taller than 3 rows.
	Long
	// The top of the PR summary screen, which adds the head branch.
	Full
)

// Returns the details, from the state, review and merge state to the checks
// and labels, at the length detail says. The status strip keeps the state,
// checks and repo rows, so a fourth row never pushes the repo off a phone.
func RenderDetails(
	pr github.PullRequest,
	detail Detail,
	s Style,
) string {
	dot := " " + s.paintGray("·") + " "
	status := s.paintBold("#"+strconv.Itoa(pr.Number)) + " " + s.paintState(pr.State)
	if pr.IsDraft {
		status += " " + s.paintGray("draft")
	}
	status += dot + s.formatReview(pr.ReviewDecision) + dot + s.formatMerge(pr.MergeStateStatus)
	checks := s.formatCheckCounts(pr.StatusCheckRollup)
	where := s.formatWhereRow(
		pr.Repo(),
		s.formatAge(pr.UpdatedAt),
		pr.Author.Login,
	)
	if detail == Strip {
		return status + "\n" + checks + "\n" + where + "\n"
	}
	var out strings.Builder
	out.WriteString(status + "\n")
	out.WriteString(s.paintBold(WrapProse(
		pr.Title,
		s.Width,
	)) + "\n")
	out.WriteString(where + "\n")
	out.WriteString(s.paintGreen("+"+strconv.Itoa(pr.Additions)) + " " +
		s.paintRed("-"+strconv.Itoa(pr.Deletions)) +
		" in " + strconv.Itoa(pr.ChangedFiles) + " files" +
		dot + "into " + pr.BaseRefName + "\n")
	if detail == Full {
		out.WriteString(s.paintGray("from "+pr.HeadRefName) + "\n")
	}
	out.WriteString(checks + "\n")
	for _, check := range pr.StatusCheckRollup {
		if result := check.Result(); result != "pass" {
			out.WriteString("  " + s.paintResult(result) + ": " + check.Label() + "\n")
		}
	}
	var labels []string
	for _, label := range pr.Labels {
		labels = append(
			labels,
			label.Name,
		)
	}
	if len(labels) > 0 {
		out.WriteString(s.paintGray("labels: "+strings.Join(
			labels,
			", ",
		)) + "\n")
	}
	return out.String()
}

// Returns one tab-separated row per pull request for fzf. The repo and the
// number come first and are hidden, then what the list shows.
func RenderListRows(
	prs []github.PullRequest,
	s Style,
) string {
	var out strings.Builder
	for _, pr := range prs {
		number := strconv.Itoa(pr.Number)
		title := rowBreaks.Replace(pr.Title)
		shown := s.paintYellow("#"+number) + " " + title
		if pr.IsDraft {
			shown += " " + s.paintGray("[draft]")
		}
		out.WriteString(pr.Repo() + "\t" + number + "\t" + shown + "\n")
	}
	return out.String()
}

func (s Style) paintResult(result string) string {
	return s.paint(
		resultColors[result],
		result,
	)
}

func (s Style) paintState(state string) string {
	switch state {
	case "OPEN":
		return s.paintGreen(state)
	case "MERGED":
		return s.paint(
			magenta,
			state,
		)
	}
	return s.paintRed(state)
}

// Returns GitHub's merge state in plain words and color.
func (s Style) formatMerge(state string) string {
	found, ok := mergeWords[state]
	if !ok {
		found = coloredWords{
			words: "merge unknown",
			color: gray,
		}
	}
	return s.paintWords(found)
}

// Returns the check counts, gray when a count is 0.
func (s Style) formatCheckCounts(checks []github.Check) string {
	counts := map[string]int{}
	for _, check := range checks {
		counts[check.Result()]++
	}
	var parts []string
	for _, result := range []string{
		"pass",
		"fail",
		"pending",
	} {
		part := strconv.Itoa(counts[result]) + " " + result
		if counts[result] == 0 {
			part = s.paintGray(part)
		} else {
			part = s.paint(
				resultColors[result],
				part,
			)
		}
		parts = append(
			parts,
			part,
		)
	}
	return "checks: " + strings.Join(
		parts,
		", ",
	)
}

// Returns the repo, the last update and the author, dropping the repo's
// owner and then the author when the row is too narrow.
func (s Style) formatWhereRow(
	repo string,
	age string,
	who string,
) string {
	_, name, _ := strings.Cut(
		repo,
		"/",
	)
	tail := " · " + age + " · " + who
	dot := " " + s.paintGray("·") + " "
	switch {
	case utf8.RuneCountInString(repo+tail) <= s.Width:
		return s.paintCyan(repo) + dot + s.paintGray(age) + dot + who
	case utf8.RuneCountInString(name+tail) <= s.Width:
		return s.paintCyan(name) + dot + s.paintGray(age) + dot + who
	}
	return s.paintCyan(name) + dot + s.paintGray(age)
}
