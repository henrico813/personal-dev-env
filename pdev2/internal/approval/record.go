package approval

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
// the PID of a phone question or runner is not a change; see SaveKeepingAge.
const Expiry = 4 * time.Hour

// An answer from the terminal, the Herdr popup, or the phone moves a record
// from pending to approved or declined. pde-gh-write, or the request's runner,
// moves an approved record to running before gh starts and to ran after it
// exits. Finding running under the lock means that run died, and it is never
// retried.
const (
	StatePending  = "pending"
	StateApproved = "approved"
	StateDeclined = "declined"
	StateRunning  = "running"
	StateRan      = "ran"
)

// Answers from the popup, the terminal prompt, and the phone. ChoiceNone means
// no answer yet; ChoiceDismiss and ChoiceFocus close the popup without one.
const (
	ChoiceNone    = ""
	ChoiceYes     = "yes"
	ChoiceNo      = "no"
	ChoiceDismiss = "dismiss"
	ChoiceFocus   = "focus"
)

// Record is a request saved as JSON in $XDG_STATE_HOME/pde/pr-write/ID.json.
// Renaming a field or its JSON tag loses requests saved by an older build.
type Record struct {
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
	PaneID       string `json:"pane_id,omitempty"`   // For g and the sidebar marker.
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

func Load(id string) (Record, error) {
	var rec Record
	data, err := os.ReadFile(recordPath(id))
	if err != nil {
		return rec, err
	}
	err = json.Unmarshal(data, &rec)
	return rec, err
}

// Save records a change and restarts the four-hour expiry.
func Save(rec Record) error {
	rec.Updated = time.Now()
	return Write(rec)
}

// SaveKeepingAge saves a change that is not an answer, such as the PID of a
// phone question or runner, without restarting the four-hour expiry.
// Otherwise each rerun that replaced a failed phone question or runner would
// restart expiry, and the request would never expire. The caller holds the
// request lock.
func SaveKeepingAge(rec Record) error {
	stored, err := Load(rec.ID)
	if err != nil {
		return err
	}
	rec.Updated = stored.Updated
	return Write(rec)
}

// Write saves rec with its Updated time as given; Save is for a change.
// Replacing the completed temporary file keeps readers from seeing a partial record.
func Write(rec Record) error {
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

// Lock waits for the request's lock and holds it until the file is closed.
func Lock(id string) (*os.File, error) {
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

func Expired(rec Record) bool {
	return time.Since(rec.Updated) > Expiry
}

// Queued returns the pending requests the popup visits, oldest first.
func Queued() []Record { return pendingRecords(false) }

// Inbox also includes dismissed requests, which the inbox still lists.
func Inbox() []Record { return pendingRecords(true) }

// Oldest first; Updated moves forward when a record changes, so it cannot
// order the queue.
func pendingRecords(includeDismissed bool) []Record {
	entries, err := os.ReadDir(stateDir())
	if err != nil {
		return nil
	}
	var records []Record
	for _, entry := range entries {
		if filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		rec, err := Load(strings.TrimSuffix(entry.Name(), ".json"))
		if err == nil && rec.State == StatePending && !Expired(rec) && (includeDismissed || !rec.Dismissed) {
			records = append(records, rec)
		}
	}
	slices.SortFunc(records, func(a, b Record) int { return a.Created.Compare(b.Created) })
	return records
}
