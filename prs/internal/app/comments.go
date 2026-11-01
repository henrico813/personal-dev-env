package app

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"prs/internal/codewrap"
	"prs/internal/github"
	"prs/internal/layout"
	"prs/internal/view"
)

// The comments screen's bound letters, freed for typing while a search is
// open.
const commentLetters = "j,k,g,G,l,o,h,q,J,K,/,?"

// The comments, reviews and code threads, saved in the comments screen's
// temporary folder for the side pane to read.
const feedbackFile = "feedback.json"

const commentsHelp = `l Enter  read it full screen
o        show or hide the side pane
h q      back
/        find; Esc cancels it
j k      move     gg G  first, last
J K      scroll the side pane
Tap a help text row word to run it.
Double tap a row to read it.
`

// Shows the comments screen, as _comments REPO NUMBER.
func (a *prsApp) openComments(args []string) int {
	repo, number := args[0], readNumber(args[1])
	pr, extras, err := a.loadPullRequest(
		repo,
		number,
	)
	if err != nil {
		a.printError(err.Error())
		a.waitForKey("Could not get the comments of #" + strconv.Itoa(number) + ".")
		return 0
	}
	a.showComments(
		pr,
		extras.Feedback,
	)
	return 0
}

// Shows the comments screen, one row per conversation post and code
// thread with the selected one in the side pane, until h or q.
func (a *prsApp) showComments(
	pr github.PullRequest,
	feedback github.Feedback,
) {
	number := strconv.Itoa(pr.Number)
	items := view.CollectItems(feedback)
	if len(items) == 0 {
		a.waitForKey(clearScreen + "#" + number + " has no comments.")
		return
	}
	a.runInTempDir(
		nil,
		func(dir string) int {
			data, err := json.Marshal(feedback)
			if err == nil {
				err = os.WriteFile(
					filepath.Join(
						dir,
						feedbackFile,
					),
					data,
					0o600,
				)
			}
			if err != nil {
				a.printError(err.Error())
				a.waitForKey("Could not save the comments of #" + number + ".")
				return 0
			}
			layoutCommand := a.self + " _comments_layout " + quoteShell(dir)
			a.runFzf(
				view.RenderItemRows(
					items,
					a.makeStyle(0),
				),
				a.buildCommentActions(
					dir,
					number,
				),
				"--delimiter=\\t",
				"--with-nth=2",
				"--no-hscroll",
				"--prompt="+buildCommentsPrompt(number),
				"--footer="+layout.CommentsKeys,
				"--preview="+a.self+" _item "+quoteShell(dir)+" {1}",
				"--preview-window=down,50%,border-top,nowrap",
				"--bind=start,result:transform("+layoutCommand+")",
				"--bind=resize:transform("+layoutCommand+" refresh)",
				"--bind=click-footer:transform("+a.self+" _comments_tap "+quoteShell(dir)+" "+number+")",
			)
			return 0
		},
	)
}

// Returns the fzf action of each key in the comments screen.
func (a *prsApp) buildCommentActions(
	dir,
	number string,
) map[string]string {
	open := "execute(" + a.self + " _item_page " + quoteShell(dir) + " {1})"
	return map[string]string{
		"l":            open,
		"enter":        open,
		"double-click": open,
		"o":            "transform(" + a.self + " _comments_layout " + quoteShell(dir) + " toggle)",
		"g":            "transform(" + a.self + " _g " + quoteShell(dir) + ")",
		"h":            "abort",
		"q":            "abort",
		"/":            buildSearchStart(commentLetters),
		"?":            "transform(" + a.self + " _help comments)",
		"esc": "clear-query+rebind(" + commentLetters + ")+change-prompt(" + buildCommentsPrompt(number) + ")+transform(" +
			a.self + " _comments_layout " + quoteShell(dir) + ")",
	}
}

// Prints fzf actions that fit the comments screen to the screen, as
// _comments_layout DIR [toggle|refresh].
func (a *prsApp) printCommentsLayout(args []string) int {
	if args[1] == "toggle" {
		togglePaneClosed(args[0])
	}
	state := layout.ReadFzfState()
	actions := []string{
		layout.ChoosePaneWindow(
			state,
			layout.CommentsScreen,
			isPaneClosed(args[0]),
		),
		"change-footer(" + layout.BuildCommentsKeys(state) + ")",
	}
	if args[1] == "refresh" {
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

// Prints the fzf action for a tap on the comments screen's help text row,
// as _comments_tap DIR NUMBER.
func (a *prsApp) printCommentsTapAction(args []string) int {
	a.printTappedAction(
		layout.BuildCommentsKeys(layout.ReadFzfState()),
		a.buildCommentActions(
			args[0],
			args[1],
		),
	)
	return 0
}

func buildCommentsPrompt(number string) string {
	return "#" + number + " comments "
}

// Prints row N in full for the side pane, as _item DIR N.
func (a *prsApp) printItem(args []string) int {
	width := readPreviewWidth()
	a.printText(codewrap.Wrap(
		a.renderItem(
			args[0],
			args[1],
			width,
		),
		width,
		0,
	))
	return 0
}

// Shows row N full screen in less, as _item_page DIR N.
func (a *prsApp) openItem(args []string) int {
	width := measureScreenWidth()
	a.openInLess(
		fitLessPrompt(
			width,
			"",
			"h back  / find",
			"h back",
		),
		codewrap.Wrap(
			a.renderItem(
				args[0],
				args[1],
				width,
			),
			width,
			0,
		),
		"",
	)
	return 0
}

// Returns row N of the comments saved in dir, wrapped to width.
func (a *prsApp) renderItem(
	dir,
	n string,
	width int,
) string {
	data, err := os.ReadFile(filepath.Join(
		dir,
		feedbackFile,
	))
	if err != nil {
		return err.Error() + "\n"
	}
	var feedback github.Feedback
	if err := json.Unmarshal(
		data,
		&feedback,
	); err != nil {
		return err.Error() + "\n"
	}
	items := view.CollectItems(feedback)
	index, err := strconv.Atoi(n)
	if err != nil || index < 0 || index >= len(items) {
		return ""
	}
	return view.RenderItem(
		items[index],
		a.makeStyle(width),
	)
}
