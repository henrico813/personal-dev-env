package app

import (
	"context"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	"prs/internal/cache"
	"prs/internal/github"
	"prs/internal/layout"
	"prs/internal/view"
)

// The PR list screen's prompt, which a refresh adds the list's age to.
const listPrompt = "prs "

// Files the PR list screen keeps in its temporary folder.
const (
	// The time of the list on screen while a refresh runs or after one
	// failed, which the prompt shows as the list's age.
	refreshStateFile = "refresh"
	// The error from the last failed refresh, from gh or from saving the list.
	refreshErrorFile = "error"
	// fzf's socket, which the refresher sends actions to.
	socketFile = "fzf.sock"
)

const listHelp = `l Enter  PR summary screen
c        comments screen
d        diff screen
r        refresh the list
o        show or hide the side pane
q        quit
/        find; Esc cancels it
j k      move     gg G  first, last
J K      scroll the side pane
Tap a help text row word to run it.
Double tap a row to open it.
`

// Shows the PR list screen, from the saved list at once while it
// refreshes in place. Without a saved list it fetches before fzf starts,
// so a GitHub failure shows as an error rather than as an empty list.
func (a *prsApp) pickPullRequest() int {
	ctx, cancel := context.WithCancel(context.Background())
	// One count covers the first fetch and then the refresher, so stop
	// waits for any gh that either one started.
	var background sync.WaitGroup
	background.Add(1)
	stop := func() {
		cancel()
		background.Wait()
	}
	return a.runInTempDir(
		stop,
		func(dir string) int {
			list, err := a.store.ReadList()
			saved := err == nil
			if !saved {
				a.printLine("Getting your pull requests")
				list, err = fetchList(ctx)
				if err != nil {
					background.Done()
					a.printError(err.Error())
					a.printError("Could not get your pull requests from GitHub.")
					return 1
				}
				_ = a.store.WriteList(list)
				a.store.RemoveUnlisted(list.PullRequests)
			}
			requests := make(
				chan os.Signal,
				1,
			)
			signal.Notify(
				requests,
				syscall.SIGUSR1,
			)
			defer signal.Stop(requests)
			r := newRefresher(
				a.self,
				dir,
				a.store,
			)
			if saved {
				// The list's age shows from the first frame, since a refresh
				// starts at once.
				r.writeState(list.FetchedAt)
			}
			go func() {
				defer background.Done()
				r.run(
					ctx,
					list,
					saved,
					requests,
				)
			}()
			layoutCommand := a.self + " _layout " + quoteShell(dir)
			// --no-hscroll keeps the PR number in view when a search matches
			// further along the row. --id-nth keeps the cursor on the same PR
			// when a refresh reloads the list.
			_, ok := a.runFzf(
				view.RenderListRows(
					list.PullRequests,
					a.makeStyle(0),
				),
				a.buildListActions(dir),
				"--delimiter=\\t",
				"--with-nth=3",
				"--id-nth=1,2",
				"--no-hscroll",
				"--listen="+filepath.Join(
					dir,
					socketFile,
				),
				"--prompt="+buildListPrompt(
					dir,
					time.Now(),
				),
				"--footer="+layout.ListKeys,
				// The side pane's command names no field such as {1}, so fzf
				// runs it on an empty list too.
				"--preview="+a.self+" _meta "+quoteShell(dir),
				"--preview-window=down,50%,border-top,wrap-word",
				"--bind=start:transform("+layoutCommand+" start)",
				"--bind=result:transform("+layoutCommand+" result)",
				"--bind=resize:transform("+layoutCommand+" resize)",
				// Each load, the refresher's reload included, sets the prompt
				// again, which drops the list's age after a refresh that works.
				"--bind=load:transform("+a.self+" _prompt "+quoteShell(dir)+")",
				"--bind=click-footer:transform("+a.self+" _tap list "+quoteShell(dir)+")",
			)
			stop()
			if !ok {
				return 1
			}
			return 0
		},
	)
}

func (a *prsApp) buildListActions(dir string) map[string]string {
	summary := "execute(" + a.self + " _summary {1} {2})"
	return addSearchKeys(
		map[string]string{
			"l":            summary,
			"enter":        summary,
			"double-click": summary,
			"c":            "execute(" + a.self + " _comments {1} {2})",
			"d":            "execute(" + a.self + " _diff {1} {2})",
			// Only the main prs process may build this table, because r
			// signals os.Getpid().
			"r": "execute-silent(kill -USR1 " + strconv.Itoa(os.Getpid()) + ")",
			"o": "transform(" + a.self + " _layout " + quoteShell(dir) + " toggle)",
			"g": "transform(" + a.self + " _g " + quoteShell(dir) + ")",
			// h does nothing on the PR list screen, but stays bound so it does
			// not type into the query.
			"h": "ignore",
			"q": "abort",
			"?": "transform(" + a.self + " _help list)",
		},
		listPrompt,
		"transform("+a.self+" _prompt "+quoteShell(dir)+")+transform("+a.self+" _layout "+quoteShell(dir)+" result)",
	)
}

