// Command card keeps kanban cards as notes in the Obsidian vault. This Go
// version runs the read commands; card.py still runs every command.
package main

import (
	"os"

	"kanban/internal/cmd"
)

func main() {
	os.Exit(cmd.Main(os.Args))
}
