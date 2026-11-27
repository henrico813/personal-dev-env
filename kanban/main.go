// Command card keeps kanban cards as notes in the Obsidian vault. This Go
// version runs every command, and card.py stays the installed card for now.
package main

import (
	"os"

	"kanban/internal/cmd"
)

func main() {
	os.Exit(cmd.Main(os.Args))
}
