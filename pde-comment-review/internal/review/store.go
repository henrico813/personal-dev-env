package review

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"sort"
)

// What the owner asks the AI to change about an item. It stays open until a
// later round includes the item again.
type Note struct {
	Text  string `json:"text"`
	Batch int    `json:"round"`
}

// The owner's answer to one item in one round.
type Answer struct {
	Action string `json:"action"`
	Text   string `json:"text,omitempty"` // the owner's comment lines, for ActionEdited
	Batch  int    `json:"round"`
}

const (
	ActionKeep   = "keep"
	ActionCut    = "cut"
	ActionAccept = "accept" // take the AI's suggestion
	ActionEdited = "edited" // use the owner's own wording
)

// One named review: its rounds of proposals and the owner's notes, saved in
// Dir.
type Review struct {
	Name    string
	Dir     string
	Batches []Batch // Batches[0] is batch 1.
	Notes   map[int]Note
	Answers map[int]Answer
	// The round apply last wrote into the code, or 0.
	AppliedBatch int
}

// The newest version of an item, the round it is from, and the owner's
// note on that version.
type RequestWithFeedback struct {
	Request  FeedbackRequest
	Batch    int
	Feedback UserFeedback
}

// The owner's answer and note on one FeedbackRequest.
type UserFeedback struct {
	Answer *Answer // nil until the owner answers the request from its latest batch
	Note   string  // the open note, or ""
}

var namePattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._-]*$`)

// Returns the folder that holds every review:
// $XDG_STATE_HOME/pde/comment-review, or ~/.local/state/pde/comment-review.
func FindStateRoot() string {
	root := os.Getenv("XDG_STATE_HOME")
	if root == "" {
		home, _ := os.UserHomeDir()
		root = filepath.Join(home, ".local", "state")
	}
	return filepath.Join(root, "pde", "comment-review")
}

// Saves p as the next round of the review called name, starting the review
// if it is new. Returns the round number.
func AddBatch(name string, p Batch) (int, error) {
	r, err := BuildReviewWithBatch(
		name,
		p,
	)
	if err != nil {
		return 0, err
	}
	batch := len(r.Batches)
	return batch, replaceJSONFile(
		buildBatchFilePath(
			r.Dir,
			batch,
		),
		p,
	)
}

// Returns the whole review called name, every earlier round plus p as
// its next one, without saving anything, so add can check it before the
// owner sees it.
func BuildReviewWithBatch(
	name string,
	p Batch,
) (*Review, error) {
	r, err := loadReview(name)
	if err != nil {
		return nil, err
	}
	if len(r.Batches) > 0 && r.Repo() != p.Repo {
		return nil, fmt.Errorf(
			"review %s is for %s, not %s",
			name,
			r.Repo(),
			p.Repo,
		)
	}
	r.Batches = append(
		r.Batches,
		p,
	)
	return r, nil
}

// Loads the review called name, which must have at least one round.
func OpenReview(name string) (*Review, error) {
	r, err := loadReview(name)
	if err != nil {
		return nil, err
	}
	if len(r.Batches) == 0 {
		return nil, fmt.Errorf("no review named %q", name)
	}
	return r, nil
}

// Reads whatever is saved for name; a new review has no rounds.
func loadReview(name string) (*Review, error) {
	dir, err := buildReviewDir(name)
	if err != nil {
		return nil, err
	}
	r := &Review{Name: name, Dir: dir, Notes: map[int]Note{}, Answers: map[int]Answer{}}
	for batch := 1; ; batch++ {
		data, err := os.ReadFile(buildBatchFilePath(r.Dir, batch))
		if errors.Is(err, fs.ErrNotExist) {
			break
		}
		if err != nil {
			return nil, err
		}
		var p Batch
		if err := json.Unmarshal(data, &p); err != nil {
			return nil, fmt.Errorf("round %d of %s: %w", batch, name, err)
		}
		r.Batches = append(r.Batches, p)
	}
	if err := readJSONFile(filepath.Join(r.Dir, "notes.json"), &r.Notes); err != nil {
		return nil, err
	}
	if err := readJSONFile(filepath.Join(r.Dir, "decisions.json"), &r.Answers); err != nil {
		return nil, err
	}
	var applied appliedFile
	if err := readJSONFile(filepath.Join(r.Dir, "applied.json"), &applied); err != nil {
		return nil, err
	}
	r.AppliedBatch = applied.Batch
	return r, nil
}

func buildReviewDir(name string) (string, error) {
	// The name becomes a folder name, so it must not climb out of the state
	// folder.
	if !namePattern.MatchString(name) {
		return "", fmt.Errorf("review name %q may use only a-z, 0-9, '.', '_', and '-'", name)
	}
	return filepath.Join(FindStateRoot(), name), nil
}

func buildBatchFilePath(dir string, batch int) string {
	return filepath.Join(dir, fmt.Sprintf("round-%d.json", batch))
}

// The repository the review changes.
func (r *Review) Repo() string { return r.Batches[0].Repo }

// Returns the latest version of every item, by id.
func (r *Review) ListFeedbackRequests() []RequestWithFeedback {
	latest := map[int]RequestWithFeedback{}
	for i, p := range r.Batches {
		for _, request := range p.Requests {
			latest[request.ID] = RequestWithFeedback{Request: request, Batch: i + 1}
		}
	}
	requests := make([]RequestWithFeedback, 0, len(latest))
	for id, e := range latest {
		if n, ok := r.Notes[id]; ok && n.Batch == e.Batch {
			e.Feedback.Note = n.Text
		}
		if d, ok := r.Answers[id]; ok && d.Batch == e.Batch {
			e.Feedback.Answer = &d
		}
		requests = append(requests, e)
	}
	sort.Slice(requests, func(i, j int) bool { return requests[i].Request.ID < requests[j].Request.ID })
	return requests
}

// Saves a note on the latest round of item id; empty text removes it.
func (r *Review) SaveNote(id int, text string) error {
	// The popup and a Herdr tab can both save this review, so read it first.
	saved, err := loadReview(r.Name)
	if err != nil {
		return err
	}
	*r = *saved
	if text == "" {
		delete(r.Notes, id)
	} else {
		r.Notes[id] = Note{Text: text, Batch: r.findLatestBatch(id)}
	}
	return replaceJSONFile(filepath.Join(r.Dir, "notes.json"), r.Notes)
}

// Saves the owner's answer to the suggestion they saw in the given batch.
func (r *Review) SaveAnswer(id, batch int, action, text string) error {
	// The popup and a Herdr tab can both save this review, so read it first.
	saved, err := loadReview(r.Name)
	if err != nil {
		return err
	}
	*r = *saved
	r.Answers[id] = Answer{Action: action, Text: text, Batch: batch}
	return replaceJSONFile(filepath.Join(r.Dir, "decisions.json"), r.Answers)
}

// Returns every file the review's items are in, sorted.
func (r *Review) ListFiles() []string {
	var files []string
	for _, e := range r.ListFeedbackRequests() {
		files = append(files, e.Request.File)
	}
	sort.Strings(files)
	return slices.Compact(files)
}

// Returns the AI's "OLD=NEW" renames from every round, each once.
func (r *Review) ListRenames() []string {
	var pairs []string
	for _, p := range r.Batches {
		for _, pair := range p.Renames {
			if !slices.Contains(pairs, pair) {
				pairs = append(pairs, pair)
			}
		}
	}
	return pairs
}

func (r *Review) findLatestBatch(id int) int {
	for i := len(r.Batches) - 1; i >= 0; i-- {
		for _, request := range r.Batches[i].Requests {
			if request.ID == id {
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
