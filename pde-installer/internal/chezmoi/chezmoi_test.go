package chezmoi

import (
	"fmt"
	"strings"
	"testing"
)

// Shared workflow skills ship once under ai/skills and deploy to
// ~/.agents/skills; a re-added Codex mapping would install a second copy.
func TestExternalTemplates(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full")
	for _, name := range []string{"cleanup-plan", "create-plan", "design-doc", "implement-plan", "research-codebase", "review-plan"} {
		mapping := fmt.Sprintf("[%q]\ntype = \"file\"\nurl = 'file://%s/ai/skills/%s/SKILL.md'",
			".agents/skills/"+name+"/SKILL.md", repoRoot(t), name)
		if strings.Count(text, mapping) != 1 {
			t.Fatalf("mapping %s does not point once to ai/skills/%s/SKILL.md", name, name)
		}
		stale := fmt.Sprintf(`[".codex/skills/%s/SKILL.md"]`, name)
		if strings.Contains(text, stale) {
			t.Fatalf("stale Codex workflow skill mapping remains: %s", stale)
		}
	}
}
