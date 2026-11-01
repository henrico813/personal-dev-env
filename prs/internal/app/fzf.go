package app

import (
	"bytes"
	"errors"
	"maps"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"time"

	"prs/internal/layout"
)

// Marker files the fzf screens keep in their temporary folders.
const (
	// Exists while the o key closes the side pane, so a resize keeps it
	// closed.
	paneClosedFile = "pane-closed"
	// Holds the time of a first g, so a second g soon after goes to the
	// first row.
	firstGFile = "g"
)

// Runs fzf on rows with the shared flags, one binding per action and the
// extra flags. Returns what fzf printed, and false only when fzf itself
// failed.
func (a *prsApp) runFzf(
	rows string,
	actions map[string]string,
	extra ...string,
) (
	string,
	bool,
) {
	// The height, border, margin and padding flags undo a box that the
	// shell's FZF_DEFAULT_OPTS may set, such as a border or a smaller height.
	args := []string{
		"--ansi",
		"--exact",
		"--no-sort",
		"--layout=reverse",
		"--height=100%",
		"--no-border",
		"--margin=0",
		"--padding=0",
		"--info=inline-right",
		"--no-separator",
		"--ellipsis=…",
		"--gutter= ",
		"--highlight-line",
		"--track",
		`--info-command=echo "$FZF_MATCH_COUNT/$FZF_TOTAL_COUNT"`,
		"--footer-border=none",
		"--bind=j:down,k:up,G:last",
		"--bind=ctrl-d:half-page-down,ctrl-u:half-page-up",
		"--bind=J:preview-half-page-down,K:preview-half-page-up",
	}
	for _, key := range slices.Sorted(maps.Keys(actions)) {
		args = append(
			args,
			"--bind="+key+":"+actions[key],
		)
	}
	var out bytes.Buffer
	cmd := exec.Command(
		"fzf",
		append(
			args,
			extra...,
		)...,
	)
	cmd.Stdin = strings.NewReader(rows)
	cmd.Stdout = &out
	cmd.Stderr = a.stderr
	err := cmd.Run()
	var exit *exec.ExitError
	// fzf ends with 1 when a key accepts while nothing matches, and with 130
	// on an abort, such as from q or Ctrl-C.
	if errors.As(
		err,
		&exit,
	) && (exit.ExitCode() == 1 || exit.ExitCode() == 130) {
		return out.String(), true
	}
	if err != nil {
		a.printError("prs: fzf failed: " + err.Error())
		return "", false
	}
	return out.String(), true
}

// Returns the fzf actions that open a search, where letters type text
// until Esc. show-input brings back an input row that FZF_DEFAULT_OPTS may
// hide.
func buildSearchStart(letters string) string {
	return "unbind(" + letters + ")+show-input+change-prompt(" + layout.SearchPrompt + ")+change-footer(" +
		layout.SearchKeys + ")"
}

// fzf's names for the keys that the help text rows write capitalized.
var tappedKeyNames = map[string]string{
	"Enter": "enter",
	"Esc":   "esc",
}

// Prints the action of the help text row word that a tap hit, if any.
func (a *prsApp) printTappedAction(
	keys string,
	actions map[string]string,
) {
	key := layout.FindTappedKey(
		keys,
		layout.ReadFzfState().ClickColumn,
	)
	if name, ok := tappedKeyNames[key]; ok {
		key = name
	}
	if action, ok := actions[key]; ok {
		a.printLine(action)
	}
}

// Prints fzf's first action when this g comes within a second of the one
// before, as _g DIR. fzf binds single keys, not sequences, so DIR/g keeps
// the time of the first g.
func (a *prsApp) printFirstOnSecondG(args []string) int {
	marker := filepath.Join(
		args[0],
		firstGFile,
	)
	saved, err := os.ReadFile(marker)
	if err == nil {
		first, err := time.Parse(
			time.RFC3339Nano,
			string(saved),
		)
		if err == nil && a.now.Sub(first) < time.Second {
			_ = os.Remove(marker)
			a.printLine("first")
			return 0
		}
	}
	_ = os.WriteFile(
		marker,
		[]byte(a.now.Format(time.RFC3339Nano)),
		0o600,
	)
	return 0
}

// Opens the side pane in dir when o closed it, and closes it otherwise.
func togglePaneClosed(dir string) {
	marker := filepath.Join(
		dir,
		paneClosedFile,
	)
	if isPresent(marker) {
		_ = os.Remove(marker)
		return
	}
	_ = os.WriteFile(
		marker,
		nil,
		0o600,
	)
}

// Returns whether o closed the side pane in dir.
func isPaneClosed(dir string) bool {
	return isPresent(filepath.Join(
		dir,
		paneClosedFile,
	))
}

// Returns s quoted for the shell that fzf runs commands with.
func quoteShell(s string) string {
	if s != "" && strings.Trim(
		s,
		"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/._-",
	) == "" {
		return s
	}
	return "'" + strings.ReplaceAll(
		s,
		"'",
		`'\''`,
	) + "'"
}

func isPresent(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// Returns the side pane's columns, or 80 when fzf did not say.
func readPreviewWidth() int {
	width, err := strconv.Atoi(os.Getenv("FZF_PREVIEW_COLUMNS"))
	if err != nil {
		return 80
	}
	return width
}
