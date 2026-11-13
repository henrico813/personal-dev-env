package installer

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"pde-installer/internal/run"
)

// One line of update state. Doctor prints a line that needs attention as a
// warning.
type updateStateLine struct {
	text      string
	attention bool
}

// Returns the last update line and the main line that version and doctor
// print. Root is the checkout, or "" when none is known.
func readUpdateState(home, root string, runner run.Runner) []updateStateLine {
	return []updateStateLine{
		describeLastUpdate(home),
		describeMainLead(
			home,
			root,
			runner,
		),
	}
}

// Returns the path of update.json, which update writes after every run.
func updateRecordPath(home string) string {
	return filepath.Join(
		home,
		".local",
		"state",
		"pde",
		"update.json",
	)
}

// Returns the result, time and reason that the last update saved.
func describeLastUpdate(home string) updateStateLine {
	data, err := os.ReadFile(updateRecordPath(home))
	if errors.Is(
		err,
		fs.ErrNotExist,
	) {
		return updateStateLine{text: "no update recorded"}
	}
	var record updateRecord
	if err == nil {
		err = json.Unmarshal(
			data,
			&record,
		)
	}
	if err != nil {
		return updateStateLine{
			text:      "last update unreadable: " + err.Error(),
			attention: true,
		}
	}
	text := fmt.Sprintf(
		"last update %s at %s",
		record.Result,
		record.CheckedAt,
	)
	if record.Reason != "" {
		text += ": " + record.Reason
	}
	return updateStateLine{
		text:      text,
		attention: record.Result == updateSkipped || record.Result == updateFailed,
	}
}

// Returns how many commits the checkout's local main has that the installed
// commit lacks. It fetches nothing, so it sees only what update or a pull
// already brought into main.
func describeMainLead(home, root string, runner run.Runner) updateStateLine {
	if root == "" {
		return updateStateLine{text: "main not compared: no checkout found"}
	}
	record, err := readInstallRecord(home)
	if errors.Is(
		err,
		fs.ErrNotExist,
	) {
		return updateStateLine{text: "main not compared: no install recorded"}
	}
	if err != nil {
		return updateStateLine{text: "main not compared: " + err.Error()}
	}
	output, err := runner.Query(
		"count commits main has beyond the install",
		makeGitCommand(
			root,
			"rev-list",
			"--count",
			record.Commit+"..refs/heads/main",
		),
	)
	if err != nil {
		return updateStateLine{text: "main not compared: " + err.Error()}
	}
	count := strings.TrimSpace(string(output))
	noun := "commits"
	if count == "1" {
		noun = "commit"
	}
	return updateStateLine{
		text: fmt.Sprintf(
			"main is %s %s ahead of the install",
			count,
			noun,
		),
		attention: count != "0",
	}
}
