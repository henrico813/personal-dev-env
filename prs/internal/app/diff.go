package app

import (
	"bytes"
	"context"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"strconv"
	"strings"

	"prs/internal/codewrap"
	"prs/internal/diffsplit"
	"prs/internal/github"
	"prs/internal/layout"
	"prs/internal/view"
)

// The diff view's own keys, besides h and q. r quits so prs can draw the
// diff again at a new width.
const diffViewKeys = "L quit L\nH quit H\no quit o\nr quit r\n? quit ?\n"

const diffViewHelp = `j k      scroll
d u J K  half page down, up
L H      next, previous file
o        open the file picker
r        redraw at this width
h q      back
g G      top, bottom
/        find; Esc cancels it
`

const filePickerHelp = `j k      choose a file
l Enter  open it in the diff view
o h      close the file picker
q        leave the diff screen
/        find; Esc cancels it
gg G     first, last
J K      scroll the diff
Tap a help text row word to run it.
Double tap a file to open it.
`

// Shows the diff screen, as _diff REPO NUMBER.
func (a *prsApp) openDiff(args []string) int {
	repo, number := args[0], readNumber(args[1])
	pr := github.PullRequest{
		Number:     number,
		Repository: github.Repository{NameWithOwner: repo},
	}
	if listed, ok := a.store.FindPullRequest(
		repo,
		number,
	); ok {
		pr = listed
	}
	a.showDiff(pr)
	return 0
}

// Shows the diff screen until h or q in the diff view, or q in the file
// picker. This one process owns the split diff and the current file, and
// swaps the diff view in less and the file picker in fzf so they look like
// one screen.
func (a *prsApp) showDiff(pr github.PullRequest) {
	number := strconv.Itoa(pr.Number)
	diff, err := a.loadDiff(pr)
	if err != nil {
		a.printError(err.Error())
		a.waitForKey("Could not get the diff of #" + number + ".")
		return
	}
	a.runInTempDir(
		nil,
		func(dir string) int {
			files, err := diffsplit.WriteFiles(
				dir,
				diff,
			)
			if err != nil {
				a.printError(err.Error())
				a.waitForKey("Could not split the diff of #" + number + ".")
				return 0
			}
			if len(files) == 0 {
				a.waitForKey(clearScreen + "#" + number + " changes no files.")
				return 0
			}
			current := 0
			for {
				switch a.showFileDiff(
					dir,
					files,
					current,
				) {
				case 'L':
					current = min(
						current+1,
						len(files)-1,
					)
				case 'H':
					current = max(
						current-1,
						0,
					)
				case 'o':
					key, chosen := a.pickFile(
						dir,
						number,
						files,
						current,
					)
					switch key {
					case "l":
						current = chosen
					case "o":
					default:
						return 0
					}
				case 'r':
				case '?':
					a.showHelpPage(diffViewHelp)
				default:
					return 0
				}
			}
		},
	)
}

// Returns the diff from the cache, or fetches and saves it with a loading
// line. A pull request without an update time is never cached, since its
// file name would hold the zero time and a newer diff could never replace it.
func (a *prsApp) loadDiff(pr github.PullRequest) (
	[]byte,
	error,
) {
	cached := !pr.UpdatedAt.IsZero()
	if cached {
		if diff, ok := a.store.ReadDiff(pr); ok {
			return diff, nil
		}
	}
	a.printLine(clearScreen + "Getting the diff of #" + strconv.Itoa(pr.Number))
	diff, err := github.FetchDiff(
		context.Background(),
		pr.Repo(),
		pr.Number,
	)
	if err == nil && cached {
		_ = a.store.WriteDiff(
			pr,
			diff,
		)
	}
	return diff, err
}

// Shows the current file full screen in less, as the diff view, and returns
// the key that closed it.
func (a *prsApp) showFileDiff(
	dir string,
	files []diffsplit.File,
	current int,
) byte {
	file := files[current]
	width := measureScreenWidth()
	return a.openInLess(
		fitLessPrompt(
			width,
			strconv.Itoa(file.Number)+"/"+strconv.Itoa(len(files))+" "+path.Base(file.Path),
			"o files  L next  d u page  h back  ? help",
			"o files  L next  h back  ? help",
		),
		a.renderFileDiff(
			dir,
			file.Number,
			width,
		),
		diffViewKeys,
	)
}

