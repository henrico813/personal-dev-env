// Command card keeps kanban cards as notes in the Obsidian vault.
package main

import (
	"os"

	"kanban/internal/cmd"
)

func main() {
	os.Exit(cmd.Main(os.Args))
}
