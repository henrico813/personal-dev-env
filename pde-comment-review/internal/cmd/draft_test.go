package cmd

import (
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"pde-comment-review/internal/review"
)

// Written line by line, not as one raw string, so the comment checker
// does not take these lines for this file's own comments.
const shopGo = "package shop\n" +
	"\n" +
	"// Sums the prices.\n" +
	"func Total(prices []int) int { return 0 }\n" +
	"\n" +
	"// Reports whether every price is above zero.\n" +
	"func Valid(prices []int) bool { return true }\n" +
	"\n" +
	"// Returns the tax on a price.\n" +
	"func Tax(price int) int { return 0 }\n" +
	"\n" +
	"// Rounds a price to whole cents.\n" +
	"func Round(price int) int { return price }\n" +
	"\n" +
	"// Returns the discount on a price.\n" +
	"func Discount(price int) int { return 0 }\n" +
	"\n" +
	"// Registers the shop's prices.\n" +
	"func init() {}\n"

const shopApprovals = `{"approved": [
	{"file": "shop.go", "name": "Total", "text": "// Sums the prices.\n"},
	{"file": "shop.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"},
	{"file": "shop.go", "name": "Tax", "text": "// Returns the tax on a price.\n"},
	{"file": "shop.go", "name": "init", "text": "// Registers the shop's prices.\n"},
	{"file": "shop.go", "name": "Reset", "text": ""},
	{"file": "price.go", "name": "Price", "text": "// Prices are in cents, so sums never round.\n"}
], "avoid": {}}`

// The branch's shop.go: Total's approved comment edited, Valid deleted,
// Tax renamed to SalesTax, and Discount's unapproved comment edited.
const editedShopGo = "package shop\n" +
	"\n" +
	"// Returns the sum of prices, in cents.\n" +
	"func Total(prices []int) int { return 0 }\n" +
	"\n" +
	"// Returns the tax on a price.\n" +
	"func SalesTax(price int) int { return 0 }\n" +
	"\n" +
	"// Rounds a price to whole cents.\n" +
	"func Round(price int) int { return price }\n" +
	"\n" +
	"// Returns the discount on a price, in cents.\n" +
	"func Discount(price int) int { return 0 }\n" +
	"\n" +
	"// Registers the shop's prices.\n" +
	"func init() {}\n"

func writeShopCheckout(t *testing.T) string {
	t.Helper()
	repo := t.TempDir()
	writeFile(
		t,
		filepath.Join(repo, "shop.go"),
		shopGo,
	)
	writeFile(
		t,
		filepath.Join(repo, "price.go"),
		"package shop\n\n// Prices are in cents, so sums never round.\ntype Price int\n",
	)
	writeFile(
		t,
		filepath.Join(repo, ".code-comments.json"),
		shopApprovals,
	)
	return repo
}

// The owner answers only what the draft lists, and the write after the
// last answer approves it. A missed edit or a guessed rename would approve
// text the owner never saw; a renamed function taken for deleted code
// would ask the owner to drop a comment that is still there.
func TestDraftFindsCommentsNeedingTheOwner(t *testing.T) {
	repo := writeShopCheckout(t)
	writeFile(
		t,
		filepath.Join(repo, "shop.go"),
		editedShopGo,
	)

	p, err := buildDraft(repo, findRepoChecker(t))

	if err != nil {
		t.Fatalf("buildDraft() error = %v", err)
	}
	want := review.Proposals{
		Version: review.Version,
		Repo:    repo,
		Items: []review.Item{
			{
				ID:       1,
				File:     "shop.go",
				Name:     "Total",
				Old:      "// Sums the prices.\n",
				Proposal: review.ProposeReword,
				New:      "// Returns the sum of prices, in cents.\n",
			},
			{
				ID:       2,
				File:     "shop.go",
				Name:     "SalesTax",
				Old:      "// Returns the tax on a price.\n",
				Proposal: review.ProposeKeep,
			},
			{
				ID:          3,
				File:        "shop.go",
				Name:        "Valid",
				Old:         "// Reports whether every price is above zero.\n",
				Proposal:    review.ProposeCut,
				CodeRemoved: true,
			},
		},
		Renames: []string{
			"Tax=SalesTax",
		},
	}
	if !reflect.DeepEqual(p, want) {
		t.Errorf(
			"draft =\n%+v\nwant\n%+v",
			p,
			want,
		)
	}
}

// A draft the tool cannot be sure of must not reach the owner: the tool
// says what to fix.
func TestUnsureDraftIsRefused(t *testing.T) {
	tests := []struct {
		name   string
		shopGo string
		want   string
	}{
		{
			name: "rename with two candidates",
			shopGo: strings.Replace(
				shopGo,
				"func Tax(price int) int { return 0 }",
				"func SalesTax(price int) int { return 0 }\n\n// Returns the tax on a price.\nfunc UseTax(price int) int { return 0 }",
				1,
			),
			want: "shop.go Tax: the owner approved this comment, and it is gone. It may now be SalesTax or UseTax",
		},
		{
			// Agents have put back comments the owner cut.
			name:   "cut comment put back",
			shopGo: shopGo + "\n// Empties the cart.\nfunc Reset() {}\n",
			want:   "shop.go Reset: the owner cut this comment",
		},
		{
			// The checker names neither init, so the approved one looks
			// deleted though its code is still there.
			name:   "approved name now shared",
			shopGo: shopGo + "\n// Loads the tax table.\nfunc init() {}\n",
			want:   "shop.go init: the owner approved this comment, and now more than one comment in the file has its name",
		},
	}
	for _, tt := range tests {
		t.Run(
			tt.name,
			func(t *testing.T) {
				repo := writeShopCheckout(t)
				writeFile(
					t,
					filepath.Join(repo, "shop.go"),
					tt.shopGo,
				)

				_, err := buildDraft(repo, findRepoChecker(t))

				if err == nil || !strings.Contains(err.Error(), tt.want) {
					t.Fatalf(
						"buildDraft() error = %v, want it to contain %q",
						err,
						tt.want,
					)
				}
			},
		)
	}
}
