package card

import (
	"errors"
	"fmt"
	"regexp"
	"slices"
	"strings"
	"time"
)

var checkboxPattern = regexp.MustCompile(`(?m)^[` + spaceClass + `]*- \[[ xX]\] [^` + spaceClass + `]`)

// Returns the stages a move from the card's stage passes through, target
// last. Each step is checked alone for who may move, written criteria,
// the title rules and the card limits.
func PlanMoves(projects Projects, cardKey, text, target string, byUser bool) ([]string, error) {
	current := ReadFields(text)["stage"].Text
	chain := findMoveChain(current, target, byUser)
	if chain == nil {
		return nil, errors.New(formatRefusedMove(cardKey, current, target, byUser))
	}
	source := current
	for _, stage := range chain {
		if err := checkMove(projects, cardKey, text, source, stage, byUser); err != nil {
			return nil, err
		}
		source = stage
	}
	return chain, nil
}

// Formats a move's Log text, such as `active: by worker-1`. The
// author holds its verb, such as "by" or "adopted by".
func FormatMoveLogText(stage, author string, byUser bool, why string) string {
	text := stage + ": " + author
	// The Log is the only record that the user, not the agent, decided.
	if byUser {
		text += " for the user"
	}
	if why != "" {
		text += ". " + why
	}
	return text
}

// Sets stage and stage_date and adds the move's Log line. It fails only
// for a card with no frontmatter.
func ApplyMove(text, stage string, today time.Time, logText string) (string, error) {
	day := today.Format(time.DateOnly)
	text = AddToSection(text, "Log", "- "+day+" "+logText)
	text, err := SetField(text, "stage", FieldValue{Text: stage})
	if err != nil {
		return "", err
	}
	return SetField(text, "stage_date", FieldValue{Text: day})
}

// Refuses a move to blocked without a --why naming who the card waits on.
func RequireBlockedReason(stage, why string) error {
	if stage == Blocked && strings.TrimSpace(why) == "" {
		return errors.New(Blocked + " needs --why naming who the card waits on")
	}
	return nil
}

// Refuses a title that breaks a title rule, listing every finding.
func RequirePassingTitle(rules NameRules, title string) error {
	findings := CheckTitle(rules, title)
	if len(findings) == 0 {
		return nil
	}
	lines := []string{"title refused:"}
	for _, finding := range findings {
		lines = append(lines, "  "+finding.String())
	}
	lines = append(lines, fmt.Sprintf(`Write it as a commit title of at most %d characters, such as "Retry failed uploads".`, TitleLimit))
	return errors.New(strings.Join(lines, "\n"))
}

// Returns the shortest chain of allowed moves, passing only through
// ChainStages, or nil. Without byUser only staff moves count.
func findMoveChain(current, target string, byUser bool) []string {
	if current == target {
		return nil
	}
	cameFrom := map[string]string{current: current}
	queue := []string{current}
	for len(queue) > 0 {
		if _, found := cameFrom[target]; found {
			break
		}
		stage := queue[0]
		queue = queue[1:]
		if stage != current && !slices.Contains(ChainStages, stage) {
			continue
		}
		for _, move := range Moves {
			if _, seen := cameFrom[move.To]; move.From == stage && !seen && (byUser || move.Who == "staff") {
				cameFrom[move.To] = stage
				queue = append(queue, move.To)
			}
		}
	}
	if _, found := cameFrom[target]; !found {
		return nil
	}
	chain := []string{target}
	for cameFrom[chain[len(chain)-1]] != current {
		chain = append(chain, cameFrom[chain[len(chain)-1]])
	}
	slices.Reverse(chain)
	return chain
}

// Says why there is no chain, either no allowed move at all or a move
// only the user makes, with the stage the caller can still reach.
func formatRefusedMove(cardKey, current, target string, byUser bool) string {
	var chain []string
	if !byUser {
		chain = findMoveChain(current, target, true)
	}
	if chain == nil {
		options := []string{}
		for _, move := range Moves {
			if move.From == current {
				options = append(options, move.To)
			}
		}
		allowed := strings.Join(options, ", ")
		if allowed == "" {
			allowed = "none, the stage is unknown"
		}
		return fmt.Sprintf("no move %s -> %s. Allowed: %s", current, target, allowed)
	}
	source, stage := current, ""
	for _, stage = range chain {
		if findMoveWho(source, stage) == "user" {
			break
		}
		source = stage
	}
	reason := fmt.Sprintf("only the user moves %s -> %s", source, stage)
	if source != current {
		return fmt.Sprintf("%s. You can move to %s: card move %s %s", reason, source, cardKey, source)
	}
	if stage != target {
		reason += ", on the way to " + target
	}
	return reason + ". Pass --user only when the user said so."
}

// Returns who may make the move, "staff" or "user", or "" for no move.
func findMoveWho(from, to string) string {
	for _, move := range Moves {
		if move.From == from && move.To == to {
			return move.Who
		}
	}
	return ""
}

// Refuses one move for unwritten criteria, a failing title or a full stage.
func checkMove(projects Projects, cardKey, text, current, target string, byUser bool) error {
	criteria, _ := ReadSection(text, Criteria)
	if current == Draft && target == Todo && !checkboxPattern.MatchString(criteria) {
		return fmt.Errorf("add a '- [ ] ...' item to %s before moving to %s", Criteria, Todo)
	}
	// Workers start todo cards, so every move into todo is checked, a
	// staff pause included.
	if target == Todo {
		if err := RequirePassingTitle(projects.Rules, readHeadingTitle(text, cardKey)); err != nil {
			return err
		}
	}
	limits := map[string]int{Active: ActiveLimit, InReview: ReviewLimit}
	limit, hasLimit := limits[target]
	// The user's moves may fill a stage past its limit.
	if !hasLimit || byUser {
		return nil
	}
	cards, err := LoadAll(projects)
	if err != nil {
		return err
	}
	count := 0
	for _, c := range cards {
		if c.ReadStage() == target {
			count++
		}
	}
	if count >= limit {
		return fmt.Errorf("%d cards are in %s, the limit is %d. Ask the user which to finish first.", count, target, limit) //nolint:staticcheck // the Python card's words
	}
	return nil
}

// Reads the title from the first "# " heading without the card key, or ""
// with no heading. Unlike Card.ReadTitle, it never falls back to the file
// name, so a card with no heading fails the title rules.
func readHeadingTitle(text, cardKey string) string {
	lines := strings.Split(text, "\n")
	for _, line := range lines[findFrontmatterEnd(lines)+1:] {
		if heading, ok := strings.CutPrefix(line, "# "); ok {
			return trimCardKey(heading, cardKey)
		}
	}
	return ""
}
