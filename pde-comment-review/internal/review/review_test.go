package review

import (
	"encoding/json"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"
)

const configGo = `package config

const (
	DefaultHost = "localhost"
	// Used when the config file sets no port. Ports below 1024 need root
	// on Linux, so the default is above them.
	DefaultPort = 8080
)

// Reads the config file at path and returns its settings.
func ParseConfig(path string) (map[string]string, error) { return nil, nil }

// Deprecated: use ParseConfig.
func Load(path string) (map[string]string, error) { return ParseConfig(path) }

// Deprecated: use ParseConfig.
func Read(path string) (map[string]string, error) { return ParseConfig(path) }

// Names one setting in the config file.
//go:generate stringer -type=Setting
type Setting int
` + "\nvar sample = `\n// Sample settings, as the help text prints them.\nport = 8080\n`\n"

// Writes config.go into a new repository folder, points the review state at
// a new folder, and returns the repository path.
func newRepo(t *testing.T) string {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	repo := t.TempDir()
	if err := os.WriteFile(filepath.Join(repo, "config.go"), []byte(configGo), 0o644); err != nil {
		t.Fatal(err)
	}
	return repo
}

func marshalBatch(t *testing.T, repo string, requests ...FeedbackRequest) string {
	t.Helper()
	data, err := json.Marshal(Batch{Version: Version, Repo: repo, Requests: requests})
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func addTestBatch(t *testing.T, name, data string) {
	t.Helper()
	p, err := ParseBatch([]byte(data))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := AddBatch(name, p); err != nil {
		t.Fatal(err)
	}
}

var (
	cutPort = FeedbackRequest{ID: 1, File: "config.go", Suggestion: SuggestCut,
		Old: "\t// Used when the config file sets no port. Ports below 1024 need root\n\t// on Linux, so the default is above them.\n"}
	rewordParse = FeedbackRequest{ID: 2, File: "config.go", Suggestion: SuggestReword,
		Old: "// Reads the config file at path and returns its settings.\n", New: "// Reads the config file at path and fills in a default for each missing key.\n"}
)

// An agent writes the proposals file by hand, so a typo or a path outside the
// repo must be refused before the owner sees the review.
func TestParseBatchRefusesUnsafeFiles(t *testing.T) {
	repo := newRepo(t)
	tests := []struct {
		name string
		data string
		want string // "" when the file is accepted
	}{
		{
			name: "misspelled field",
			data: `{"version": 1, "repo": "` + repo + `", "itemz": []}`,
			want: "unknown field",
		},
		{
			name: "text after the JSON",
			data: marshalBatch(t, repo, rewordParse) + "\n{}",
			want: "text after the JSON object",
		},
		{
			name: "file outside repo",
			data: marshalBatch(t, repo, FeedbackRequest{ID: 1, File: "../config.go", Suggestion: SuggestKeep, Old: rewordParse.Old}),
			want: "inside the repo",
		},
		{
			name: "id used twice",
			data: marshalBatch(t, repo, rewordParse, rewordParse),
			want: "used twice",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := ParseBatch([]byte(tt.data))

			if (err == nil) != (tt.want == "") || err != nil && !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("ParseBatch() error = %v, want it to contain %q", err, tt.want)
			}
		})
	}
}

// The owner leaves notes, and the AI's next round may answer only some of
// them. Each round was once its own file, so answers were merged by hand.
// A new round replaces only the items it answers and closes their notes;
// the other notes stay open, and so does a note made after that round.
func TestNewBatchClosesOnlyItsNotes(t *testing.T) {
	repo := newRepo(t)
	addTestBatch(t, "config", marshalBatch(t, repo, cutPort, rewordParse))
	r, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	if err := r.SaveNote(1, "keep the first sentence"); err != nil {
		t.Fatal(err)
	}
	if err := r.SaveNote(2, "shorter"); err != nil {
		t.Fatal(err)
	}
	shorter := rewordParse
	shorter.New = "// Reads the config file at path.\n"

	addTestBatch(t, "config", marshalBatch(t, repo, shorter))
	r, err = OpenReview("config")

	if err != nil {
		t.Fatal(err)
	}
	requests := r.ListFeedbackRequests()
	if len(requests) != 2 {
		t.Fatalf("got %d entries, want 2", len(requests))
	}
	if e := requests[0]; e.Batch != 1 || e.Feedback.Note != "keep the first sentence" {
		t.Errorf("item 1 = %+v, want round 1 with its note open", e)
	}
	if e := requests[1]; e.Batch != 2 || e.Request.New != shorter.New || e.Feedback.Note != "" {
		t.Errorf("item 2 = %+v, want round 2's wording with no open note", e)
	}
	if err := r.SaveNote(2, "name the keys"); err != nil {
		t.Fatal(err)
	}
	if e := r.ListFeedbackRequests()[1]; e.Feedback.Note != "name the keys" {
		t.Errorf("item 2 note = %q, want the note made in round 2 open", e.Feedback.Note)
	}
}

