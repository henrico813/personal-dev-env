package review

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"sort"
)

// What the owner asks the AI to change about an item. It stays open until a
// later round includes the item again.
type Note struct {
	Text  string `json:"text"`
	Round int    `json:"round"`
}

// One named review: its rounds of proposals and the owner's notes, saved in
// Dir.
type Review struct {
	Name   string
	Dir    string
	Rounds []Proposals // Rounds[0] is round 1.
	Notes  map[int]Note
}

// The newest version of an item, the round it is from, and the owner's
// note on that version.
type Entry struct {
	Item  Item
	Round int
	Note  string // the open note, or ""
}

var namePattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._-]*$`)

// Returns the folder that holds every review:
// $XDG_STATE_HOME/pde/comment-review, or ~/.local/state/pde/comment-review.
func findStateRoot() string {
	root := os.Getenv("XDG_STATE_HOME")
	if root == "" {
		home, _ := os.UserHomeDir()
		root = filepath.Join(home, ".local", "state")
	}
	return filepath.Join(root, "pde", "comment-review")
}

// Saves p as the next round of the review called name, starting the review
// if it is new. Returns the round number.
func AddRound(name string, p Proposals) (int, error) {
	r, err := loadReview(name)
	if err != nil {
		return 0, err
	}
	if len(r.Rounds) > 0 && r.Repo() != p.Repo {
		return 0, fmt.Errorf("review %s is for %s, not %s", name, r.Repo(), p.Repo)
	}
	round := len(r.Rounds) + 1
	return round, replaceJSONFile(buildRoundFilePath(r.Dir, round), p)
}

// Loads the review called name, which must have at least one round.
func OpenReview(name string) (*Review, error) {
	r, err := loadReview(name)
	if err != nil {
		return nil, err
	}
	if len(r.Rounds) == 0 {
		return nil, fmt.Errorf("no review named %q", name)
	}
	return r, nil
}

// Reads whatever is saved for name; a new review has no rounds.
func loadReview(name string) (*Review, error) {
	// The name becomes a folder name, so it must not climb out of the state
	// folder.
	if !namePattern.MatchString(name) {
		return nil, fmt.Errorf("review name %q may use only a-z, 0-9, '.', '_', and '-'", name)
	}
	r := &Review{Name: name, Dir: filepath.Join(findStateRoot(), name), Notes: map[int]Note{}}
	for round := 1; ; round++ {
		data, err := os.ReadFile(buildRoundFilePath(r.Dir, round))
		if errors.Is(err, fs.ErrNotExist) {
			break
		}
		if err != nil {
			return nil, err
		}
		var p Proposals
		if err := json.Unmarshal(data, &p); err != nil {
			return nil, fmt.Errorf("round %d of %s: %w", round, name, err)
		}
		r.Rounds = append(r.Rounds, p)
	}
	if err := readJSONFile(filepath.Join(r.Dir, "notes.json"), &r.Notes); err != nil {
		return nil, err
	}
	return r, nil
}

func buildRoundFilePath(dir string, round int) string {
	return filepath.Join(dir, fmt.Sprintf("round-%d.json", round))
}

// The repository the review changes.
func (r *Review) Repo() string { return r.Rounds[0].Repo }

// Returns the latest version of every item, by id.
func (r *Review) ListEntries() []Entry {
	latest := map[int]Entry{}
	for i, p := range r.Rounds {
		for _, item := range p.Items {
			latest[item.ID] = Entry{Item: item, Round: i + 1}
		}
	}
	entries := make([]Entry, 0, len(latest))
	for id, e := range latest {
		if n, ok := r.Notes[id]; ok && n.Round == e.Round {
			e.Note = n.Text
		}
		entries = append(entries, e)
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Item.ID < entries[j].Item.ID })
	return entries
}

// Saves a note on the latest round of item id; empty text removes it.
func (r *Review) SetNote(id int, text string) error {
	if text == "" {
		delete(r.Notes, id)
	} else {
		r.Notes[id] = Note{Text: text, Round: r.findLatestRound(id)}
	}
	return replaceJSONFile(filepath.Join(r.Dir, "notes.json"), r.Notes)
}

func (r *Review) findLatestRound(id int) int {
	for i := len(r.Rounds) - 1; i >= 0; i-- {
		for _, item := range r.Rounds[i].Items {
			if item.ID == id {
				return i + 1
			}
		}
	}
	return 0
}

// Reads the JSON in path into value; a missing file leaves value as it is.
func readJSONFile(path string, value any) error {
	data, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	if err := json.Unmarshal(data, value); err != nil {
		return fmt.Errorf("%s: %w", path, err)
	}
	return nil
}

func replaceJSONFile(path string, value any) error {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	return replaceFile(path, append(data, '\n'))
}

// Writes data to a temporary file, then renames it over path, so readers
// never see a half-written file.
func replaceFile(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".tmp-")
	if err != nil {
		return err
	}
	defer os.Remove(tmp.Name())
	if _, err := tmp.Write(data); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	return os.Rename(tmp.Name(), path)
}
