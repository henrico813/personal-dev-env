// Package cmd runs the pde-comment-review commands. Main picks one from the
// first argument.
package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

const usage = `usage:
  pde-comment-review add NAME FILE
      save proposals FILE as the next round of review NAME
  pde-comment-review open NAME
      answer the items of review NAME one per screen, each saved at once:
      o keep, c cut, a accept, e edit in nvim, m note for the AI,
      v view the file, s search with rg, j/k scroll, h/l previous/next
      item, q quit; after the last answer, every answer is written into the
      code and approved, and d shows the diff
  pde-comment-review notes NAME
      print the owner's open notes on review NAME as JSON
  pde-comment-review help
      show this help
Herdr runs pde-comment-review popup, edit and diff itself, and
pde-comment-review open with no NAME when the owner presses Ctrl+b e.
`

func Main(args []string) int {
	setUpToolEnvironment()
	if len(args) == 1 && args[0] == "help" {
		fmt.Print(usage)
		return 0
	}
	var err error
	switch {
	case len(args) == 3 && args[0] == "add":
		var checker string
		if checker, err = findCheckerPath(); err == nil {
			err = addRound(
				args[1],
				args[2],
				checker,
			)
		}
	case len(args) == 1 && args[0] == "open":
		err = openWaitingReview()
	case len(args) == 2 && args[0] == "open":
		err = answerReview(args[1], os.Stdin, os.Stdout, popup.Settle)
	case len(args) == 1 && args[0] == "popup":
		err = runReviewPopup(os.Stdin, os.Stdout)
	case len(args) == 1 && args[0] == "edit":
		err = runEditTab()
	case len(args) == 1 && args[0] == "diff":
		err = runDiffTab()
	case len(args) == 2 && args[0] == "notes":
		err = printNotes(args[1])
	default:
		fmt.Fprint(os.Stderr, usage)
		return 2
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "pde-comment-review: %v\n", err)
		return 1
	}
	return 0
}

// Herdr runs plugin commands with its server's environment, which may lack
// ~/.local/bin and aqua's folder and settings. Without the settings, aqua's
// rg and delta fail with "command is not found". Unset settings get the
// full profile's values from chezmoi/dot_zshrc.tmpl.
func setUpToolEnvironment() {
	home, err := os.UserHomeDir()
	if err != nil {
		return
	}
	aquaConfig := filepath.Join(home, ".config", "aquaproj-aqua")
	for name, value := range map[string]string{
		"AQUA_ROOT_DIR":       filepath.Join(home, ".local", "share", "aquaproj-aqua"),
		"AQUA_GLOBAL_CONFIG":  filepath.Join(aquaConfig, "aqua.yaml"),
		"AQUA_CHECKSUMS_PATH": filepath.Join(aquaConfig, "aqua-checksums.json"),
	} {
		if os.Getenv(name) == "" {
			os.Setenv(name, value)
		}
	}
	folders := []string{
		os.Getenv("PATH"),
		filepath.Join(home, ".local", "bin"),
		filepath.Join(os.Getenv("AQUA_ROOT_DIR"), "bin"),
	}
	os.Setenv("PATH", strings.Join(folders, string(os.PathListSeparator)))
}

func addRound(
	name string,
	file string,
	checker string,
) error {
	data, err := os.ReadFile(file)
	if err != nil {
		return err
	}
	p, err := review.ParseProposals(data)
	if err != nil {
		return err
	}
	next, err := review.BuildReviewWithRound(
		name,
		p,
	)
	if err != nil {
		return err
	}
	err = refuseUnwritableReview(
		next,
		checker,
	)
	if err != nil {
		return fmt.Errorf(
			"nothing was saved: %w",
			err,
		)
	}
	round, err := review.AddRound(name, p)
	if err != nil {
		return err
	}
	r, err := review.OpenReview(name)
	if err != nil {
		return err
	}
	origin := findReviewOrigin(p.Repo)
	if err := r.SetOrigin(origin); err != nil {
		return err
	}
	fmt.Printf("saved round %d of review %s: %s\n", round, name, origin.Label())
	return nil
}

type noteForAI struct {
	ID   int         `json:"id"`
	Note string      `json:"note"`
	Item review.Item `json:"item"`
}

func printNotes(name string) error {
	r, err := review.OpenReview(name)
	if err != nil {
		return err
	}
	out := struct {
		Review string      `json:"review"`
		Repo   string      `json:"repo"`
		Notes  []noteForAI `json:"notes"`
	}{Review: r.Name, Repo: r.Repo(), Notes: []noteForAI{}}
	for _, e := range r.ListEntries() {
		if e.Note != "" {
			out.Notes = append(out.Notes, noteForAI{ID: e.Item.ID, Note: e.Note, Item: e.Item})
		}
	}
	data, err := json.MarshalIndent(out, "", "  ")
	if err != nil {
		return err
	}
	_, err = fmt.Printf("%s\n", data)
	return err
}
