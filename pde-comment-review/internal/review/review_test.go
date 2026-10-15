package review

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
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

// Later, the owner's answers are written over each "old" in its Go file,
// with no AI to check the result. If "old" is not one whole comment found
// once, that write could change code, change the wrong comment, or leave
// half a comment behind. Bad files must be refused when added, before the
// owner reviews them.
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
			name: "code in new",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeReword, Old: rewordParse.Old, New: "// Reads the config.\nfunc Parse() {}\n"}),
			want: "new: line 2 is not a // comment",
		},
		{
			name: "comment repeated",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeCut, Old: "// Deprecated: use ParseConfig.\n"}),
			want: "found 2 times, at lines 13, 16",
		},
		{
			name: "comment mid-line",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeCut, Old: "// on Linux, so the default is above them.\n"}),
			want: "does not start a line",
		},
		{
			name: "start of a comment",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeCut, Old: "\t// Used when the config file sets no port. Ports below 1024 need root\n"}),
			want: "only part of a comment",
		},
		{
			name: "end of a comment",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeCut, Old: "\t// on Linux, so the default is above them.\n"}),
			want: "only part of a comment",
		},
		{
			name: "comment above a directive",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeKeep, Old: "// Names one setting in the config file.\n"}),
		},
		{
			name: "comment inside a string",
			data: marshalProposals(t, repo, Item{ID: 1, File: "config.go", Proposal: ProposeCut, Old: "// Sample settings, as the help text prints them.\n"}),
			want: "inside a string",
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
// them. The Python prototype kept each round in its own file, so answers
// were merged by hand. A new round must replace only its own items and
// close their notes; other notes stay open, as does a new note on it.
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
