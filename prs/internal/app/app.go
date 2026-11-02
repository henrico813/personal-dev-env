// Package app runs prs, starting at Run, then pickPullRequest for the PR
// list screen and showSummary, showComments and showDiff for the others. A
// screen's flow, keys and help are in its file here, such as comments.go,
// its text is in the file of the same name in view, and its side pane and
// help text row are in layout.
package app

import (
	"fmt"
	"io"
	"os"
	"os/signal"
	"path/filepath"
	"regexp"
	"strconv"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
	"prs/internal/view"
)

// Holds what every command needs. Each fzf event starts a new process, so
// nothing here outlives one command.
type prsApp struct {
	// This binary's path, quoted for the shell that fzf runs commands with.
	self   string
	color  bool
	stdin  io.Reader
	stdout io.Writer
	stderr io.Writer
	now    time.Time
	store  cache.Store
}

// The hidden commands that fzf runs again as prs, on screen events and most
// keys in an fzf screen. Each handler's comment shows its arguments.
var hiddenCommands = map[string]func(*prsApp, []string) int{
	"_rows":        (*prsApp).printListRows,
	"_layout":      (*prsApp).printListLayout,
	"_meta":        (*prsApp).printDetails,
	"_prompt":      (*prsApp).printListPrompt,
	"_summary":     (*prsApp).openSummary,
	"_comments":    (*prsApp).openComments,
	"_item":        (*prsApp).printItem,
	"_item_page":   (*prsApp).openItem,
	"_diff":        (*prsApp).openDiff,
	"_file":        (*prsApp).printFileDiff,
	"_pane_layout": (*prsApp).printPaneLayout,
	"_tap":         (*prsApp).printTapAction,
	"_g":           (*prsApp).printFirstOnSecondG,
	"_help":        (*prsApp).printHelpAction,
	"_keys":        (*prsApp).printHelp,
}

var numberPattern = regexp.MustCompile(`^[0-9]+$`)

// Runs the command in args and returns the exit code. It leaves Ctrl-C
// caught for the rest of the process, and a hang-up or TERM while a
// temporary folder is open exits the process instead of returning.
//
//	prs            the PR list screen
//	prs NUM        one PR's summary screen, from the current folder's repo
//	prs REPO NUM   the same for any repo, such as prs owner/app 12
func Run(
	args []string,
	stdin io.Reader,
	stdout,
	stderr io.Writer,
) int {
	// prs catches Ctrl-C so it can still remove its folders after fzf, less
	// or delta handles the key. Catching the signal, rather than ignoring
	// it, leaves those programs their usual Ctrl-C.
	signal.Notify(
		make(
			chan os.Signal,
			1,
		),
		os.Interrupt,
	)
	self, err := os.Executable()
	if err == nil {
		self, err = filepath.EvalSymlinks(self)
	}
	if err != nil {
		_, _ = fmt.Fprintln(
			stderr,
			"prs: cannot find its own path:",
			err,
		)
		return 1
	}
	store, err := cache.Open()
	if err != nil {
		_, _ = fmt.Fprintln(
			stderr,
			"prs:",
			err,
		)
		return 1
	}
	a := &prsApp{
		self:   quoteShell(self),
		color:  os.Getenv("NO_COLOR") == "",
		stdin:  stdin,
		stdout: stdout,
		stderr: stderr,
		now:    time.Now(),
		store:  store,
	}
	if len(args) == 0 {
		return a.pickPullRequest()
	}
	if run, ok := hiddenCommands[args[0]]; ok {
		// A command typed by hand without its arguments gets empty ones
		// rather than a crash.
		return run(
			a,
			append(
				args[1:],
				"",
				"",
				"",
				"",
			),
		)
	}
	if len(args) == 1 && numberPattern.MatchString(args[0]) {
		repo, err := github.FindCurrentRepo()
		if err != nil {
			a.printError(err.Error())
			a.printError("Could not find this folder's GitHub repository.")
			return 1
		}
		args = []string{
			repo,
			args[0],
		}
	}
	if len(args) != 2 || !numberPattern.MatchString(args[1]) {
		a.printError("usage: prs [NUMBER | OWNER/REPO NUMBER]")
		return 2
	}
	return a.openSummaryAlone(
		args[0],
		readNumber(args[1]),
	)
}

// Returns the style for text wrapped to width columns.
func (a *prsApp) makeStyle(width int) view.Style {
	return view.Style{
		Width: width,
		Now:   a.now,
		Color: a.color,
	}
}

func (a *prsApp) printText(text string) {
	_, _ = io.WriteString(
		a.stdout,
		text,
	)
}

func (a *prsApp) printLine(text string) {
	a.printText(text + "\n")
}

func (a *prsApp) printError(text string) {
	_, _ = io.WriteString(
		a.stderr,
		text+"\n",
	)
}

// Returns a number from a hidden command's argument, such as a pull
// request's or a file's, or 0.
func readNumber(text string) int {
	number, _ := strconv.Atoi(text)
	return number
}
