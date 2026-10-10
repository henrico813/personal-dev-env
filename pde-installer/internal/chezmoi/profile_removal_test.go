package chezmoi

import (
	"strings"
	"testing"
)

// chezmoi skips a removal target that .chezmoiignore matches, so every target
// must stay out of the ignore list. Otherwise the retired web service, or the
// old approval scripts on terminal machines, would never be cleaned up.
func TestIgnoreTemplateKeepsRemoveTargets(t *testing.T) {
	for _, selected := range []string{"full", "terminal"} {
		t.Run(selected, func(t *testing.T) {
			ignored := renderProfileTemplate(t, ".chezmoiignore.tmpl", selected)
			removed := renderProfileTemplate(t, removeFileTemplate, selected)
			for _, line := range strings.Split(removed, "\n") {
				target := strings.TrimSpace(line)
				if target != "" && !strings.HasPrefix(target, "#") && containsLine(ignored, target) {
					t.Errorf("%s ignores removal target %q", selected, target)
				}
			}
		})
	}
}

// chezmoi deletes a target whose template renders empty. Terminal machines
// have no pde-gh-write, so a guard left there would block every PR write.
func TestGuardRendersEmptyForTerminal(t *testing.T) {
	if text := renderProfileTemplate(t, "dot_local/bin/executable_gh.tmpl", "terminal"); strings.TrimSpace(text) != "" {
		t.Errorf("terminal guard is not empty:\n%s", text)
	}
	if text := renderProfileTemplate(t, "dot_local/bin/executable_gh.tmpl", "full"); !strings.HasPrefix(text, "#!/usr/bin/env bash\n") {
		t.Errorf("full guard does not start with its shebang:\n%s", text)
	}
}
