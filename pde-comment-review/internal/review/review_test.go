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

func marshalProposals(t *testing.T, repo string, items ...Item) string {
	t.Helper()
	data, err := json.Marshal(Proposals{Version: Version, Repo: repo, Items: items})
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func addTestRound(t *testing.T, name, data string) {
	t.Helper()
	p, err := ParseProposals([]byte(data))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := AddRound(name, p); err != nil {
		t.Fatal(err)
	}
}

var (
	cutPort = Item{ID: 1, File: "config.go", Proposal: ProposeCut,
		Old: "\t// Used when the config file sets no port. Ports below 1024 need root\n\t// on Linux, so the default is above them.\n"}
	rewordParse = Item{ID: 2, File: "config.go", Proposal: ProposeReword,
		Old: "// Reads the config file at path and returns its settings.\n", New: "// Reads the config file at path and fills in a default for each missing key.\n"}
)

// An agent writes the proposals file by hand, so a typo or a path outside the
// repo must be refused before the owner sees the review.
func TestParseProposalsRefusesUnsafeFiles(t *testing.T) {
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
			data: marshalProposals(t, repo, rewordParse) + "\n{}",
			want: "text after the JSON object",
		},
		{
			name: "file outside repo",
			data: marshalProposals(t, repo, Item{ID: 1, File: "../config.go", Proposal: ProposeKeep, Old: rewordParse.Old}),
			want: "inside the repo",
		},
		{
			name: "id used twice",
			data: marshalProposals(t, repo, rewordParse, rewordParse),
			want: "used twice",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := ParseProposals([]byte(tt.data))

			if (err == nil) != (tt.want == "") || err != nil && !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("ParseProposals() error = %v, want it to contain %q", err, tt.want)
			}
		})
	}
}

// The owner leaves notes, and the AI's next round may answer only some of
// them. Each round was once its own file, so answers were merged by hand.
// A new round replaces only the items it answers and closes their notes;
// the other notes stay open, and so does a note made after that round.
func TestNewRoundClosesOnlyItsNotes(t *testing.T) {
	repo := newRepo(t)
	addTestRound(t, "config", marshalProposals(t, repo, cutPort, rewordParse))
	r, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	if err := r.SetNote(1, "keep the first sentence"); err != nil {
		t.Fatal(err)
	}
	if err := r.SetNote(2, "shorter"); err != nil {
		t.Fatal(err)
	}
	shorter := rewordParse
	shorter.New = "// Reads the config file at path.\n"

	addTestRound(t, "config", marshalProposals(t, repo, shorter))
	r, err = OpenReview("config")

	if err != nil {
		t.Fatal(err)
	}
	entries := r.ListEntries()
	if len(entries) != 2 {
		t.Fatalf("got %d entries, want 2", len(entries))
	}
	if e := entries[0]; e.Round != 1 || e.Note != "keep the first sentence" {
		t.Errorf("item 1 = %+v, want round 1 with its note open", e)
	}
	if e := entries[1]; e.Round != 2 || e.Item.New != shorter.New || e.Note != "" {
		t.Errorf("item 2 = %+v, want round 2's wording with no open note", e)
	}
	if err := r.SetNote(2, "name the keys"); err != nil {
		t.Fatal(err)
	}
	if e := r.ListEntries()[1]; e.Note != "name the keys" {
		t.Errorf("item 2 note = %q, want the note made in round 2 open", e.Note)
	}
}

// The popup and a review tab each hold their own copy of one review. A save
// from either must keep the other's saves and land on the newest round.
func TestSaveKeepsWhatOtherCopiesSaved(t *testing.T) {
	repo := newRepo(t)
	addTestRound(t, "config", marshalProposals(t, repo, cutPort, rewordParse))
	popup, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	tab, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}

	if err := popup.SetNote(1, "keep the first sentence"); err != nil {
		t.Fatal(err)
	}
	if err := popup.SetDecision(1, ActionKeep, ""); err != nil {
		t.Fatal(err)
	}
	addTestRound(t, "config", marshalProposals(t, repo, rewordParse))
	if err := tab.SetDecision(2, ActionCut, ""); err != nil {
		t.Fatal(err)
	}
	if err := popup.SetNote(2, "name the keys"); err != nil {
		t.Fatal(err)
	}

	saved, err := OpenReview("config")
	if err != nil {
		t.Fatal(err)
	}
	entries := saved.ListEntries()
	if e := entries[0]; e.Note != "keep the first sentence" || e.Decision == nil {
		t.Errorf("item 1 = %+v, want the popup's note and answer", e)
	}
	if e := entries[1]; e.Round != 2 || e.Note != "name the keys" || e.Decision == nil {
		t.Errorf("item 2 = %+v, want a note and answer on round 2", e)
	}
}

// Ctrl+b e opens the only waiting review, or lists them newest first within
// each session. The owner once left ten notes on another session's review,
// so the wrong order, an applied or removed review still listed, or a new
// round left out costs real answers.
func TestWaitingReviewsListNewestFirst(t *testing.T) {
	repo := newRepo(t)
	start := time.Now()
	// Sets a round's file time, since rounds saved in one test can share one.
	setRoundTime := func(name string, round int, minutes int) {
		t.Helper()
		r, err := OpenReview(name)
		if err != nil {
			t.Fatal(err)
		}
		added := start.Add(time.Duration(minutes) * time.Minute)
		if err := os.Chtimes(buildRoundFilePath(r.Dir, round), added, added); err != nil {
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
		addTestRound(t, name, marshalProposals(t, repo, rewordParse))
		setRoundTime(name, 1, i)
	}
	setApplied("applied")
	// An applied review waits again once an AI adds a round.
	setApplied("readded")
	addTestRound(t, "readded", marshalProposals(t, repo, rewordParse))
	setRoundTime("readded", 2, 10)
	gone := t.TempDir()
	if err := os.WriteFile(filepath.Join(gone, "config.go"), []byte(configGo), 0o644); err != nil {
		t.Fatal(err)
	}
	addTestRound(t, "gone", marshalProposals(t, gone, rewordParse))
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
