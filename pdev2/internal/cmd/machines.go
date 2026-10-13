package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
	"slices"
	"sync"
	"time"
)

// Each herdr --machine call starts its own SSH bridge. In live testing the
// first call to a machine took about 10 seconds and later ones 4.6 to 5.6,
// so a shorter limit would call a working machine unreachable. Tests
// shorten it.
var machineTimeout = 20 * time.Second

type savedMachine struct {
	ID      string `json:"id"`
	Label   string `json:"label"`
	Enabled bool   `json:"enabled"`
}

// savedMachines lists the enabled Herdr machines saved on this computer. It
// is empty when Herdr cannot list them.
func savedMachines() []savedMachine {
	output, err := herdrOutput("machine", "list", "--json")
	if err != nil {
		return nil
	}
	var all []savedMachine
	if json.Unmarshal(output, &all) != nil {
		return nil
	}
	var enabled []savedMachine
	for _, machine := range all {
		if machine.Enabled {
			enabled = append(enabled, machine)
		}
	}
	return enabled
}

// otherMachineLines checks every enabled saved Herdr machine at once and
// says what waits on each, such as "Wallace: not reachable", or
// "Wallace: 1 waiting in" followed by "  notes". It only reads Herdr's pane
// and workspace lists there; requests are still answered on the machine
// that runs gh.
func otherMachineLines() []string {
	machines := savedMachines()
	results := make([][]string, len(machines))
	var wg sync.WaitGroup
	for i, machine := range machines {
		wg.Add(1)
		go func(i int, machine savedMachine) {
			defer wg.Done()
			results[i] = machineLines(machine)
		}(i, machine)
	}
	wg.Wait()
	var lines []string
	for _, result := range results {
		lines = append(lines, result...)
	}
	return lines
}

// machineLines counts the panes that carry the approval marker on one
// machine and lists their workspaces, one per line so that none is cut at
// the popup's width. One pane can hold several requests, so it counts
// agents waiting, not requests.
func machineLines(machine savedMachine) []string {
	ctx, cancel := context.WithTimeout(context.Background(), machineTimeout)
	defer cancel()
	var workspaces []byte
	listed := make(chan struct{})
	// Each call takes seconds, so ask for workspace labels at the same time.
	go func() {
		defer close(listed)
		workspaces, _ = remoteHerdr(ctx, machine.ID, "workspace", "list")
	}()
	panes, err := remoteHerdr(ctx, machine.ID, "pane", "list")
	<-listed
	if err != nil {
		return []string{machine.Label + ": not reachable"}
	}
	var envelope struct {
		Result struct {
			Panes []struct {
				Workspace string            `json:"workspace_id"`
				Tokens    map[string]string `json:"tokens"`
			} `json:"panes"`
		} `json:"result"`
	}
	if json.Unmarshal(panes, &envelope) != nil {
		return []string{machine.Label + ": cannot read its panes"}
	}
	labels := map[string]string{}
	for _, item := range parseWorkspaces(workspaces) {
		labels[item.ID] = item.Label
	}
	waiting := 0
	var names []string
	for _, pane := range envelope.Result.Panes {
		if pane.Tokens[markerToken] == "" {
			continue
		}
		waiting++
		name := labels[pane.Workspace]
		if name == "" {
			name = pane.Workspace
		}
		if !slices.Contains(names, "  "+name) {
			names = append(names, "  "+name)
		}
	}
	if waiting == 0 {
		return []string{machine.Label + ": none waiting"}
	}
	return append([]string{fmt.Sprintf("%s: %d waiting in", machine.Label, waiting)}, names...)
}

// Closing the popup mid-check does not wait for these calls. herdr and its
// ssh child share the popup's terminal, so its hangup signal should end
// them; failing that, Herdr's ssh settings should: a 10-second connect
// timeout and keepalives that give up after about a minute.
func remoteHerdr(ctx context.Context, machine string, args ...string) ([]byte, error) {
	command := exec.CommandContext(ctx, "herdr", append([]string{"--machine", machine}, args...)...)
	// Killing herdr at the time limit can leave its ssh child holding the
	// output open; without this, Output would wait for that child.
	command.WaitDelay = time.Second
	return command.Output()
}
