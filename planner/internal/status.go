package internal

import "strings"

// PlanStatus is a normalized status for a vault issue.
type PlanStatus string

const (
	PlanStatusOpen       PlanStatus = "open"
	PlanStatusInProgress PlanStatus = "in-progress"
	PlanStatusDone       PlanStatus = "done"
	PlanStatusWontDo     PlanStatus = "wont-do"
	PlanStatusUnknown    PlanStatus = "unknown"
)

func standardPlanStatuses() [4]PlanStatus {
	return [...]PlanStatus{
		PlanStatusOpen,
		PlanStatusInProgress,
		PlanStatusDone,
		PlanStatusWontDo,
	}
}

// normalizePlanStatus maps known legacy values without changing their source value.
func normalizePlanStatus(raw any) PlanStatus {
	value, ok := raw.(string)
	if !ok {
		return PlanStatusUnknown
	}
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "open", "planning":
		return PlanStatusOpen
	case "in-progress":
		return PlanStatusInProgress
	case "done", "completed", "closed":
		return PlanStatusDone
	case "wont-do", "won't do", "wontdo", "obsolete", "superseded", "failed":
		return PlanStatusWontDo
	default:
		return PlanStatusUnknown
	}
}

func isStandardPlanStatus(status PlanStatus) bool {
	for _, standard := range standardPlanStatuses() {
		if status == standard {
			return true
		}
	}
	return false
}

func planStatusOrder(includeUnknown bool) []PlanStatus {
	standard := standardPlanStatuses()
	statuses := append([]PlanStatus(nil), standard[:]...)
	if includeUnknown {
		statuses = append(statuses, PlanStatusUnknown)
	}
	return statuses
}
