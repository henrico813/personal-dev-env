package card

import (
	"regexp"
	"slices"
	"strconv"
	"strings"
	"time"
)

var (
	// A GitHub pull request URL, maybe followed by a page such as /files.
	prURLPattern = regexp.MustCompile(`(?i)^(?:https?://)?(?:www\.)?github\.com/([^/` + spaceClass + `]+)/([^/` + spaceClass + `]+)/pull/(\d+)`)
	// An Acceptance Criteria checkbox that is not ticked.
	untickedPattern = regexp.MustCompile(`(?m)^[` + spaceClass + `]*(?:[-*+]|\d+[.)]) \[[^xX\]]\] [^` + spaceClass + `]`)
)

// Moves a card that links prURL to done when all its PRs merged and its
// criteria are ticked, else logs once per merge why it stays. It returns
// card merged's line, such as `DEMO-005 active -> in-review -> done`, and
// asks readState only about the card's other PRs.
func ApplyMerge(projects Projects, c Card, prURL string, readState func(url string) (string, error), today time.Time) (string, error) {
	var unmerged []string
	for _, url := range ListCardPRs(c.Text) {
		if url == prURL {
			continue
		}
		state, err := readState(url)
		if err != nil {
			return "", err
		}
		if state != "MERGED" {
			unmerged = append(unmerged, url+" is "+strings.ToLower(state))
		}
	}
	day := today.Format(time.DateOnly)
	var line string
	_, err := Update(c, func(text string) (string, error) {
		current := ReadFields(text)["stage"].Text
		waits := slices.Clone(unmerged)
		criteria, _ := ReadSection(text, Criteria)
		if unticked := len(untickedPattern.FindAllString(criteria, -1)); unticked > 0 {
			waits = append(waits, strconv.Itoa(unticked)+" unticked in "+Criteria)
		}
		var chain []string
		if len(waits) == 0 {
			var err error
			if chain, err = PlanMoves(projects, c.CardKey, text, Done, true); err != nil {
				waits = append(waits, err.Error())
			}
		}
		if len(waits) > 0 {
			line = c.CardKey + " stays " + current + ": " + strings.Join(waits, ", ")
			note := "note (card merged): " + prURL + " merged, but the card stays " + current + ": " + strings.Join(waits, ", ")
			// A rerun for the same merge must not add the same note again.
			if log, _ := ReadSection(text, "Log"); strings.HasSuffix(strings.TrimRight(log, " \t\n\r\v\f"), note) {
				return text, nil
			}
			return AddToSection(text, "Log", "- "+day+" "+note), nil
		}
		for _, stage := range chain {
			why := ""
			if stage == Done {
				why = prURL + " merged"
			}
			var err error
			if text, err = ApplyMove(text, stage, today, FormatMoveLogText(stage, "by card merged", true, why)); err != nil {
				return "", err
			}
		}
		line = c.CardKey + " " + strings.Join(append([]string{current}, chain...), " -> ")
		return text, nil
	})
	return line, err
}

// Normalizes a PR URL, so two spellings of it match.
// "http://GitHub.com/Me/Repo/pull/7/files#top" becomes
// "https://github.com/me/repo/pull/7". Any other URL only loses its "#..."
// part and a trailing slash.
func NormalizePRURL(url string) string {
	url = strings.TrimSpace(url)
	if match := prURLPattern.FindStringSubmatch(url); match != nil {
		return strings.ToLower("https://github.com/" + match[1] + "/" + match[2] + "/pull/" + match[3])
	}
	url, _, _ = strings.Cut(url, "#")
	return strings.TrimRight(url, "/")
}

// Lists the PR URLs in a card's prs field, normalized, each once, in field
// order.
func ListCardPRs(text string) []string {
	var urls []string
	for _, url := range ReadFields(text)["prs"].AsList() {
		if url = NormalizePRURL(url); !slices.Contains(urls, url) {
			urls = append(urls, url)
		}
	}
	return urls
}
