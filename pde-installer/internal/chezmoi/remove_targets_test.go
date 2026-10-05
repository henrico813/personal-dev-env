package chezmoi

import (
	"path"
	"strings"
	"testing"
)

// The full profile must clean up the workflow paths it no longer manages:
// the old Codex skill copies and the old Claude command files.
func TestRemoveTargetsListOldWorkflowPaths(t *testing.T) {
	removeText := renderProfileTemplate(t, removeFileTemplate, "full")
	var removeTargets []string
	for _, line := range strings.Split(removeText, "\n") {
		if target := strings.TrimSpace(line); target != "" && !strings.HasPrefix(target, "#") {
			removeTargets = append(removeTargets, path.Clean(target))
		}
	}
	expected := []string{
		".codex/skills/create-plan",
		".codex/skills/review-plan",
		".codex/skills/implement-plan",
		".codex/skills/cleanup-plan",
		".codex/skills/design-doc",
		".codex/skills/research-codebase",
		".claude/commands/create_plan.md",
		".claude/commands/review_plan.md",
		".claude/commands/implement_plan.md",
		".claude/commands/cleanup_plan.md",
		".claude/commands/design_doc.md",
		".claude/commands/research_codebase.md",
	}
	present := make(map[string]bool, len(removeTargets))
	for _, target := range removeTargets {
		present[target] = true
	}
	for _, target := range expected {
		if !present[target] {
			t.Fatalf("%s omits removal target %q", removeFileTemplate, target)
		}
	}
}
