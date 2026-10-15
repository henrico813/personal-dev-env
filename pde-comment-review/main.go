// Package main runs pde-comment-review, which keeps the owner's reviews of
// the code comments an AI proposes.
package main

import (
	"os"

	"pde-comment-review/internal/cmd"
)

func main() {
	os.Exit(cmd.Main(os.Args[1:]))
}
