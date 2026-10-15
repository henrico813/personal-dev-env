// Package cmd runs the pde-comment-review commands. Main picks one from the
// first argument.
package cmd

import (
	"encoding/json"
	"fmt"
	"os"

	"pde-comment-review/internal/review"
)

const usage = `usage:
  pde-comment-review add NAME FILE
      save proposals FILE as the next round of review NAME
  pde-comment-review notes NAME
      print the owner's open notes on review NAME as JSON
  pde-comment-review help
      show this help
`

func Main(args []string) int {
	if len(args) == 1 && args[0] == "help" {
		fmt.Print(usage)
		return 0
	}
	var err error
	switch {
	case len(args) == 3 && args[0] == "add":
		err = addRound(args[1], args[2])
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

func addRound(name, file string) error {
	data, err := os.ReadFile(file)
	if err != nil {
		return err
	}
	p, err := review.ParseProposals(data)
	if err != nil {
		return err
	}
	round, err := review.AddRound(name, p)
	if err != nil {
		return err
	}
	fmt.Printf("saved round %d of review %s\n", round, name)
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
