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
	requests ...review.FeedbackRequest,
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
	batchFile := filepath.Join(
		review.FindStateRoot(),
		"cart",
		"round-1.json",
	)
	return batchFile, addBatchFile(
		"cart",
		writeBatchFile(
			t,
			repo,
			requests...,
		),
		buildRepoCheckerOnPath(t),
	)
}

// Writes items as a proposals file for repo and returns its path.
func writeBatchFile(
	t *testing.T,
	repo string,
	requests ...review.FeedbackRequest,
) string {
	t.Helper()
	data, err := json.Marshal(review.Batch{
		Version:  review.Version,
		Repo:     repo,
		Requests: requests,
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
func startSecondBatch(
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
		extra  []review.FeedbackRequest // requests after the three every row has
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
			extra: []review.FeedbackRequest{{
				ID:         4,
				File:       "cart.go",
				Suggestion: review.SuggestKeep,
				Old:        "// Reports whether no price is negative.\n",
			}},
			want: []string{
				"cart.go Valid: differs from the text the owner approved",
				"\n    // Reports whether every price is above zero.",
			},
		},
		{
			name:   "two items for one comment",
			onDisk: cartGo,
			extra: []review.FeedbackRequest{{
				ID:         4,
				File:       "cart.go",
				Suggestion: review.SuggestKeep,
				Old:        keepTooHigh.Old,
			}},
			want: []string{
				"items 3 and 4 are the same comment in cart.go",
			},
		},
		{
			name:   "drafted comment worded like another",
			onDisk: cartGo + "\n// Reports whether every price is above zero.\nfunc Positive() bool { return true }\n",
			extra: []review.FeedbackRequest{{
				ID:         4,
				File:       "cart.go",
				Name:       "Positive",
				Suggestion: review.SuggestKeep,
				Old:        "// Reports whether every price is above zero.\n",
			}},
			want: []string{
				"item 4: cart.go: comment found 2 times",
			},
		},
		{
			name:   "reword that adds a directive",
			onDisk: cartGo,
			extra: []review.FeedbackRequest{{
				ID:         4,
				File:       "cart.go",
				Suggestion: review.SuggestReword,
				Old:        "// Reports whether every price is above zero.\n",
				New:        "// Reports whether every price is above zero.\n//go:noinline\n",
			}},
			want: []string{
				"item 4: cart.go Valid: the new text could change how the code builds",
			},
		},
		{
			name:   "removed code back",
			onDisk: cartGo,
			extra: []review.FeedbackRequest{{
				ID:          4,
				File:        "cart.go",
				Name:        "Valid",
				Suggestion:  review.SuggestCut,
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
				requests := append(
					[]review.FeedbackRequest{
						cutCurrency,
						rewordTotal,
						keepTooHigh,
					},
					tt.extra...,
				)

				batchFile, err := addScratchReview(
					t,
					tt.onDisk,
					validApproval,
					requests...,
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
				_, err = os.Stat(batchFile)
				if !errors.Is(
					err,
					fs.ErrNotExist,
				) {
					t.Errorf(
						"round file %s was left behind: %v",
						batchFile,
						err,
					)
				}
			},
		)
	}
}

// Verifies that an item may quote the approved wording, which is how
// agents propose rewording an approved comment.
func TestAddSavesRequestQuotingApprovedText(t *testing.T) {
	rewordValid := review.FeedbackRequest{
		ID:         4,
		File:       "cart.go",
		Suggestion: review.SuggestReword,
		Old:        "// Reports whether every price is above zero.\n",
		New:        "// Reports whether no price is negative.\n",
	}

	batchFile, err := addScratchReview(
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
	if _, err := os.Stat(batchFile); err != nil {
		t.Errorf(
			"round 1 was not saved: %v",
			err,
		)
	}
}

// Verifies that two approved comments with the same words are told apart
// by name, so an agent can propose rewording both.
func TestAddKeepsSameWordedCommentsApart(t *testing.T) {
	rewordValid := review.FeedbackRequest{
		ID:         1,
		File:       "cart.go",
		Suggestion: review.SuggestReword,
		Old:        "// Reports whether every price is above zero.\n",
		New:        "// Reports whether no price is negative.\n",
	}
	rewordPositive := review.FeedbackRequest{
		ID:         2,
		File:       "cart.go",
		Name:       "Positive",
		Suggestion: review.SuggestReword,
		Old:        "// Reports whether every price is above zero.\n",
		New:        "// Reports whether no price is below zero.\n",
	}

	batchFile, err := addScratchReview(
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
	if _, err := os.Stat(batchFile); err != nil {
		t.Errorf(
			"round 1 was not saved: %v",
			err,
		)
	}
}

// Verifies that saving a later round rechecks earlier rounds' files, so a
// problem there is caught before the owner answers.
func TestAddRechecksFilesFromEarlierBatches(t *testing.T) {
	repo := startSecondBatch(
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
	keepTax := review.FeedbackRequest{
		ID:         4,
		File:       "tax.go",
		Suggestion: review.SuggestKeep,
		Old:        "// Returns the tax on a price.\n",
	}

	err := addBatchFile(
		"cart",
		writeBatchFile(
			t,
			repo,
			keepTax,
		),
		buildRepoCheckerOnPath(t),
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
	batchTwo := filepath.Join(
		review.FindStateRoot(),
		"cart",
		"round-2.json",
	)
	if _, err := os.Stat(batchTwo); !errors.Is(
		err,
		fs.ErrNotExist,
	) {
		t.Errorf(
			"round file %s was left behind: %v",
			batchTwo,
			err,
		)
	}
}

// Keeping a drafted item puts the approved text back over an agent's edit,
// so add must check that write too, or it fails after the owner answers.
func TestAddRefusesKeepInUnformattedFile(t *testing.T) {
	editedAndUnformatted := strings.NewReplacer(
		"every price is above zero",
		"no price is negative",
		"Max = 50",
		"Max  =  50",
	).Replace(cartGo)
	keepValid := review.FeedbackRequest{
		ID:         1,
		File:       "cart.go",
		Name:       "Valid",
		Suggestion: review.SuggestKeep,
		Old:        "// Reports whether every price is above zero.\n",
	}

	batchFile, err := addScratchReview(
		t,
		editedAndUnformatted,
		validApproval,
		keepValid,
	)

	want := "nothing was saved: gofmt would change cart.go"
	if err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf(
			"addRound() error = %v, want it to contain %q",
			err,
			want,
		)
	}
	if _, err := os.Stat(batchFile); !errors.Is(
		err,
		fs.ErrNotExist,
	) {
		t.Errorf(
			"round file %s was left behind: %v",
			batchFile,
			err,
		)
	}
}

// Python reviews stay off until the review screen can show them, so add
// must refuse a hand-written Python item before the owner sees it.
func TestAddRefusesPythonWhileReviewsOff(t *testing.T) {
	repo := startSecondBatch(
		t,
		map[string]string{
			"tax.py": "# Returns the tax on a price.\ndef tax(price):\n    return 0\n",
		},
	)
	keepTax := review.FeedbackRequest{
		ID:         4,
		File:       "tax.py",
		Suggestion: review.SuggestKeep,
		Old:        "# Returns the tax on a price.\n",
	}

	err := addBatchFile(
		"cart",
		writeBatchFile(
			t,
			repo,
			keepTax,
		),
		buildRepoCheckerOnPath(t),
	)

	want := "nothing was saved: item 4: tax.py: owner reviews are off for this language"
	if err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf(
			"addRound() error = %v, want it to contain %q",
			err,
			want,
		)
	}
	batchTwo := filepath.Join(
		review.FindStateRoot(),
		"cart",
		"round-2.json",
	)
	if _, err := os.Stat(batchTwo); !errors.Is(
		err,
		fs.ErrNotExist,
	) {
		t.Errorf(
			"round file %s was left behind: %v",
			batchTwo,
			err,
		)
	}
}

// Short comments such as "// Sums the prices." repeat across files. Two
// items are the same comment only when both their file and old match, so
// equal wording in two files must not be refused as a repeated item.
func TestAddAcceptsSameWordingInTwoFiles(t *testing.T) {
	repo := startSecondBatch(
		t,
		map[string]string{
			"tax.go": "package cart\n\n// Sums the prices.\nfunc SumTax() {}\n",
		},
	)
	sumTax := review.FeedbackRequest{
		ID:         4,
		File:       "tax.go",
		Suggestion: review.SuggestKeep,
		Old:        rewordTotal.Old,
	}

	err := addBatchFile(
		"cart",
		writeBatchFile(
			t,
			repo,
			sumTax,
		),
		buildRepoCheckerOnPath(t),
	)

	if err != nil {
		t.Fatalf(
			"addRound() error = %v",
			err,
		)
	}
}

// Verifies that add refuses an earlier request's id for another comment,
// which would drop the owner's answer.
func TestAddRefusesIDOfAnotherComment(t *testing.T) {
	repo := startSecondBatch(
		t,
		map[string]string{"cart.go": cartGo + "\nfunc Clear() {\n\t// Empties nothing yet.\n}\n"},
	)
	validAsRequest2 := review.FeedbackRequest{
		ID:         2,
		File:       "cart.go",
		Suggestion: review.SuggestKeep,
		Old:        "// Reports whether every price is above zero.\n",
	}
	// A comment inside a function has no name, yet takes a named request's id.
	unnamedAsRequest3 := review.FeedbackRequest{
		ID:         3,
		File:       "cart.go",
		Suggestion: review.SuggestKeep,
		Old:        "\t// Empties nothing yet.\n",
	}

	err := addBatchFile(
		"cart",
		writeBatchFile(
			t,
			repo,
			validAsRequest2,
			unnamedAsRequest3,
		),
		buildRepoCheckerOnPath(t),
	)

	want := "nothing was saved: item 2: an earlier round used this id for another comment"
	if err == nil || !strings.Contains(err.Error(), want) || !strings.Contains(err.Error(), "item 3: an earlier round used this id") {
		t.Fatalf("addRound() error = %v, want items 2 and 3 refused", err)
	}
}
