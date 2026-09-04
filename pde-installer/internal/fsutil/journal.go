package fsutil

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const journalVersion = 1

// JournalConfig names the paths used for durable filesystem journals.
type JournalConfig struct {
	Home      string
	Directory string
}

// Change records enough activation state to restore one destination.
type Change struct {
	Destination string
	Backup      string
	Existed     bool
}

type pathIdentity struct {
	Device uint64 `json:"device,omitempty"`
	Inode  uint64 `json:"inode,omitempty"`
	Digest string `json:"digest,omitempty"`
}

type journalChange struct {
	Change
	Stage      string       `json:"stage,omitempty"`
	StageID    pathIdentity `json:"stage_identity"`
	OriginalID pathIdentity `json:"original_identity"`
	BackupID   pathIdentity `json:"backup_identity"`
	Complete   bool         `json:"complete"`
}

type journalState struct {
	Version   int             `json:"version"`
	Home      string          `json:"home"`
	Committed bool            `json:"committed"`
	Changes   []journalChange `json:"changes,omitempty"`
	Cleanup   []string        `json:"cleanup,omitempty"`
}

// Journal groups filesystem activations into a reversible operation.
type Journal struct {
	// Home remains public for existing struct-literal callers.
	Home    string
	Changes []Change

	directory string
	path      string
	state     journalState
	err       error
}

// NewJournal creates a journal that persists when its first change is added.
func NewJournal(config JournalConfig) (*Journal, error) {
	directory, err := journalDirectory(config)
	if err != nil {
		return nil, err
	}
	return &Journal{Home: config.Home, directory: directory}, nil
}

// RecoverJournals rolls back unfinished journals and finishes committed cleanup.
func RecoverJournals(config JournalConfig) error {
	directory, err := journalDirectory(config)
	if err != nil {
		return err
	}
	entries, err := os.ReadDir(directory)
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		return fmt.Errorf("read journal directory: %w", err)
	}
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}
		path := filepath.Join(directory, entry.Name())
		journal, err := loadJournal(config.Home, directory, path)
		if err != nil {
			return err
		}
		if journal.state.Committed {
			err = journal.finishCommit()
		} else {
			err = journal.rollback()
		}
		if err != nil {
			return fmt.Errorf("recover journal %s: %w", path, err)
		}
	}
	return nil
}

// Activate records intent before atomically activating a staged path.
func (j *Journal) Activate(stage, destination string) error {
	if err := j.ready(); err != nil {
		return err
	}
	if j.Home != "" {
		if err := GuardHomeAllowLeafSymlink(j.Home, stage, destination); err != nil {
			return err
		}
	}
	stageID, err := identifyPath(stage)
	if err != nil {
		return fmt.Errorf("identify activation stage: %w", err)
	}
	originalID, statErr := identifyPath(destination)
	if statErr != nil && !os.IsNotExist(statErr) {
		return fmt.Errorf("identify activation destination: %w", statErr)
	}
	change := journalChange{
		Change:     Change{Destination: destination, Backup: stage, Existed: statErr == nil},
		Stage:      stage,
		StageID:    stageID,
		OriginalID: originalID,
	}
	j.state.Changes = append(j.state.Changes, change)
	if err := j.persist(); err != nil {
		j.state.Changes = j.state.Changes[:len(j.state.Changes)-1]
		return err
	}
	backup, err := Activate(stage, destination)
	if err != nil {
		return err
	}
	index := len(j.state.Changes) - 1
	j.state.Changes[index].Backup = backup
	j.state.Changes[index].Complete = true
	j.Changes = append(j.Changes, j.state.Changes[index].Change)
	return j.persist()
}

// RecordCreated records a destination that did not exist before mutation.
func (j *Journal) RecordCreated(destination string) error {
	return j.record(Change{Destination: destination})
}

// RecordReplaced records a destination and its existing backup.
func (j *Journal) RecordReplaced(destination, backup string) error {
	return j.record(Change{Destination: destination, Backup: backup, Existed: true})
}

// Record adds an existing mutation to the journal.
// Deprecated: use RecordCreated or RecordReplaced so errors are handled.
func (j *Journal) Record(destination, backup string, existed bool) {
	if existed {
		j.err = errors.Join(j.err, j.RecordReplaced(destination, backup))
		return
	}
	j.err = errors.Join(j.err, j.RecordCreated(destination))
}

// AddCleanup schedules a path for removal after commit or rollback.
func (j *Journal) AddCleanup(path string) error {
	if err := j.ready(); err != nil {
		return err
	}
	if j.Home != "" {
		if err := GuardHomeAllowLeafSymlink(j.Home, path); err != nil {
			return err
		}
	}
	j.state.Cleanup = append(j.state.Cleanup, path)
	return j.persist()
}

