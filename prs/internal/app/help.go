package app

import (
	"os"
	"strconv"
	"strings"
)

// The rows of the longest fzf screen's help, so the help key knows whether the
// side pane has room for it. Each help fits 35 columns and 13 rows, so it fits
// a 37x14 popup of Herdr, the terminal workspace used on the phone.
var helpRows = max(
	strings.Count(
		listHelp,
		"\n",
	),
	strings.Count(
		commentsHelp,
		"\n",
	),
	strings.Count(
		filePickerHelp,
		"\n",
	),
)

// Prints the fzf action for ?, as _help SCREEN. The help shows in the side
// pane when it has room, or full screen in less.
func (a *prsApp) printHelpAction(args []string) int {
	lines, _ := strconv.Atoi(os.Getenv("FZF_PREVIEW_LINES"))
	if lines >= helpRows {
		a.printLine("preview(" + a.self + " _keys " + args[0] + ")")
	} else {
		a.printLine("execute(" + a.self + " _keys " + args[0] + " page)")
	}
	return 0
}

// Prints an fzf screen's help for the side pane, as _keys SCREEN, or shows
// it full screen in less, as _keys SCREEN page.
func (a *prsApp) printHelp(args []string) int {
	if args[1] == "page" {
		a.showHelpPage(chooseHelp(args[0]))
		return 0
	}
	a.printText(chooseHelp(args[0]))
	return 0
}

// Shows a help full screen in less until h or q.
func (a *prsApp) showHelpPage(help string) {
	a.openInLess(
		"keys  h back",
		help,
		"",
	)
}

// Returns the help of the fzf screen named list, comments or picker.
func chooseHelp(screen string) string {
	switch screen {
	case "picker":
		return filePickerHelp
	case "comments":
		return commentsHelp
	}
	return listHelp
}
