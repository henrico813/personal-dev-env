package popup

import (
	"fmt"

	"pde-comment-review/internal/review"
)

// One waiting review in the list Ctrl+b e shows when several wait.
type Choice struct {
	Name         string
	Origin       review.Origin
	RequestCount int
	Answered     int
}

type group struct {
	title   string
	choices []Choice
}

// Lists the waiting reviews, grouped by the agent session that added them,
// for the owner to pick one.
type Picker struct {
	Cols, Rows int
	groups     []group
	choices    []Choice // in the order shown, for moving with j and k
	index      int
}

// Groups choices, given newest first, by session. A group's place and title
// come from its newest review; reviews with no session come last.
func NewPicker(choices []Choice) *Picker {
	p := &Picker{}
	var unknown []Choice
	groupOf := map[string]int{}
	for _, choice := range choices {
		session := choice.Origin.Session
		if session == "" {
			unknown = append(unknown, choice)
			continue
		}
		i, seen := groupOf[session]
		if !seen {
			i = len(p.groups)
			groupOf[session] = i
			p.groups = append(p.groups, group{title: nameSession(choice.Origin)})
		}
		p.groups[i].choices = append(p.groups[i].choices, choice)
	}
	if len(unknown) > 0 {
		p.groups = append(p.groups, group{title: "Unknown session", choices: unknown})
	}
	for _, g := range p.groups {
		p.choices = append(p.choices, g.choices...)
	}
	return p
}

// Herdr's title for the session, or the start of its ID when there was none.
func nameSession(o review.Origin) string {
	if o.Agent != "" {
		return o.Agent
	}
	return "Session " + clip(o.Session, 8)
}

func (p *Picker) Chosen() string { return p.choices[p.index].Name }

func (p *Picker) PressKey(key string) Action {
	switch key {
	case "q", "\x03": // q or Ctrl-c
		return Quit
	case "\r", "\n":
		return Open
	case "j":
		p.index = min(p.index+1, len(p.choices)-1)
	case "k":
		p.index = max(p.index-1, 0)
	}
	return None
}

// Returns every row of the screen; each review takes two rows.
func (p *Picker) BuildScreen() []string {
	var rows []string
	chosenRow := 0
	for _, g := range p.groups {
		rows = append(rows, bold+clip(escapeControlCharacters(g.title), p.Cols)+reset)
		for _, choice := range g.choices {
			marker := "  "
			if choice.Name == p.Chosen() {
				marker = "> "
				chosenRow = len(rows)
			}
			rows = append(rows, marker+fitLabel(choice.Origin, p.Cols-len(marker)))
			name := escapeControlCharacters(choice.Name)
			details := fmt.Sprintf("    %s · %d/%d answered", name, choice.Answered, choice.RequestCount)
			rows = append(rows, dim+clip(details, p.Cols)+reset)
		}
	}
	// Scrolls so both rows of the chosen review are in view.
	room := max(2, p.Rows-2)
	first := max(0, chosenRow+2-room)
	rows = rows[first:min(len(rows), first+room)]
	lines := append([]string{bold + clip("Comment reviews waiting", p.Cols) + reset}, rows...)
	for len(lines) < p.Rows-1 {
		lines = append(lines, "")
	}
	return append(lines, dim+clip("j/k move Enter open q quit", p.Cols)+reset)
}