// Prints the saved list's rows, as _rows, for the refresher's reload.
func (a *prsApp) printListRows([]string) int {
	list, _ := a.store.ReadList()
	a.printText(view.RenderListRows(
		list.PullRequests,
		a.makeStyle(0),
	))
	return 0
}

// Prints the selected pull request's details from the saved list, as
// _meta DIR, under the error of a failed refresh. A side pane of 3 rows or
// fewer gets the 3-row status strip, without the error.
func (a *prsApp) printDetails(args []string) int {
	dir := args[0]
	detail := view.Long
	if lines, err := strconv.Atoi(os.Getenv("FZF_PREVIEW_LINES")); err == nil && lines <= 3 {
		detail = view.Strip
	}
	if failure, err := os.ReadFile(filepath.Join(
		dir,
		refreshErrorFile,
	)); err == nil && detail != view.Strip {
		a.printLine(a.paintRed("Could not refresh the list."))
		a.printText(string(failure))
	}
	// fzf sets FZF_CURRENT_ITEM only while a row is selected, and runFzf
	// keeps an older one out of fzf's environment.
	item, ok := os.LookupEnv("FZF_CURRENT_ITEM")
	if !ok {
		return 0
	}
	fields := strings.SplitN(
		item,
		"\t",
		3,
	)
	if len(fields) < 2 {
		return 0
	}
	number := readNumber(fields[1])
	pr, found := a.store.FindPullRequest(
		fields[0],
		number,
	)
	if !found {
		a.printLine("#" + strconv.Itoa(number) + " is not in the saved list. Press r to refresh it.")
		return 0
	}
	a.printText(view.RenderDetails(
		pr,
		detail,
		a.makeStyle(readPreviewWidth()),
	))
	return 0
}

// Prints fzf actions that fit the PR list screen to the screen, as
// _layout DIR EVENT. The toggle event opens or closes the side pane.
func (a *prsApp) printListLayout(args []string) int {
	dir, event := args[0], layout.Event(args[1])
	if event == "toggle" {
		togglePaneClosed(dir)
		event = layout.Resize
	}
	state := layout.ReadFzfState()
	window, refresh := layout.ChooseListWindow(
		state,
		event,
		isPaneClosed(dir),
	)
	actions := []string{
		window,
		"change-footer(" + buildListKeys(
			state,
			dir,
		) + ")",
	}
	if refresh {
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

// Returns the PR list screen's help text row for fzf's width and whether
// the last refresh failed.
func buildListKeys(
	state layout.FzfState,
	dir string,
) string {
	return layout.BuildListKeys(
		state,
		isPresent(filepath.Join(
			dir,
			refreshErrorFile,
		)),
	)
}

// Prints the action that sets the prompt, as _prompt DIR. While a search
// is open the prompt belongs to the search, so it prints nothing.
func (a *prsApp) printListPrompt(args []string) int {
	if layout.ReadFzfState().IsSearching() {
		return 0
	}
	a.printLine("change-prompt(" + buildListPrompt(
		args[0],
		a.now,
	) + ")")
	return 0
}

// Returns the prompt, which names the list's age, such as "updated 5s
// ago", while a refresh runs or after one failed.
func buildListPrompt(
	dir string,
	now time.Time,
) string {
	saved, err := os.ReadFile(filepath.Join(
		dir,
		refreshStateFile,
	))
	if err != nil {
		return listPrompt
	}
	fetched, err := time.Parse(
		time.RFC3339Nano,
		strings.TrimSpace(string(saved)),
	)
	if err != nil {
		return listPrompt
	}
	return listPrompt + "updated " + formatShortAge(now.Sub(fetched)) + " ago "
}

// Returns an age such as 5s, 3m, 2h or 4d.
func formatShortAge(age time.Duration) string {
	switch {
	case age < time.Minute:
		return strconv.Itoa(int(age.Seconds())) + "s"
	case age < time.Hour:
		return strconv.Itoa(int(age.Minutes())) + "m"
	case age < 24*time.Hour:
		return strconv.Itoa(int(age.Hours())) + "h"
	}
	return strconv.Itoa(int(age.Hours()/24)) + "d"
}

// Returns the PR list from GitHub, stamped with the time it came.
func fetchList(ctx context.Context) (
	cache.List,
	error,
) {
	prs, err := github.ListPullRequests(ctx)
	return cache.List{
		FetchedAt:    time.Now(),
		PullRequests: prs,
	}, err
}

func (a *prsApp) paintRed(text string) string {
	if !a.color {
		return text
	}
	return "\x1b[31m" + text + "\x1b[0m"
}
