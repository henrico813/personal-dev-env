package approval

import (
	"strings"
	"testing"
)

// For pr edit the popup shows changed lines as "-old" and "+new". In a live
// edit popup, Markdown bullets ("- item") in the new body were shown too and
// looked like removed lines.
func TestEditChangesStopsBeforeBody(t *testing.T) {
	text := EditHeader + "Title:\n-old\n+new\n\nbody:\n- bullet\n"

	got := strings.Join(editChanges(text), "\n")

	if got != "-old\n+new" {
		t.Fatalf("changes %q", got)
	}
}
