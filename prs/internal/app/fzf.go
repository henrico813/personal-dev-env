package app

import (
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

// The actions every fzf screen binds, besides its own.
var sharedActions = map[string]string{
	"j":      "down",
	"k":      "up",
	"G":      "last",
	"ctrl-d": "half-page-down",
	"ctrl-u": "half-page-up",
	"J":      "preview-half-page-down",
	"K":      "preview-half-page-up",
}

// The screens that share _comments_or_picker_layout and BuildCommentsOrPickerKeys, by the name their
// hidden commands pass.
var commentsOrPickerScreens = map[string]layout.Screen{
	"comments": layout.CommentsScreen,
	"picker":   layout.FilePicker,
}

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
	}
	for _, key := range slices.Sorted(maps.Keys(sharedActions)) {
		args = append(
			args,
			"--bind="+key+":"+sharedActions[key],
		)
	}
	for _, key := range slices.Sorted(maps.Keys(actions)) {
		args = append(
			args,
			"--bind="+key+":"+actions[key],
		)
	}
	cmd := exec.Command(
		"fzf",
		append(
			args,
			extra...,
		)...,
	)
	// fzf passes its own environment on, so an FZF_CURRENT_ITEM from a prs
	// started in another fzf would reach the side pane of an empty list.
	cmd.Env = slices.DeleteFunc(
		os.Environ(),
		func(variable string) bool {
			return strings.HasPrefix(
				variable,
				"FZF_CURRENT_ITEM=",
			)
		},
	)
	cmd.Stdin = strings.NewReader(rows)
	cmd.Stderr = a.stderr
	out, err := cmd.Output()
	var exit *exec.ExitError
	// fzf ends with 1 when a key accepts while nothing matches, and with 130
	// on an abort, such as from q or Ctrl-C.
	if errors.As(
		err,
		&exit,
	) && (exit.ExitCode() == 1 || exit.ExitCode() == 130) {
		return string(out), true
	}
	if err != nil {
		a.printError("prs: fzf failed: " + err.Error())
		return "", false
	}
	return string(out), true
}

// Returns actions with / and Esc added. / frees every one-character key the
// screen binds for typing until Esc binds them again, and onEsc runs after
// Esc.
func addSearchKeys(
	actions map[string]string,
	prompt,
	onEsc string,
) map[string]string {
	letters := []string{
		"/",
	}
	for _, bound := range []map[string]string{
		sharedActions,
		actions,
	} {
		for key := range bound {
			if len(key) == 1 {
				letters = append(
					letters,
					key,
				)
			}
		}
	}
	slices.Sort(letters)
	list := strings.Join(
		letters,
		",",
	)
	// show-input brings back an input row that FZF_DEFAULT_OPTS may hide.
	actions["/"] = "unbind(" + list + ")+show-input+change-prompt(" + layout.SearchPrompt + ")+change-footer(" +
		layout.SearchKeys + ")"
	actions["esc"] = "clear-query+rebind(" + list + ")+change-prompt(" + prompt + ")+" + onEsc
	return actions
}

// Prints trigger(KEY) for the help text row word under a tap, as _tap
// SCREEN DIR, so fzf runs that key's own binding. SCREEN is list, comments
// or picker.
func (a *prsApp) printTapAction(args []string) int {
	state := layout.ReadFzfState()
	var keys string
	if screen, ok := commentsOrPickerScreens[args[0]]; ok {
		keys = layout.BuildCommentsOrPickerKeys(
			state,
			screen,
		)
	} else {
		keys = buildListKeys(
			state,
			args[1],
		)
	}
	key := layout.FindTappedKey(
		keys,
		state.ClickColumn,
	)
	switch {
	case key == "Esc" || key == "Enter":
		key = strings.ToLower(key)
	case len(key) != 1:
		// A word that is not a key, such as GitHub in "GitHub failed", runs
		// nothing.
		return 0
	}
	a.printLine("trigger(" + key + ")")
	return 0
}

// Prints fzf actions that fit the comments screen or the file picker to the
// screen, as _comments_or_picker_layout SCREEN DIR EVENT. Start, result and
// Esc pass result, toggle opens or closes the side pane, and refresh reruns
// it after a resize.
func (a *prsApp) printCommentsOrPickerLayout(args []string) int {
	screen, dir, event := commentsOrPickerScreens[args[0]], args[1], args[2]
	if event == "toggle" {
		togglePaneClosed(dir)
	}
	state := layout.ReadFzfState()
	actions := []string{
		layout.ChoosePaneWindow(
			state,
			screen,
			isPaneClosed(dir),
		),
		"change-footer(" + layout.BuildCommentsOrPickerKeys(
			state,
			screen,
		) + ")",
	}
	if event == "refresh" {
		actions = append(
			actions,
			"refresh-preview",
		)
	}
	a.printLine(strings.Join(
		actions,
		"+",
	))
	return 0
}

// Prints fzf's first action when this g comes within a second of the one
// before, as _g DIR.
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

// Creates pane-closed in dir when it is missing and removes it otherwise,
// so the next layout closes or opens the side pane.
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
