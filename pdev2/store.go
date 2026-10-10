package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"syscall"
	"time"
)

// Expiry is measured from the record's last change, not its creation. Saving
// the PID of a phone ask or runner is not a change; see saveKeepingAge.
const expiry = 4 * time.Hour

// An answer from the terminal, the Herdr popup, or the phone moves a record
// from pending to approved or declined. pde-gh-write moves an approved record
// to running before gh starts and to ran after it exits. Finding running under
// the lock means that run died, and it is never retried.
const (
	statePending  = "pending"
	stateApproved = "approved"
	stateDeclined = "declined"
	stateRunning  = "running"
	stateRan      = "ran"
)

type record struct {
	// The request as the agent made it.
	ID           string    `json:"id"`
	Args         []string  `json:"args"`
	Title        string    `json:"title"`
	Body         string    `json:"body"`
	BodyFile     bool      `json:"body_file"`
	ApprovalText string    `json:"approval_text"`
	Directory    string    `json:"directory,omitempty"`
	Created      time.Time `json:"created"`
	// The real gh found when the request was created. The detached runner
	// starts from a popup or phone environment whose PATH may lack it.
	GH string `json:"gh,omitempty"`

	// Its state and result.
	Updated   time.Time `json:"updated"`
	State     string    `json:"state"`
	Exit      int       `json:"exit_code"`
	Output    string    `json:"output,omitempty"`
	MoshiPID  int       `json:"moshi_pid,omitempty"`
	RunnerPID int       `json:"runner_pid,omitempty"`
	Dismissed bool      `json:"dismissed,omitempty"`

	// Context shown to the approver.
	Repo         string `json:"repo,omitempty"`
	PRTitle      string `json:"pr_title,omitempty"` // Looked up when the request sets no title.
	SessionID    string `json:"session_id,omitempty"`
	SessionTitle string `json:"session_title,omitempty"`
	Workspace    string `json:"workspace,omitempty"` // Herdr label, or the ID when unlabeled.
	PaneID       string `json:"pane_id,omitempty"`
}

func stateDir() string {
	root := os.Getenv("XDG_STATE_HOME")
	if root == "" {
		home, _ := os.UserHomeDir()
		root = filepath.Join(home, ".local", "state")
	}
	return filepath.Join(root, "pde", "pr-write")
}

func recordPath(id string) string {
	return filepath.Join(stateDir(), id+".json")
}

func load(id string) (record, error) {
	var rec record
	data, err := os.ReadFile(recordPath(id))
	if err != nil {
		return rec, err
	}
	err = json.Unmarshal(data, &rec)
	return rec, err
}

// save records a change and restarts the four-hour expiry.
func save(rec record) error {
	rec.Updated = time.Now()
	return writeRecord(rec)
}

// saveKeepingAge saves bookkeeping, such as the PID of a phone ask or runner,
// with the stored record's age. Otherwise an agent rerunning a request whose
// ask or runner keeps dying would restart expiry each time, and the request
// would never expire. The caller holds the request lock.
func saveKeepingAge(rec record) error {
	stored, err := load(rec.ID)
	if err != nil {
		return err
	}
	rec.Updated = stored.Updated
	return writeRecord(rec)
}

// Replacing the completed temporary file keeps readers from seeing a partial record.
func writeRecord(rec record) error {
	dir := stateDir()
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return err
	}
	data, err := json.Marshal(rec)
	if err != nil {
		return err
	}
	file, err := os.CreateTemp(dir, ".record-")
	if err != nil {
		return err
	}
	defer os.Remove(file.Name())
	if _, err := file.Write(append(data, '\n')); err != nil {
		file.Close()
		return err
	}
	if err := file.Close(); err != nil {
		return err
	}
	return os.Rename(file.Name(), recordPath(rec.ID))
}

func lockRequest(id string) (*os.File, error) {
	if err := os.MkdirAll(stateDir(), 0o700); err != nil {
		return nil, err
	}
	file, err := os.OpenFile(recordPath(id)+".lock", os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, err
	}
	if err := syscall.Flock(int(file.Fd()), syscall.LOCK_EX); err != nil {
		file.Close()
		return nil, err
	}
	return file, nil
}

func expired(rec record) bool {
	return time.Since(rec.Updated) > expiry
}

// queuedRecords returns the pending requests the popup visits, oldest first.
func queuedRecords() []record { return pendingRecords(false) }

// inboxRecords also includes dismissed requests, which the inbox still lists.
func inboxRecords() []record { return pendingRecords(true) }

// Oldest first; Updated moves forward when a record changes, so it cannot
// order the queue.
func pendingRecords(includeDismissed bool) []record {
	entries, err := os.ReadDir(stateDir())
	if err != nil {
		return nil
	}
	var records []record
	for _, entry := range entries {
		if filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		rec, err := load(strings.TrimSuffix(entry.Name(), ".json"))
		if err == nil && rec.State == statePending && !expired(rec) && (includeDismissed || !rec.Dismissed) {
			records = append(records, rec)
		}
	}
	slices.SortFunc(records, func(a, b record) int { return a.Created.Compare(b.Created) })
	return records
}
