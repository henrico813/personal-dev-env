package cmd

import (
	"path/filepath"
	"strings"
	"testing"

	"pde-comment-review/internal/review"
)

// A small model must finish a review with status, draft, set and submit,
// and the owner must never answer an item twice.
func TestAgentAnswersNoteInSecondRound(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	checker := findRepoChecker(t)

	// The agent drafts, rewords the new comment and sends the round.
	replyToDraft(repo, checker)
	replyToSet(
		repo,
		1,
		review.ProposeReword,
		"Removes every price from the cart.",
	)
	sent := replyToSubmit(repo, checker)
	resent := replyToSubmit(repo, checker)
	early := replyToSet(
		repo,
		1,
		review.ProposeKeep,
		"",
	)
	// The owner leaves a note, and the agent answers it.
	r, err := review.OpenReview("feat")
	if err != nil {
		t.Fatalf(
			"OpenReview() error = %v after replyToSubmit() problems %q",
			err,
			sent.Problems,
		)
	}
	if err := r.SetNote(1, "say the cart stays"); err != nil {
		t.Fatal(err)
	}
	noted := replyToStatus(repo)
	replyToSet(
		repo,
		1,
		review.ProposeReword,
		"Removes every price, and keeps the cart.",
	)
	replyToSubmit(repo, checker)

	if !strings.Contains(strings.Join(resent.Problems, "\n"), "no draft waits to be sent") || !strings.Contains(resent.Next, "Tell the user") {
		t.Errorf("second replyToSubmit() = %+v, want a refusal that still says to tell the user", resent)
	}
	if !strings.Contains(strings.Join(early.Problems, "\n"), "item 1 is not in the draft") {
		t.Errorf("replyToSet() before the note problems = %q, want a refusal", early.Problems)
	}
	if noted.State != "notes_waiting" || len(noted.Notes) != 1 || noted.Notes[0].ID != 1 {
		t.Errorf("replyToStatus() with a note = %+v, want notes_waiting with item 1's note", noted)
	}
	r, err = review.OpenReview("feat")
	if err != nil {
		t.Fatal(err)
	}
	// Round 2 holds only the noted item, in its new wording.
	if len(r.Rounds) != 2 || len(r.Rounds[1].Items) != 1 || r.Rounds[1].Items[0].New != "// Removes every price, and keeps the cart.\n" {
		t.Errorf("rounds = %+v, want round 2 to hold only item 1's new wording", r.Rounds)
	}
}
