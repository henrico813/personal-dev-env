// Package main is pde-pr-approve, where a human answers the pull request
// writes that pde-gh-write asks for in a terminal, a Herdr popup, or on the
// phone through Moshi. pde-gh starts it for the popup and the phone question.
package main

import (
	"os"

	"pdev/internal/cmd"
)

func main() {
	os.Exit(cmd.Main(os.Args))
}
