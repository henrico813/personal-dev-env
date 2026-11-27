package card

import (
	"slices"
	"time"
)

// A Herdr agent's name and its sessions entry, such as "opencode ses_abc".
type AgentSession struct{ Name, Entry string }

// Appends value to a list field, such as plans or prs. A value already in
// the list changes nothing.
func AddToList(text, field, value string) (string, error) {
	values := ReadFields(text)[field].AsList()
	if slices.Contains(values, value) {
		return text, nil
	}
	return SetField(text, field, FieldValue{List: append(slices.Clip(values), value), IsList: true})
}

// Appends an agent's entry to sessions and logs it, unless the card
// already has it.
func AddSession(text string, today time.Time, session AgentSession) (string, error) {
	if slices.Contains(ReadFields(text)["sessions"].AsList(), session.Entry) {
		return text, nil
	}
	text, err := AddToList(text, "sessions", session.Entry)
	if err != nil {
		return "", err
	}
	line := "- " + today.Format(time.DateOnly) + " session: " + session.Name + " (" + session.Entry + ")"
	return AddToSection(text, "Log", line), nil
}

// Finds the first card whose sessions field holds entry.
func FindSessionHolder(cards []Card, entry string) (Card, bool) {
	for _, c := range cards {
		if slices.Contains(ReadFields(c.Text)["sessions"].AsList(), entry) {
			return c, true
		}
	}
	return Card{}, false
}
