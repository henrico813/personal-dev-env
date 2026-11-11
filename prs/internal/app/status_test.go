package app

import (
	"testing"
)

// Agents paste this list into status updates, and the owner reads it on a
// phone to pick what to review. A stacked pull request must come right
// after the one it builds on, and the bold one must be one the owner can
// review now.
func TestStatusListsStacksInMergeOrder(t *testing.T) {
	want := "Open pull requests in example/repo, in merge order:\n\n" +
		"1. example/repo #20: Logins time out on slow networks. No checks. Blocked by: draft.\n" +
		"2. example/repo #22: Retries leave no trace. 1 of 2 checks fail: lint. Blocked by: waits for #20, failing checks.\n" +
		"3. **example/repo #21: Fix typo. Checks pass. Blocked by: nothing.**\n"

	output := runPRs(
		t,
		"works",
		"status",
	)

	if output != want {
		t.Errorf(
			"prs status printed\n%s\nwant\n%s",
			output,
			want,
		)
	}
}
