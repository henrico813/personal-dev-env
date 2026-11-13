package installer

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"pde-installer/internal/run"
)

const hubWatchUnit = "hub-watch.service"

// A process of the current user that runs a deleted binary from HOME.
type staleProcess struct {
	pid  int
	name string
}

// Restarts hub-watch.service when it still runs a hub binary that an install
// replaced, and warns about other processes on deleted binaries from HOME.
// Failures only warn, because every component is already in place.
func refreshStaleProcesses(home string, runner run.Runner) {
	if runner.DryRun {
		_ = runner.Plan(
			"restart "+hubWatchUnit+" if it runs a replaced hub",
			nil,
		)
		return
	}
	stale, err := findStaleProcesses(home)
	if err != nil {
		fmt.Fprintf(
			runner.Err(),
			"warning: look for processes on replaced binaries: %v\n",
			err,
		)
		return
	}
	if len(stale) == 0 {
		return
	}
	watchPID := readHubWatchPID(runner)
	for _, process := range stale {
		if process.pid != watchPID {
			fmt.Fprintf(
				runner.Err(),
				"warning: PID %d (%s) runs a deleted binary from HOME; restart it to use the installed build\n",
				process.pid,
				process.name,
			)
			continue
		}
		if err := runner.Run(
			"restart "+hubWatchUnit,
			run.Command{
				Name: "systemctl",
				Args: []string{
					"--user",
					"try-restart",
					hubWatchUnit,
				},
			},
		); err != nil {
			fmt.Fprintf(
				runner.Err(),
				"warning: %v\n",
				err,
			)
			continue
		}
		fmt.Fprintf(
			runner.Out(),
			"Restarted %s so it runs the new hub.\n",
			hubWatchUnit,
		)
	}
}

// Returns the current user's processes whose binary was a file under home
// that is now deleted, in PID order.
func findStaleProcesses(home string) ([]staleProcess, error) {
	entries, err := os.ReadDir("/proc")
	if err != nil {
		return nil, fmt.Errorf(
			"scan /proc: %w",
			err,
		)
	}
	var stale []staleProcess
	for _, entry := range entries {
		pid, err := strconv.Atoi(entry.Name())
		if err != nil || !processOwnedByCurrentUser(pid) {
			continue
		}
		executable, err := os.Readlink(filepath.Join(
			"/proc",
			entry.Name(),
			"exe",
		))
		path, deleted := strings.CutSuffix(
			executable,
			" (deleted)",
		)
		if err != nil || !deleted || !within(home, path) {
			continue
		}
		name, _ := os.ReadFile(filepath.Join(
			"/proc",
			entry.Name(),
			"comm",
		))
		stale = append(
			stale,
			staleProcess{
				pid:  pid,
				name: strings.TrimSpace(string(name)),
			},
		)
	}
	sort.Slice(stale, func(i, j int) bool { return stale[i].pid < stale[j].pid })
	return stale, nil
}

// Returns the PID of hub-watch.service's main process, or 0 when it is not
// running or this machine has no systemd user manager.
func readHubWatchPID(runner run.Runner) int {
	output, err := runner.Query(
		"read "+hubWatchUnit+" PID",
		run.Command{
			Name: "systemctl",
			Args: []string{
				"--user",
				"show",
				"--property=MainPID",
				"--value",
				hubWatchUnit,
			},
		},
	)
	if err != nil {
		return 0
	}
	pid, _ := strconv.Atoi(strings.TrimSpace(string(output)))
	return pid
}
