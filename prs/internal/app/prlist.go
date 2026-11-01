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

// The PR list screen's bound letters, freed for typing while a search is
// open.
const listLetters = "j,k,g,G,l,d,c,o,r,h,q,J,K,/,?"

// The PR list screen's prompt, which a refresh adds the list's age to.
const listPrompt = "prs "

// Files the PR list screen keeps in its temporary folder.
const (
	// The refresher's state, such as "running", then the saved list's
	// time. It is gone while the list is fresh.
	refreshStateFile = "refresh"
	// The error from the last failed refresh, from gh or from saving the list.
	refreshErrorFile = "error"
	// Exists while the side pane shows that error.
	showingErrorFile = "showing-error"
	// The main prs process's ID, which the r key signals to refresh.
	pidFile = "pid"
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
	store, err := cache.Open()
	if err != nil {
		a.printError("prs: " + err.Error())
		return 1
	}
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
			list, err := store.ReadList()
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
				_ = store.WriteList(list)
				store.RemoveUnlisted(list.PullRequests)
			}
			_ = os.WriteFile(
				filepath.Join(
					dir,
					pidFile,
				),
				[]byte(strconv.Itoa(os.Getpid())),
				0o600,
			)
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
				store,
			)
			if saved {
				// The list's age shows from the first frame, since a refresh
				// starts at once.
				r.writeState(
					"running",
					list.FetchedAt,
				)
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
				"--preview="+a.self+" _meta "+quoteShell(dir)+" {1} {2}",
				"--preview-window=down,50%,border-top,wrap-word",
				"--bind=start:transform("+layoutCommand+" start)",
				"--bind=result:transform("+layoutCommand+" result)",
				"--bind=resize:transform("+layoutCommand+" resize)",
				// Each load, the refresher's reload included, sets the prompt
				// again, which drops the list's age after a refresh that works.
				"--bind=load:transform("+a.self+" _prompt "+quoteShell(dir)+")",
				"--bind=click-footer:transform("+a.self+" _tap "+quoteShell(dir)+")",
			)
			stop()
			if !ok {
				return 1
			}
			return 0
		},
	)
}

// Returns the fzf action of each key in the PR list screen. A tap on the
// help text row runs the same action as the key.
func (a *prsApp) buildListActions(dir string) map[string]string {
	summary := "execute(" + a.self + " _summary {1} {2})"
	return map[string]string{
		"l":            summary,
		"enter":        summary,
		"double-click": summary,
		"c":            "execute(" + a.self + " _comments {1} {2})",
		"d":            "execute(" + a.self + " _diff {1} {2})",
		"r":            "execute-silent(" + a.self + " _refresh " + quoteShell(dir) + ")",
		"o":            "transform(" + a.self + " _layout " + quoteShell(dir) + " toggle)",
		"g":            "transform(" + a.self + " _g " + quoteShell(dir) + ")",
		// h does nothing on the PR list screen, but stays bound so it does not
		// type into the query.
		"h": "ignore",
		"q": "abort",
		"/": buildSearchStart(listLetters),
		"?": "transform(" + a.self + " _help list)",
		"esc": "clear-query+rebind(" + listLetters + ")+change-prompt(" + listPrompt + ")+transform(" +
			a.self + " _prompt " + quoteShell(dir) + ")+transform(" +
			a.self + " _layout " + quoteShell(dir) + " result)",
	}
}

// Prints the saved list's rows, as _rows, for the refresher's reload.
func (a *prsApp) printListRows([]string) int {
	store, err := cache.Open()
	if err != nil {
		return 0
	}
	list, _ := store.ReadList()
	a.printText(view.RenderListRows(
		list.PullRequests,
		a.makeStyle(0),
	))
	return 0
}

// Prints the details from the saved list, as _meta DIR REPO NUMBER. It
// prints the 3-row status strip when the side pane has 3 rows or fewer,
// and the longer details otherwise, under gh's error after a failed
// refresh.
func (a *prsApp) printDetails(args []string) int {
	dir, repo, number := args[0], args[1], readNumber(args[2])
	detail := view.Long
	if lines, err := strconv.Atoi(os.Getenv("FZF_PREVIEW_LINES")); err == nil && lines <= 3 {
		detail = view.Strip
	}
	store, err := cache.Open()
	if err != nil {
		a.printLine(err.Error())
		return 0
	}
	pr, ok := store.FindPullRequest(
		repo,
		number,
	)
	if !ok {
		a.printLine("#" + strconv.Itoa(number) + " is not in the saved list. Press r to refresh it.")
		return 0
	}
	if failure, err := os.ReadFile(filepath.Join(
		dir,
		refreshErrorFile,
	)); err == nil && detail != view.Strip {
		a.printLine(a.paintRed("Could not refresh the list."))
		a.printText(string(failure))
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
	toggle := event == "toggle"
	if toggle {
		event = layout.Resize
	}
	errorFile := filepath.Join(
		dir,
		refreshErrorFile,
	)
	showingError := filepath.Join(
		dir,
		showingErrorFile,
	)
	if toggle {
		togglePaneClosed(dir)
	}
	state := layout.ReadFzfState()
	window, refresh := layout.ChooseListWindow(
		state,
		event,
		isPaneClosed(dir),
	)
	actions := []string{
		window,
		"change-footer(" + layout.BuildListKeys(
			state,
			isPresent(errorFile),
		) + ")",
	}
	// fzf skips a preview that reads a row, such as _meta DIR {1} {2}, when
	// the list has no rows. So after a failed refresh the side pane shows
	// the error file instead.
	switch {
	case state.Total == 0 && isPresent(errorFile):
		_ = os.WriteFile(
			showingError,
			nil,
			0o600,
		)
		actions = append(
			actions,
			"change-preview(cat "+quoteShell(errorFile)+")",
		)
		refresh = true
	case isPresent(showingError):
		_ = os.Remove(showingError)
		actions = append(
			actions,
			"change-preview("+a.self+" _meta "+quoteShell(dir)+" {1} {2})",
		)
		refresh = true
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

// Prints the fzf action for a tap on the PR list screen's help text row,
// as _tap DIR.
func (a *prsApp) printListTapAction(args []string) int {
	dir := args[0]
	keys := layout.BuildListKeys(
		layout.ReadFzfState(),
		isPresent(filepath.Join(
			dir,
			refreshErrorFile,
		)),
	)
	a.printTappedAction(
		keys,
		a.buildListActions(dir),
	)
	return 0
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

// Asks the main prs process to refresh the list, as _refresh DIR.
func (a *prsApp) requestListRefresh(args []string) int {
	saved, err := os.ReadFile(filepath.Join(
		args[0],
		pidFile,
	))
	if err != nil {
		return 0
	}
	if pid, err := strconv.Atoi(string(saved)); err == nil {
		_ = syscall.Kill(
			pid,
			syscall.SIGUSR1,
		)
	}
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
	_, stamp, _ := strings.Cut(
		string(saved),
		"\n",
	)
	fetched, err := time.Parse(
		time.RFC3339Nano,
		strings.TrimSpace(stamp),
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