// The popup and a Herdr tab can be open on the same review. A save in one
// once erased the notes and answers saved in the other.
func TestConsistentTabPopupStateOnSave(t *testing.T) {
	repo := newRepo(t)
	addTestBatch(t, "config", marshalBatch(t, repo, cutPort, rewordParse))
	inPopup, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	inTab, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}

	// The owner saves a note on each request, one in the popup and one in
	// the tab.
	if err := inPopup.SaveNote(1, "keep the first sentence"); err != nil {
		t.Fatal(err)
	}
	if err := inTab.SaveNote(2, "name the keys"); err != nil {
		t.Fatal(err)
	}
	// The owner answers each request, one in the popup and one in the tab.
	if err := inPopup.SaveAnswer(1, 1, ActionKeep, ""); err != nil {
		t.Fatal(err)
	}
	if err := inTab.SaveAnswer(2, 1, ActionCut, ""); err != nil {
		t.Fatal(err)
	}

	saved, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	requests := saved.ListFeedbackRequests()
	if e := requests[0]; e.Feedback.Note != "keep the first sentence" || e.Feedback.Answer == nil {
		t.Errorf("request 1 = %+v, want the popup's note and answer", e)
	}
	if e := requests[1]; e.Feedback.Note != "name the keys" || e.Feedback.Answer == nil {
		t.Errorf("request 2 = %+v, want the tab's note and answer", e)
	}
}

// The AI can send a new suggestion while the owner reads the old one. An
// answer to the old one must not count as an answer to text the owner never
// saw.
func TestReplacedSuggestionNeedsNewAnswer(t *testing.T) {
	repo := newRepo(t)
	addTestBatch(t, "config", marshalBatch(t, repo, cutPort, rewordParse))
	r, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	shown := r.ListFeedbackRequests()[1]

	// The AI sends a new suggestion while the owner reads the first one.
	addTestBatch(t, "config", marshalBatch(t, repo, rewordParse))
	if err := r.SaveAnswer(shown.Request.ID, shown.Batch, ActionAccept, ""); err != nil {
		t.Fatal(err)
	}

	saved, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	if e := saved.ListFeedbackRequests()[1]; e.Batch != 2 || e.Feedback.Answer != nil {
		t.Errorf("request 2 = %+v, want the new suggestion with no answer", e)
	}
}

// Ctrl+b e opens the only waiting review, or lists them newest first within
// each session. The owner once left ten notes on another session's review,
// so the wrong order, an applied or removed review still listed, or a new
// round left out costs real answers.
func TestWaitingReviewsListNewestFirst(t *testing.T) {
	repo := newRepo(t)
	start := time.Now()
	// Sets a batch's file time, since batches saved in one test can share one.
	setBatchTime := func(name string, batch int, minutes int) {
		t.Helper()
		r, err := OpenReview(name)
		if err != nil {
			t.Fatal(err)
		}
		added := start.Add(time.Duration(minutes) * time.Minute)
		if err := os.Chtimes(buildBatchFilePath(r.Dir, batch), added, added); err != nil {
			t.Fatal(err)
		}
	}
	setApplied := func(name string) {
		t.Helper()
		r, err := OpenReview(name)
		if err != nil {
			t.Fatal(err)
		}
		if err := r.SetApplied(); err != nil {
			t.Fatal(err)
		}
	}
	for i, name := range []string{"older", "applied", "newer", "readded"} {
		addTestBatch(t, name, marshalBatch(t, repo, rewordParse))
		setBatchTime(name, 1, i)
	}
	setApplied("applied")
	// An applied review waits again once an AI adds a batch.
	setApplied("readded")
	addTestBatch(t, "readded", marshalBatch(t, repo, rewordParse))
	setBatchTime("readded", 2, 10)
	gone := t.TempDir()
	if err := os.WriteFile(filepath.Join(gone, "config.go"), []byte(configGo), 0o644); err != nil {
		t.Fatal(err)
	}
	addTestBatch(t, "gone", marshalBatch(t, gone, rewordParse))
	if err := os.RemoveAll(gone); err != nil {
		t.Fatal(err)
	}

	waiting, err := ListWaitingReviews()

	if err != nil {
		t.Fatal(err)
	}
	var names []string
	for _, r := range waiting {
		names = append(names, r.Name)
	}
	if want := []string{"readded", "newer", "older"}; !slices.Equal(names, want) {
		t.Errorf("waiting reviews = %q, want %q", names, want)
	}
}
