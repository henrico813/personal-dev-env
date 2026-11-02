// Package cache keeps the PR list and each pull request's description,
// files, comments and diff on disk, so screens open without calling GitHub.
// An entry's file name holds the pull request's last-update time, so a
// changed pull request misses the cache and an unchanged one never
// refetches.
package cache

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"prs/internal/github"
)

// The PR list's file in the cache folder.
const listFile = "list.json"

// Holds the PR list and when prs got it from GitHub.
type List struct {
	FetchedAt    time.Time            `json:"fetchedAt"`
	PullRequests []github.PullRequest `json:"pullRequests"`
}

// Reads and writes one cache folder.
type Store struct {
	dir string
}

// Returns the store in $XDG_CACHE_HOME/prs, which is usually ~/.cache/prs.
// The folder is made when the store first writes to it.
func Open() (
	Store,
	error,
) {
	base, err := os.UserCacheDir()
	if err != nil {
		return Store{}, err
	}
	return Store{dir: filepath.Join(
		base,
		"prs",
	)}, nil
}

// Returns the saved PR list, or an error when there is none.
func (s Store) ReadList() (
	List,
	error,
) {
	var list List
	err := s.readJSON(
		listFile,
		&list,
	)
	return list, err
}

func (s Store) WriteList(list List) error {
	return s.writeJSON(
		listFile,
		list,
	)
}

// Returns the pull request with this repo and number from the saved list.
func (s Store) FindPullRequest(
	repo string,
	number int,
) (
	github.PullRequest,
	bool,
) {
	list, err := s.ReadList()
	if err != nil {
		return github.PullRequest{}, false
	}
	for _, pr := range list.PullRequests {
		if pr.Repo() == repo && pr.Number == number {
			return pr, true
		}
	}
	return github.PullRequest{}, false
}

// Returns the description, files and comments saved for the pull request
// at its update time.
func (s Store) ReadExtras(pr github.PullRequest) (
	github.Extras,
	bool,
) {
	var extras github.Extras
	err := s.readJSON(
		nameEntry(pr)+".json",
		&extras,
	)
	return extras, err == nil
}

func (s Store) WriteExtras(
	pr github.PullRequest,
	extras github.Extras,
) error {
	return s.writeJSON(
		nameEntry(pr)+".json",
		extras,
	)
}

// Returns whether the description, files and comments are saved for the
// pull request at its update time.
func (s Store) HasExtras(pr github.PullRequest) bool {
	return s.hasFile(nameEntry(pr) + ".json")
}

// Returns whether a diff is saved for the pull request at its update time.
func (s Store) HasDiff(pr github.PullRequest) bool {
	return s.hasFile(nameEntry(pr) + ".diff")
}

// Returns the diff saved for the pull request at its update time.
func (s Store) ReadDiff(pr github.PullRequest) (
	[]byte,
	bool,
) {
	diff, err := os.ReadFile(filepath.Join(
		s.dir,
		nameEntry(pr)+".diff",
	))
	return diff, err == nil
}

func (s Store) WriteDiff(
	pr github.PullRequest,
	diff []byte,
) error {
	return s.writeFile(
		nameEntry(pr)+".diff",
		diff,
	)
}

// Removes every entry that is not for a listed pull request at its listed
// update time, which covers closed pull requests and older update times of
// open ones. A half-written file older than a minute goes too, since its
// writer died.
func (s Store) RemoveUnlisted(prs []github.PullRequest) {
	keep := map[string]bool{listFile: true}
	for _, pr := range prs {
		keep[nameEntry(pr)+".json"] = true
		keep[nameEntry(pr)+".diff"] = true
	}
	entries, err := os.ReadDir(s.dir)
	if err != nil {
		return
	}
	for _, entry := range entries {
		name := entry.Name()
		if keep[name] {
			continue
		}
		// writeFile names a file it is writing NAME.tmpRANDOM.
		if strings.HasPrefix(
			filepath.Ext(name),
			".tmp",
		) {
			info, err := entry.Info()
			if err != nil || time.Since(info.ModTime()) < time.Minute {
				continue
			}
		}
		_ = os.Remove(filepath.Join(
			s.dir,
			name,
		))
	}
}

// Returns the file name, without extension, of a pull request at one update
// time, such as "owner+repo#12@1761990452".
func nameEntry(pr github.PullRequest) string {
	return strings.ReplaceAll(
		pr.Repo(),
		"/",
		"+",
	) + "#" + strconv.Itoa(pr.Number) + "@" + strconv.FormatInt(
		pr.UpdatedAt.Unix(),
		10,
	)
}

func (s Store) hasFile(name string) bool {
	_, err := os.Stat(filepath.Join(
		s.dir,
		name,
	))
	return err == nil
}

func (s Store) readJSON(
	name string,
	into any,
) error {
	data, err := os.ReadFile(filepath.Join(
		s.dir,
		name,
	))
	if err != nil {
		return err
	}
	return json.Unmarshal(
		data,
		into,
	)
}

func (s Store) writeJSON(
	name string,
	value any,
) error {
	data, err := json.Marshal(value)
	if err != nil {
		return err
	}
	return s.writeFile(
		name,
		data,
	)
}

// Writes a file under a temporary name and renames it, so a reader in
// another prs process never sees half a file.
func (s Store) writeFile(
	name string,
	data []byte,
) error {
	if err := os.MkdirAll(
		s.dir,
		0o700,
	); err != nil {
		return err
	}
	file, err := os.CreateTemp(
		s.dir,
		name+".tmp*",
	)
	if err != nil {
		return err
	}
	_, err = file.Write(data)
	if closeErr := file.Close(); err == nil {
		err = closeErr
	}
	if err == nil {
		err = os.Rename(
			file.Name(),
			filepath.Join(
				s.dir,
				name,
			),
		)
	}
	if err != nil {
		_ = os.Remove(file.Name())
	}
	return err
}
