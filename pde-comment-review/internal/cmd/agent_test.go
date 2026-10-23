package cmd

import (
	"path/filepath"
	"strings"
	"testing"

	"pde-comment-review/internal/review"
)

// Verifies that an agent can answer the owner's note without writing
// JSON, which a small model gets wrong.
func TestAgentAnswersNoteInSecondRound(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	checker := findRepoChecker(t)
	// The agent drafts, rewords the new comment and sends the round.
	if drafted := replyToDraft(repo, checker); len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}
	replyToSet(
		repo,
		1,
		review.ProposeReword,
		"Removes every price from the cart.",
	)
	sent := replyToSubmit(repo, checker)
	// The owner leaves a note.
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

// Verifies that a second submit sends nothing but still says to tell the
// user the round is ready.
func TestSecondSubmitRefusedWithNextStep(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	checker := findRepoChecker(t)
	if drafted := replyToDraft(repo, checker); len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}
	if sent := replyToSubmit(repo, checker); len(sent.Problems) > 0 {
		t.Fatalf("replyToSubmit() problems = %q", sent.Problems)
	}

	resent := replyToSubmit(repo, checker)

	if !strings.Contains(strings.Join(resent.Problems, "\n"), "no draft waits to be sent") || !strings.Contains(resent.Next, "Tell the user") {
		t.Errorf("second replyToSubmit() = %+v, want a refusal that still says to tell the user", resent)
	}
}

// Verifies that set refuses an item with no note while a round waits, so
// the owner answers it once.
func TestSetRefusesItemWithoutOwnerNote(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	checker := findRepoChecker(t)
	if drafted := replyToDraft(repo, checker); len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}
	if sent := replyToSubmit(repo, checker); len(sent.Problems) > 0 {
		t.Fatalf("replyToSubmit() problems = %q", sent.Problems)
	}

	reply := replyToSet(
		repo,
		1,
		review.ProposeKeep,
		"",
	)

	if !strings.Contains(strings.Join(reply.Problems, "\n"), "item 1 is not in the draft") {
		t.Errorf("replyToSet() before a note problems = %q, want a refusal", reply.Problems)
	}
}

// Verifies that status, set and submit refuse another clone, since its
// agent would change a review of code it does not have.
func TestStatusSetSubmitRefuseOtherCheckout(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	runScript(
		t,
		filepath.Dir(repo),
		"git clone -q origin other && git -C other switch -q -c feat",
	)
	other := filepath.Join(filepath.Dir(repo), "other")
	checker := findRepoChecker(t)
	if drafted := replyToDraft(repo, checker); len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}

	// The other clone tries the first clone's draft, then its waiting round.
	refused := []agentReply{
		replyToStatus(other),
		replyToSet(
			other,
			1,
			review.ProposeCut,
			"",
		),
		replyToSubmit(other, checker),
	}
	replyToSubmit(repo, checker)
	refused = append(refused, replyToStatus(other))

	for _, reply := range refused {
		if len(reply.Problems) == 0 || !strings.Contains(reply.Problems[0], "tell the owner if that is another checkout") {
			t.Errorf("reply in the other clone = %+v, want a refusal", reply)
		}
	}
	r, err := review.OpenReview("feat")
	if err != nil {
		t.Fatal(err)
	}
	if len(r.Rounds) != 1 || len(r.Rounds[0].Items) != 1 || r.Rounds[0].Items[0].Proposal != review.ProposeKeep {
		t.Errorf("rounds = %+v, want one round keeping item 1, as the first clone drafted it", r.Rounds)
	}
}

// Verifies that set refuses keep for a comment whose code was removed,
// since submit would then fail.
func TestSetRefusesKeepForRemovedCode(t *testing.T) {
	repo := newShopClone(t, "feat")
	// Removing a function makes item 1 a cut of its approved comment.
	writeFile(
		t,
		filepath.Join(repo, "shop.go"),
		strings.Replace(
			shopGo,
			"// Reports whether every price is above zero.\nfunc Valid(prices []int) bool { return true }\n\n",
			"",
			1,
		),
	)
	if drafted := replyToDraft(repo, findRepoChecker(t)); len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}

	reply := replyToSet(
		repo,
		1,
		review.ProposeKeep,
		"",
	)

	if !strings.Contains(strings.Join(reply.Problems, "\n"), "item 1's code was removed, so only cut fits") {
		t.Errorf("replyToSet() problems = %q, want keep refused", reply.Problems)
	}
}

// Verifies that set refuses reword text starting with //, since the new
// comment would otherwise start with // twice.
func TestSetRefusesTextWrittenAsComment(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	if drafted := replyToDraft(repo, findRepoChecker(t)); len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}

	reply := replyToSet(
		repo,
		1,
		review.ProposeReword,
		"// Removes every price.",
	)

	if !strings.Contains(strings.Join(reply.Problems, "\n"), "without //") {
		t.Errorf("replyToSet() with // text problems = %q, want a refusal", reply.Problems)
	}
}

// Verifies that draft's reply on the command line names shell commands,
// since tool names such as review_set do not run in a shell.
func TestCommandLineDraftNamesShellCommands(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)
	drafted := replyToDraft(repo, findRepoChecker(t))
	if len(drafted.Problems) > 0 {
		t.Fatalf("replyToDraft() problems = %q", drafted.Problems)
	}

	printed := nameShellCommands(encodeReply(drafted))

	if strings.Contains(printed, "review_") || !strings.Contains(printed, "pde-comment-review set") {
		t.Errorf("draft reply for the command line = %s, want commands, not tools", printed)
	}
}

// Verifies that a draft with nothing to answer reports done, so the agent
// tells the user there is nothing to review.
func TestDraftReportsNothingToReview(t *testing.T) {
	repo := newShopClone(t, "feat")

	reply := replyToDraft(repo, findRepoChecker(t))

	if len(reply.Problems) > 0 || reply.State != "done" {
		t.Errorf("replyToDraft() = %+v, want state done", reply)
	}
}
