package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"syscall"
	"time"
)

// Expiry is measured from the record's last change, not its creation.
const expiry = 4 * time.Hour

// pde-pr-approve moves a record from pending to approved or declined.
// pde-gh-write moves an approved record to running before gh starts and to ran
// after it exits. Finding running under the lock means that run died, and it
// is never retried.
const (
	statePending  = "pending"
	stateApproved = "approved"
	stateDeclined = "declined"
	stateRunning  = "running"
	stateRan      = "ran"
)

type record struct {
	ID           string    `json:"id"`
	Updated      time.Time `json:"updated"`
	Args         []string  `json:"args"`
	Title        string    `json:"title"`
	Body         string    `json:"body"`
	BodyFile     bool      `json:"body_file"`
	ApprovalText string    `json:"approval_text"`
	State        string    `json:"state"`
	Exit         int       `json:"exit_code"`
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

// Replacing the completed temporary file keeps readers from seeing a partial record.
func save(rec record) error {
	rec.Updated = time.Now()
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