// TrackCleanup schedules a path for removal after commit or rollback.
// Deprecated: use AddCleanup so persistence errors are handled.
func (j *Journal) TrackCleanup(path string) {
	j.err = errors.Join(j.err, j.AddCleanup(path))
}

// Rollback restores all recorded destinations in reverse order.
func (j *Journal) Rollback() error {
	return errors.Join(j.err, j.rollback())
}

// Revert joins a primary failure with any rollback failure.
func (j *Journal) Revert(cause error) error {
	if err := j.Rollback(); err != nil {
		return errors.Join(cause, fmt.Errorf("rollback: %w", err))
	}
	return cause
}

// Commit marks success before removing backups and temporary paths.
func (j *Journal) Commit() error {
	if j.err != nil {
		return errors.Join(j.err, j.rollback())
	}
	if err := j.ready(); err != nil {
		return err
	}
	j.state.Committed = true
	if err := j.persist(); err != nil {
		return err
	}
	return j.finishCommit()
}

func (j *Journal) record(change Change) error {
	if err := j.ready(); err != nil {
		return err
	}
	if j.Home != "" {
		if err := GuardHomeAllowLeafSymlink(j.Home, change.Destination, change.Backup); err != nil {
			return err
		}
	}
	var backupID pathIdentity
	if change.Existed {
		var err error
		backupID, err = identifyPath(change.Backup)
		if err != nil {
			return fmt.Errorf("identify mutation backup: %w", err)
		}
	}
	j.Changes = append(j.Changes, change)
	j.state.Changes = append(j.state.Changes, journalChange{Change: change, BackupID: backupID, Complete: true})
	return j.persist()
}

func (j *Journal) ready() error {
	if j.err != nil {
		return j.err
	}
	if j.Home == "" || j.path != "" {
		return nil
	}
	directory, err := journalDirectory(JournalConfig{Home: j.Home, Directory: j.directory})
	if err != nil {
		return err
	}
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return fmt.Errorf("create journal directory: %w", err)
	}
	file, err := os.CreateTemp(directory, ".journal-name-")
	if err != nil {
		return fmt.Errorf("reserve journal name: %w", err)
	}
	name := file.Name()
	if err := file.Close(); err != nil {
		return err
	}
	if err := os.Remove(name); err != nil {
		return err
	}
	j.directory = directory
	j.path = name + ".json"
	j.state = journalState{Version: journalVersion, Home: j.Home}
	for _, change := range j.Changes {
		j.state.Changes = append(j.state.Changes, journalChange{Change: change, Complete: true})
	}
	return nil
}

func (j *Journal) rollback() error {
	if j.path == "" && len(j.state.Changes) == 0 {
		j.state = journalState{Version: journalVersion, Home: j.Home}
		for _, change := range j.Changes {
			j.state.Changes = append(j.state.Changes, journalChange{Change: change, Complete: true})
		}
	}
	for i := len(j.state.Changes) - 1; i >= 0; i-- {
		if err := j.rollbackChange(j.state.Changes[i]); err != nil {
			return err
		}
		j.state.Changes = j.state.Changes[:i]
		if err := j.persist(); err != nil {
			return err
		}
	}
	if err := j.cleanTracked(); err != nil {
		return err
	}
	j.Changes = nil
	return j.removeJournal()
}

func (j *Journal) rollbackChange(change journalChange) error {
	if j.Home != "" {
		if err := GuardHomeAllowLeafSymlink(j.Home, change.Destination, change.Backup, change.Stage); err != nil {
			return err
		}
	}
	if change.Stage != "" {
		return rollbackActivation(change)
	}
	if !change.Existed {
		if err := os.RemoveAll(change.Destination); err != nil {
			return err
		}
		if change.Backup != "" {
			return os.RemoveAll(change.Backup)
		}
		return nil
	}
	if change.BackupID == (pathIdentity{}) {
		return Rollback(change.Destination, change.Backup)
	}
	destinationID, destinationErr := identifyPath(change.Destination)
	if destinationErr == nil && destinationID == change.BackupID {
		return os.RemoveAll(change.Backup)
	}
	backupID, backupErr := identifyPath(change.Backup)
	if backupErr == nil && backupID == change.BackupID {
		return Rollback(change.Destination, change.Backup)
	}
	if destinationErr == nil && os.IsNotExist(backupErr) {
		return fmt.Errorf("mutation backup disappeared: %s", change.Backup)
	}
	return fmt.Errorf("mutation paths changed unexpectedly: %s", change.Destination)
}