// Shows the file picker on the current file, with its diff in the side
// pane. Returns the key that closed it, l, o, q or "" for Ctrl-C, which the
// caller treats as q, and the index of the file selected then.
func (a *prsApp) pickFile(
	dir,
	number string,
	files []diffsplit.File,
	current int,
) (
	string,
	int,
) {
	layoutCommand := a.self + " _pane_layout picker " + quoteShell(dir)
	// --freeze-left keeps the counts in view, and --keep-right keeps a long
	// path's file name in view while the query is empty. fzf drops the
	// space after the counts when it cuts a path, so the ellipsis brings it
	// back.
	out, _ := a.runFzf(
		view.RenderFileRows(
			files,
			a.makeStyle(0),
		),
		a.buildFilePickerActions(
			dir,
			number,
		),
		"--delimiter=\\t",
		"--with-nth=2..",
		"--freeze-left=1",
		"--keep-right",
		"--tabstop=1",
		"--ellipsis= …",
		"--prompt="+buildFilePickerPrompt(number),
		"--footer="+layout.FilePickerKeys,
		"--preview="+a.self+" _file "+quoteShell(dir)+" {1}",
		"--preview-window=down,50%,border-top,nowrap",
		// pos waits for load, since at start fzf may not have read the rows.
		"--bind=load:pos("+strconv.Itoa(current+1)+")",
		"--bind=start,result:transform("+layoutCommand+" result)",
		"--bind=resize:transform("+layoutCommand+" refresh)",
		"--bind=click-footer:transform("+a.self+" _tap picker "+quoteShell(dir)+")",
	)
	// fzf prints the key's name, then the selected row, whose first field
	// is the file's number.
	key, row, _ := strings.Cut(
		out,
		"\n",
	)
	field, _, _ := strings.Cut(
		row,
		"\t",
	)
	chosen, err := strconv.Atoi(field)
	if err != nil || chosen < 1 || chosen > len(files) {
		return key, current
	}
	return key, chosen - 1
}

// Returns the fzf action of each key in the file picker. Each way out
// prints its key first, so the diff screen knows what to do next.
func (a *prsApp) buildFilePickerActions(
	dir,
	number string,
) map[string]string {
	return addSearchKeys(
		map[string]string{
			"l":            "print(l)+accept",
			"enter":        "print(l)+accept",
			"double-click": "print(l)+accept",
			"o":            "print(o)+accept",
			"h":            "print(o)+accept",
			"q":            "print(q)+accept",
			"g":            "transform(" + a.self + " _g " + quoteShell(dir) + ")",
			"?":            "transform(" + a.self + " _help picker)",
		},
		buildFilePickerPrompt(number),
		"transform("+a.self+" _pane_layout picker "+quoteShell(dir)+" result)",
	)
}

func buildFilePickerPrompt(number string) string {
	return "#" + number + " files "
}

// Prints file N of the split diff, as _file DIR N, for the side pane.
func (a *prsApp) printFileDiff(args []string) int {
	a.printText(a.renderFileDiff(
		args[0],
		readNumber(args[1]),
		readPreviewWidth(),
	))
	return 0
}

// Returns file N of the split diff in dir at width columns, colored by
// delta. With NO_COLOR, or when delta fails, it returns the diff as git
// wrote it, rewrapped after the +/- mark and never side by side.
func (a *prsApp) renderFileDiff(
	dir string,
	n,
	width int,
) string {
	diff, err := os.ReadFile(filepath.Join(
		dir,
		strconv.Itoa(n)+".diff",
	))
	if err != nil {
		return ""
	}
	if a.color {
		if colored, err := runDelta(
			diff,
			width,
		); err == nil {
			return colored
		}
	}
	return codewrap.Wrap(
		string(diff),
		width,
		1,
	)
}

// Returns delta's colored diff, side by side from 140 columns and
// otherwise narrow and rewrapped under the code. delta runs without git
// config, so a git setting such as side-by-side cannot change the layout.
func runDelta(
	diff []byte,
	width int,
) (
	string,
	error,
) {
	// Headers without boxes wrap cleanly.
	args := []string{
		"--no-gitconfig",
		"--paging=never",
		"--width=" + strconv.Itoa(width),
		"--tabs=2",
		"--hunk-header-decoration-style=none",
		"--file-decoration-style=none",
	}
	if width >= 140 {
		args = append(
			args,
			"--side-by-side",
			"--wrap-max-lines=unlimited",
		)
	} else {
		// Each row gets one 5-column line number, then the +/- mark. Hunk
		// headers drop their line number, which reads 0 for a deleted file.
		args = append(
			args,
			"--line-numbers",
			"--line-numbers-left-format=",
			"--line-numbers-right-format={np:>4} ",
			"--keep-plus-minus-markers",
			"--hunk-header-style=syntax",
		)
	}
	cmd := exec.Command(
		"delta",
		args...,
	)
	cmd.Stdin = bytes.NewReader(diff)
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	// The empty first line goes, since a phone has few rows to spare.
	text := strings.TrimPrefix(
		string(out),
		"\n",
	)
	if width >= 140 {
		return text, nil
	}
	return codewrap.Wrap(
		text,
		width,
		6,
	), nil
}
