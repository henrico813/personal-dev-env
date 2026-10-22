package cmd

import (
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-comment-review/internal/review"
)

const validApproval = `{"approved": [{"file": "cart.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"}], "avoid": {}}`

// Saves items as a proposals file for a scratch repository holding code
// as cart.go and approvals as .code-comments.json, then runs add on it.
// Returns the path round 1 is saved at, and add's error.
func addScratchReview(
	t *testing.T,
	code string,
	approvals string,
	items ...review.Item,
) (string, error) {
	t.Helper()
	t.Setenv(
		"XDG_STATE_HOME",
		t.TempDir(),
	)
	repo := t.TempDir()
	writeFile(
		t,
		filepath.Join(
			repo,
			"cart.go",
		),
		code,
	)
	writeFile(
		t,
		filepath.Join(
			repo,
			".code-comments.json",
		),
		approvals,
	)
	roundFile := filepath.Join(
		review.FindStateRoot(),
		"cart",
		"round-1.json",
	)
	return roundFile, addRound(
		"cart",
		writeProposals(
			t,
			repo,
			items...,
		),
		findRepoChecker(t),
	)
}

// Writes items as a proposals file for repo and returns its path.
func writeProposals(
	t *testing.T,
	repo string,
	items ...review.Item,
) string {
	t.Helper()
	data, err := json.Marshal(review.Proposals{
		Version: review.Version,
		Repo:    repo,
		Items:   items,
	})
	if err != nil {
		t.Fatal(err)
	}
	proposals := filepath.Join(
		t.TempDir(),
		"proposals.json",
	)
	writeFile(
		t,
		proposals,
		string(data),
	)
	return proposals
}

// Saves round 1 of a scratch review of cartGo, then writes files, by name,
// into its repository, ready for round 2. Returns the repository.
func startSecondRound(
	t *testing.T,
	files map[string]string,
) string {
	t.Helper()
	_, err := addScratchReview(
		t,
		cartGo,
		validApproval,
		cutCurrency,
		rewordTotal,
		keepTooHigh,
	)
	if err != nil {
		t.Fatal(err)
	}
	r, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	for name, text := range files {
		writeFile(
			t,
			filepath.Join(
				r.Repo(),
				name,
			),
			text,
		)
	}
	return r.Repo()
}

// Verifies that saving refuses a review the final write would refuse,
// since one review failed after the owner answered all 22 items.
func TestAddRefusesReviewThatFailsLater(t *testing.T) {
	tests := []struct {
		name   string
		onDisk string
		extra  []review.Item // items after the three every row has
		want   []string
	}{
		{
			name: "approved comment rewritten before add",
			onDisk: strings.Replace(
				cartGo,
				"every price is above zero",
				"no price is negative",
				1,
			),
			extra: []review.Item{{
				ID:       4,
				File:     "cart.go",
				Proposal: review.ProposeKeep,
				Old:      "// Reports whether no price is negative.\n",
			}},
			want: []string{
				"cart.go Valid: differs from the text the owner approved",
				"\n    // Reports whether every price is above zero.",
			},
		},
		{
			name:   "two items for one comment",
			onDisk: cartGo,
			extra: []review.Item{{
				ID:       4,
				File:     "cart.go",
				Proposal: review.ProposeKeep,
				Old:      keepTooHigh.Old,
			}},
			want: []string{
				"items 3 and 4 are the same comment in cart.go",
			},
		},
		{
			name:   "drafted comment worded like another",
			onDisk: cartGo + "\n// Reports whether every price is above zero.\nfunc Positive() bool { return true }\n",
			extra: []review.Item{{
				ID:       4,
				File:     "cart.go",
				Name:     "Positive",
				Proposal: review.ProposeKeep,
				Old:      "// Reports whether every price is above zero.\n",
			}},
			want: []string{
				"item 4: cart.go: comment found 2 times",
			},
		},
		{
			name:   "removed code back",
			onDisk: cartGo,
			extra: []review.Item{{
				ID:          4,
				File:        "cart.go",
				Name:        "Valid",
				Proposal:    review.ProposeCut,
				Old:         "// Reports whether every price is above zero.\n",
				CodeRemoved: true,
			}},
			want: []string{
				"item 4: cart.go Valid: its removed code is back",
			},
		},
	}
	for _, tt := range tests {
		t.Run(
			tt.name,
			func(t *testing.T) {
				items := append(
					[]review.Item{
						cutCurrency,
						rewordTotal,
						keepTooHigh,
					},
					tt.extra...,
				)

				roundFile, err := addScratchReview(
					t,
					tt.onDisk,
					validApproval,
					items...,
				)

				if err == nil {
					t.Fatal("addRound() saved a review that apply would refuse")
				}
				wants := append(
					tt.want,
					"nothing was saved",
				)
				for _, want := range wants {
					if !strings.Contains(
						err.Error(),
						want,
					) {
						t.Errorf(
							"addRound() error = %v, want it to contain %q",
							err,
							want,
						)
					}
				}
				_, err = os.Stat(roundFile)
				if !errors.Is(
					err,
					fs.ErrNotExist,
				) {
					t.Errorf(
						"round file %s was left behind: %v",
						roundFile,
						err,
					)
				}
			},
		)
	}
}

