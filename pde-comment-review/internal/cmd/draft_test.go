package cmd

import (
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"pde-comment-review/internal/review"
)

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
	{"file": "price.go", "name": "Price", "text": "// Prices are in cents, so sums never round.\n"},
	{"file": "cart.go", "name": "Cart", "text": "// Holds the prices to pay.\n"}
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

// Returns a clone of a test repository on a new branch, kept apart from the
// owner's Git settings and reviews.
func newShopClone(t *testing.T, branch string) string {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	t.Setenv("GIT_CONFIG_GLOBAL", "/dev/null")
	t.Setenv("GIT_CONFIG_NOSYSTEM", "1")
	t.Setenv("GIT_AUTHOR_NAME", "Test")
	t.Setenv("GIT_AUTHOR_EMAIL", "test@example.com")
	t.Setenv("GIT_COMMITTER_NAME", "Test")
	t.Setenv("GIT_COMMITTER_EMAIL", "test@example.com")
	root := t.TempDir()
	origin := filepath.Join(root, "origin")
	if err := os.Mkdir(origin, 0o755); err != nil {
		t.Fatal(err)
	}
	writeFile(
		t,
		filepath.Join(origin, "shop.go"),
		shopGo,
	)
	writeFile(
		t,
		filepath.Join(origin, "price.go"),
		"package shop\n\n// Prices are in cents, so sums never round.\ntype Price int\n\n// Formats a price.\nfunc (Price) String() string { return \"\" }\n",
	)
	// The branch never changes cart.go, so its unapproved Cart.Add must not
	// become an item.
	writeFile(
		t,
		filepath.Join(origin, "cart.go"),
		"package shop\n\n// Holds the prices to pay.\ntype Cart struct{}\n\n// Adds a price.\nfunc (Cart) Add() {}\n",
	)
	runScript(
		t,
		origin,
		"git init -q -b main && git add . && git commit -q -m base",
	)
	runScript(
		t,
		root,
		"git clone -q origin shop && git -C shop switch -q -c "+branch,
	)
	repo := filepath.Join(root, "shop")
	writeFile(
		t,
		filepath.Join(repo, ".code-comments.json"),
		shopApprovals,
	)
	return repo
}

func runScript(t *testing.T, dir, script string) string {
	t.Helper()
	run := exec.Command(
		"sh",
		"-ec",
		script,
	)
	run.Dir = dir
	output, err := run.CombinedOutput()
	if err != nil {
		t.Fatalf(
			"%s: %v\n%s",
			script,
			err,
			output,
		)
	}
	return strings.TrimSpace(string(output))
}

// Verifies that a draft lists every comment the owner must answer and no
// other, so no comment is approved unseen.
func TestDraftFindsCommentsNeedingTheOwner(t *testing.T) {
	repo := newShopClone(t, "vibe/Add-Tax")
	// Pushed, so the branch's upstream is its own copy; the base must still
	// be main, or the moved file would look deleted.
	runScript(
		t,
		repo,
		"git mv price.go prices.go && git commit -q -m 'move prices' && git push -q -u origin HEAD",
	)
	writeFile(
		t,
		filepath.Join(repo, "shop.go"),
		editedShopGo,
	)
	writeFile(
		t,
		filepath.Join(repo, "clear.go"),
		"package shop\n\n// Empties the cart.\nfunc Clear() {}\n",
	)

	d, err := saveBranchDraft(repo, findRepoChecker(t))

	if err != nil {
		t.Fatalf("saveBranchDraft() error = %v", err)
	}
	want := review.Draft{
		Base: runScript(
			t,
			repo,
			"git rev-parse origin/main",
		),
		Proposals: review.Proposals{
			Version: review.Version,
			Repo:    repo,
			Items: []review.Item{
				{
					ID:       1,
					File:     "clear.go",
					Name:     "Clear",
					Old:      "// Empties the cart.\n",
					Proposal: review.ProposeKeep,
				},
				{
					ID:       2,
					File:     "shop.go",
					Name:     "Total",
					Old:      "// Sums the prices.\n",
					Proposal: review.ProposeReword,
					New:      "// Returns the sum of prices, in cents.\n",
				},
				{
					ID:       3,
					File:     "shop.go",
					Name:     "SalesTax",
					Old:      "// Returns the tax on a price.\n",
					Proposal: review.ProposeKeep,
				},
				{
					ID:       4,
					File:     "shop.go",
					Name:     "Discount",
					Old:      "// Returns the discount on a price, in cents.\n",
					Proposal: review.ProposeKeep,
				},
				{
					ID:          5,
					File:        "shop.go",
					Name:        "Valid",
					Old:         "// Reports whether every price is above zero.\n",
					Proposal:    review.ProposeCut,
					CodeRemoved: true,
				},
			},
			Renames: []string{
				"Tax=SalesTax",
				"price.go=prices.go",
			},
		},
	}
	if !reflect.DeepEqual(d, want) {
		t.Errorf(
			"draft =\n%+v\nwant\n%+v",
			d,
			want,
		)
	}
	data, err := os.ReadFile(filepath.Join(
		review.FindStateRoot(),
		"vibe-add-tax",
		"draft.json",
	))
	if err != nil {
		t.Fatalf("draft was not saved under the branch's name: %v", err)
	}
	var saved review.Draft
	if err := json.Unmarshal(data, &saved); err != nil || !reflect.DeepEqual(saved, want) {
		t.Errorf(
			"saved draft = %+v, %v; want the returned draft",
			saved,
			err,
		)
	}
}

// Verifies that a draft that could mislead the owner is refused, with
// what to fix.
func TestUnsureDraftIsRefused(t *testing.T) {
	tests := []struct {
		name   string
		shopGo string
		script string
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
		{
			// A guessed base could hide the branch's comments.
			name:   "no remote default branch",
			shopGo: shopGo,
			script: "git remote set-head origin -d",
			want:   "set one with git remote set-head REMOTE --auto",
		},
		{
			// A new worktree has none; approved comments would look new.
			name:   "no approval record",
			shopGo: shopGo,
			script: "rm .code-comments.json",
			want:   ".code-comments.json is missing",
		},
		{
			name:   "two remote default branches",
			shopGo: shopGo,
			script: "git remote add -f up ../origin && git remote set-head up --auto",
			want:   "remotes origin and up each have a default branch",
		},
		{
			// A shallow clone can lack the commit the branch started from.
			name:   "no commit shared with the remote default",
			shopGo: shopGo,
			script: "git update-ref refs/remotes/origin/lone $(git commit-tree -m lone $(git hash-object -t tree /dev/null)) && git remote set-head origin lone",
			want:   "the branch shares no commit with origin/HEAD",
		},
	}
	for _, tt := range tests {
		t.Run(
			tt.name,
			func(t *testing.T) {
				repo := newShopClone(t, "feat")
				writeFile(
					t,
					filepath.Join(repo, "shop.go"),
					tt.shopGo,
				)
				runScript(
					t,
					repo,
					tt.script,
				)

				_, err := saveBranchDraft(repo, findRepoChecker(t))

				if err == nil || !strings.Contains(err.Error(), tt.want) || !strings.Contains(err.Error(), "nothing was saved") {
					t.Fatalf(
						"saveBranchDraft() error = %v, want it to contain %q and say nothing was saved",
						err,
						tt.want,
					)
				}
				_, err = os.Stat(filepath.Join(
					review.FindStateRoot(),
					"feat",
					"draft.json",
				))
				if !errors.Is(err, fs.ErrNotExist) {
					t.Errorf("draft.json was saved: %v", err)
				}
			},
		)
	}
}
