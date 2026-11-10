package chezmoi

import (
	"os/exec"
	"strings"
	"testing"
)

// A full install must link card to a script that runs, or hubs and workers
// on a new machine cannot read or move cards.
func TestCardLinkRunsCheckoutCardScript(t *testing.T) {
	text := renderProfileTemplate(t, "dot_local/bin/symlink_card.tmpl", "full")
	target := strings.TrimSpace(text)

	output, err := exec.Command(target, "--help").CombinedOutput()

	if err != nil {
		t.Fatalf("%s --help failed: %v\n%s", target, err, output)
	}
}