// Verifies that an item may quote the approved wording, which is how
// agents propose rewording an approved comment.
func TestAddSavesItemQuotingApprovedText(t *testing.T) {
	rewordValid := review.Item{
		ID:       4,
		File:     "cart.go",
		Proposal: review.ProposeReword,
		Old:      "// Reports whether every price is above zero.\n",
		New:      "// Reports whether no price is negative.\n",
	}

	roundFile, err := addScratchReview(
		t,
		cartGo,
		validApproval,
		cutCurrency,
		rewordTotal,
		keepTooHigh,
		rewordValid,
	)

	if err != nil {
		t.Fatalf(
			"addRound() error = %v",
			err,
		)
	}
	if _, err := os.Stat(roundFile); err != nil {
		t.Errorf(
			"round 1 was not saved: %v",
			err,
		)
	}
}

// Verifies that two approved comments with the same words are told apart
// by name, so an agent can propose rewording both.
func TestAddKeepsSameWordedCommentsApart(t *testing.T) {
	rewordValid := review.Item{
		ID:       1,
		File:     "cart.go",
		Proposal: review.ProposeReword,
		Old:      "// Reports whether every price is above zero.\n",
		New:      "// Reports whether no price is negative.\n",
	}
	rewordPositive := review.Item{
		ID:       2,
		File:     "cart.go",
		Name:     "Positive",
		Proposal: review.ProposeReword,
		Old:      "// Reports whether every price is above zero.\n",
		New:      "// Reports whether no price is below zero.\n",
	}

	roundFile, err := addScratchReview(
		t,
		cartGo+"\n// Reports whether no price is below zero.\nfunc Positive() bool { return true }\n",
		`{"approved": [{"file": "cart.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"},
			{"file": "cart.go", "name": "Positive", "text": "// Reports whether every price is above zero.\n"}], "avoid": {}}`,
		rewordValid,
		rewordPositive,
	)

	if err != nil {
		t.Fatalf(
			"addRound() error = %v",
			err,
		)
	}
	if _, err := os.Stat(roundFile); err != nil {
		t.Errorf(
			"round 1 was not saved: %v",
			err,
		)
	}
}

// Verifies that saving a later round rechecks earlier rounds' files, so a
// problem there is caught before the owner answers.
func TestAddRechecksFilesFromEarlierRounds(t *testing.T) {
	repo := startSecondRound(
		t,
		map[string]string{
			"cart.go": strings.Replace(
				cartGo,
				"// Reports whether every price is above zero.\n",
				"",
				1,
			),
			"tax.go": "package cart\n\n// Returns the tax on a price.\nfunc Tax(price int) int { return 0 }\n",
		},
	)
	keepTax := review.Item{
		ID:       4,
		File:     "tax.go",
		Proposal: review.ProposeKeep,
		Old:      "// Returns the tax on a price.\n",
	}

	err := addRound(
		"cart",
		writeProposals(
			t,
			repo,
			keepTax,
		),
		findRepoChecker(t),
	)

	if err == nil || !strings.Contains(
		err.Error(),
		"cart.go Valid: the owner's approved comment is gone",
	) {
		t.Fatalf(
			"addRound() error = %v, want it to name cart.go Valid",
			err,
		)
	}
	roundTwo := filepath.Join(
		review.FindStateRoot(),
		"cart",
		"round-2.json",
	)
	if _, err := os.Stat(roundTwo); !errors.Is(
		err,
		fs.ErrNotExist,
	) {
		t.Errorf(
			"round file %s was left behind: %v",
			roundTwo,
			err,
		)
	}
}

// Short comments such as "// Sums the prices." repeat across files. Two
// items are the same comment only when both their file and old match, so
// equal wording in two files must not be refused as a repeated item.
func TestAddAcceptsSameWordingInTwoFiles(t *testing.T) {
	repo := startSecondRound(
		t,
		map[string]string{
			"tax.go": "package cart\n\n// Sums the prices.\nfunc SumTax() {}\n",
		},
	)
	sumTax := review.Item{
		ID:       4,
		File:     "tax.go",
		Proposal: review.ProposeKeep,
		Old:      rewordTotal.Old,
	}

	err := addRound(
		"cart",
		writeProposals(
			t,
			repo,
			sumTax,
		),
		findRepoChecker(t),
	)

	if err != nil {
		t.Fatalf(
			"addRound() error = %v",
			err,
		)
	}
}
