package internal

import (
	"slices"
	"sort"
	"strings"
)

type planStatus string

const (
	statusOpen       planStatus = "open"
	statusInProgress planStatus = "in-progress"
	statusDone       planStatus = "done"
	statusWontDo     planStatus = "wont-do"
	statusUnknown    planStatus = "unknown"
)

var standardPlanStatuses = []planStatus{
	statusOpen,
	statusInProgress,
	statusDone,
	statusWontDo,
}

// Older vault notes use these spellings for the corresponding statuses.
var legacyPlanStatusAliases = map[string]planStatus{
	"planning":   statusOpen,
	"completed":  statusDone,
	"closed":     statusDone,
	"won't do":   statusWontDo,
	"wontdo":     statusWontDo,
	"obsolete":   statusWontDo,
	"superseded": statusWontDo,
	"failed":     statusWontDo,
}

// Unmapped values stay unknown rather than being inferred.
func normalizePlanStatus(raw any) planStatus {
	value, ok := raw.(string)
	if !ok {
		return statusUnknown
	}
	status, ok := parsePlanStatus(value)
	if !ok {
		return statusUnknown
	}
	return status
}

func parsePlanStatus(value string) (planStatus, bool) {
	normalized := strings.ToLower(strings.TrimSpace(value))
	if slices.Contains(standardPlanStatuses, planStatus(normalized)) {
		return planStatus(normalized), true
	}
	if normalized == string(statusUnknown) {
		return statusUnknown, true
	}
	status, ok := legacyPlanStatusAliases[normalized]
	return status, ok
}

func acceptedPlanStatuses() string {
	values := make([]string, 0, len(standardPlanStatuses)+len(legacyPlanStatusAliases)+1)
	for _, status := range standardPlanStatuses {
		values = append(values, string(status))
	}
	values = append(values, string(statusUnknown))
	aliases := make([]string, 0, len(legacyPlanStatusAliases))
	for alias := range legacyPlanStatusAliases {
		aliases = append(aliases, alias)
	}
	sort.Strings(aliases)
	return strings.Join(append(values, aliases...), ", ")
}
