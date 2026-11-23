package chezmoi

import "testing"

// A machine that moves from full to client keeps full's approval tools and
// gh guard links until chezmoi removes them, and client has no pde-gh
// for them to run. A gh link left there would break every gh call.
func TestClientRemovesFullApprovalTools(t *testing.T) {
	text := renderProfileTemplate(t, removeFileTemplate, "client")

	for _, target := range []string{
		".local/bin/gh",
		".local/bin/pde-gh",
		".local/bin/pde-gh-guard",
		".local/bin/pde-gh-write",
		".local/bin/pde-pr-approve",
	} {
		if !containsLine(text, target) {
			t.Errorf("%s omits removal target %q", removeFileTemplate, target)
		}
	}
}
