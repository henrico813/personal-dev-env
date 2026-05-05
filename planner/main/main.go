package main

import (
	"os"

	"planner/internal"
)

func main() {
	os.Exit(internal.Execute(os.Args[1:], os.Stdout, os.Stderr))
}