func rollbackActivation(change journalChange) error {
	destinationID, destinationErr := identifyPath(change.Destination)
	stageID, stageErr := identifyPath(change.Stage)
	if change.Existed {
		if destinationErr == nil && destinationID == change.OriginalID {
			if stageErr == nil {
				return os.RemoveAll(change.Stage)
			}
			return ignoreNotExist(stageErr)
		}
		if destinationErr == nil && destinationID == change.StageID && stageErr == nil && stageID == change.OriginalID {
			return Rollback(change.Destination, change.Stage)
		}
		return fmt.Errorf("activation paths changed unexpectedly: %s", change.Destination)
	}
	if destinationErr == nil && destinationID == change.StageID {
		return os.RemoveAll(change.Destination)
	}
	if os.IsNotExist(destinationErr) && stageErr == nil && stageID == change.StageID {
		return os.RemoveAll(change.Stage)
	}
	if os.IsNotExist(destinationErr) && os.IsNotExist(stageErr) {
		return nil
	}
	return fmt.Errorf("activation paths changed unexpectedly: %s", change.Destination)
}

func (j *Journal) finishCommit() error {
	for len(j.state.Changes) > 0 {
		change := j.state.Changes[0]
		if change.Backup != "" {
			if j.Home != "" {
				if err := GuardHomeAllowLeafSymlink(j.Home, change.Backup); err != nil {
					return err
				}
			}
			if err := os.RemoveAll(change.Backup); err != nil {
				return err
			}
		}
		j.state.Changes = j.state.Changes[1:]
		if err := j.persist(); err != nil {
			return err
		}
	}
	if err := j.cleanTracked(); err != nil {
		return err
	}
	j.Changes = nil
	return j.removeJournal()
}

func (j *Journal) cleanTracked() error {
	for len(j.state.Cleanup) > 0 {
		index := len(j.state.Cleanup) - 1
		path := j.state.Cleanup[index]
		if j.Home != "" {
			if err := GuardHomeAllowLeafSymlink(j.Home, path); err != nil {
				return err
			}
		}
		if err := os.RemoveAll(path); err != nil {
			return err
		}
		j.state.Cleanup = j.state.Cleanup[:index]
		if err := j.persist(); err != nil {
			return err
		}
	}
	return nil
}

func (j *Journal) persist() error {
	if j.path == "" {
		return nil
	}
	data, err := json.Marshal(j.state)
	if err != nil {
		return fmt.Errorf("encode journal: %w", err)
	}
	return atomicWriteJSON(j.path, append(data, '\n'))
}

func (j *Journal) removeJournal() error {
	if j.path == "" {
		return nil
	}
	if err := os.Remove(j.path); err != nil && !os.IsNotExist(err) {
		return err
	}
	err := syncDirectory(filepath.Dir(j.path))
	j.path = ""
	return err
}

func journalDirectory(config JournalConfig) (string, error) {
	if config.Home == "" {
		return "", fmt.Errorf("journal HOME is required")
	}
	directory := config.Directory
	if directory == "" {
		directory = filepath.Join(config.Home, ".local", "state", "pde", "fs-journals")
	}
	if err := GuardHome(config.Home, directory); err != nil {
		return "", err
	}
	return directory, nil
}

func loadJournal(home, directory, path string) (*Journal, error) {
	if err := GuardHome(home, directory, path); err != nil {
		return nil, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var state journalState
	if err := json.Unmarshal(data, &state); err != nil {
		return nil, fmt.Errorf("decode journal %s: %w", path, err)
	}
	if state.Version != journalVersion || state.Home != home {
		return nil, fmt.Errorf("journal %s has invalid version or HOME", path)
	}
	for _, change := range state.Changes {
		if err := GuardHomeAllowLeafSymlink(home, change.Destination, change.Backup, change.Stage); err != nil {
			return nil, fmt.Errorf("validate journal %s: %w", path, err)
		}
	}
	if err := GuardHomeAllowLeafSymlink(home, state.Cleanup...); err != nil {
		return nil, fmt.Errorf("validate journal %s: %w", path, err)
	}
	return &Journal{Home: home, directory: directory, path: path, state: state}, nil
}

func atomicWriteJSON(path string, data []byte) error {
	directory := filepath.Dir(path)
	temporary, err := os.CreateTemp(directory, ".journal-write-")
	if err != nil {
		return err
	}
	name := temporary.Name()
	defer func() { _ = os.Remove(name) }()
	if err := temporary.Chmod(0o600); err != nil {
		_ = temporary.Close()
		return err
	}
	if _, err := temporary.Write(data); err != nil {
		_ = temporary.Close()
		return err
	}
	if err := temporary.Sync(); err != nil {
		_ = temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	if err := os.Rename(name, path); err != nil {
		return err
	}
	return syncDirectory(directory)
}

func syncDirectory(path string) (err error) {
	directory, err := os.Open(path)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, directory.Close()) }()
	return directory.Sync()
}

func ignoreNotExist(err error) error {
	if os.IsNotExist(err) {
		return nil
	}
	return err
}
