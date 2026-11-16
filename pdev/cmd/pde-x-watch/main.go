// Package main warns the i3 desktop user when Xorg gets busy or an X client
// leaks windows. The i3 session starts it from an XDG autostart entry, and it
// runs until logout.
package main

import (
	"os"

	"pdev/internal/xwatch"
)

func main() {
	os.Exit(xwatch.WatchDisplay(os.Stderr))
}
