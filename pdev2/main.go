// Package main handles PR writes that run only after a human approves them in
// a terminal, a Herdr popup, or on the phone through Moshi.
//
// An agent runs pde-gh-write with gh pr arguments. It stores a pending record
// and waits up to 90 seconds for a human to answer in the popup, on the phone,
// or with pde-pr-approve ID.
// Rerunning the same command reports the outcome instead of running gh again.
package main

import (
	"os"

	"pdev2/internal/cmd"
)

func main() {
	os.Exit(cmd.Main(os.Args))
}
