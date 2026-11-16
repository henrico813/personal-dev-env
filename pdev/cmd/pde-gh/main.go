// Package main is pde-gh, which refuses pull request writes as gh and asks
// pde-pr-approve for approval of them as pde-gh-write.
package main

import (
	"os"

	"pdev/internal/ghtool"
)

func main() {
	os.Exit(ghtool.Main(os.Args))
}
