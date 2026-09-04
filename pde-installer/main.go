package main

import (
	"fmt"
	"os"

	"pde-installer/internal/installer"
)

func main() {
	if err := installer.NewCommand().Execute(); err != nil {
		_, _ = fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
