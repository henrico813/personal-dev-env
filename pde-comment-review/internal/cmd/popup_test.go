package cmd

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"net"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// Herdr shows one popup at a time. A comment review left open in another
// workspace once blocked every PR approval, and the owner could not tell
// which popup to close. A record left behind would name a popup that is
// gone.
func TestPopupRecordNamesItsReview(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// Keeps a live Herdr out of the test.
	t.Setenv("HERDR_ENV", "")
	t.Setenv(reviewNameEnv, "cart")
	t.Setenv("HERDR_PLUGIN_CONTEXT_JSON", `{"workspace_id":"w1"}`)
	repo := t.TempDir()
	writeFile(t, filepath.Join(repo, "cart.go"), cartGo)
	p := review.Proposals{Version: review.Version, Repo: repo, Items: []review.Item{cutCurrency}}
	if _, err := review.AddRound("cart", p); err != nil {
		t.Fatal(err)
	}
	r, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	if err := r.SetOrigin(review.Origin{RepoName: "shop", Branch: "feat/fix-cart-rounding-v2"}); err != nil {
		t.Fatal(err)
	}
	master, terminal := openFakeTerminal(t)

	done := make(chan error, 1)
	go func() { done <- runReviewPopup(terminal, terminal) }()
	// Step 1: the review is on screen; read the record.
	waitForScreen(t, master, "1/1 cut? cart.go")
	var record popupRecord
	data, err := os.ReadFile(findOpenPopupPath())
	if err == nil {
		err = json.Unmarshal(data, &record)
	}
	// Step 2: quit, then check the record is gone.
	typeKeys(t, master, "q")
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("runReviewPopup() error = %v", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("the popup did not close on q")
	}

	if err != nil {
		t.Fatalf("reading the record while the popup ran: %v", err)
	}
	// A refused approval shows the name on a phone, so it has 19 characters.
	want := popupRecord{PID: os.Getpid(), Workspace: "w1", Name: "Review: fix-c…ng-v2"}
	if record != want {
		t.Errorf("record = %+v, want %+v", record, want)
	}
	if _, err := os.Stat(findOpenPopupPath()); !errors.Is(err, fs.ErrNotExist) {
		t.Errorf("record still exists after the popup closed: %v", err)
	}
}

// Starts a fake Herdr socket with the given tabs open. It returns the
// requests it got, each as what the request asked for and its review name.
func startFakeHerdr(
	t *testing.T,
	tabLabels map[string]string,
) (socket string, received func() []string) {
	t.Helper()
	socket = filepath.Join(t.TempDir(), "herdr.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { listener.Close() })
	var mu sync.Mutex
	var requests []string
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				return
			}
			var request struct {
				Method string `json:"method"`
				Params struct {
					Entrypoint string            `json:"entrypoint"`
					Placement  string            `json:"placement"`
					TabID      string            `json:"tab_id"`
					Label      string            `json:"label"`
					Env        map[string]string `json:"env"`
				} `json:"params"`
			}
			line, _ := bufio.NewReader(conn).ReadBytes('\n')
			if json.Unmarshal(line, &request) == nil {
				asked := request.Method
				if request.Params.Entrypoint != "" {
					asked += " " + request.Params.Entrypoint + " as " + request.Params.Placement
				}
				if request.Params.TabID != "" {
					asked += " " + request.Params.TabID
				}
				if request.Params.Label != "" {
					asked += " to " + request.Params.Label
				}
				mu.Lock()
				requests = append(requests, fmt.Sprintf("%s %q", asked, request.Params.Env[reviewNameEnv]))
				mu.Unlock()
			}
			result := map[string]any{}
			switch request.Method {
			case "plugin.pane.open":
				result["plugin_pane"] = map[string]any{"pane": map[string]any{"tab_id": "w1:t9"}}
			case "tab.list":
				tabs := []map[string]string{}
				for id, label := range tabLabels {
					tabs = append(tabs, map[string]string{"tab_id": id, "label": label})
				}
				result["tabs"] = tabs
			}
			reply, _ := json.Marshal(map[string]any{"id": "comment-review", "result": result})
			fmt.Fprintf(conn, "%s\n", reply)
			conn.Close()
		}
	}()
	received = func() []string {
		mu.Lock()
		defer mu.Unlock()
		return slices.Clone(requests)
	}
	return socket, received
}

// Ctrl+b e and Ctrl+b E must open the review tool.
func TestActionOpensReviewInAskedPane(t *testing.T) {
	tests := []struct {
		name        string
		actionID    string
		args        []string
		reviews     []string          // oldest first
		tabs        map[string]string // Herdr's open tabs by ID, with labels
		want        []string
		wantRefused bool
	}{
		{
			name:        "outside the action",
			args:        []string{"open"},
			reviews:     []string{"cart"},
			wantRefused: true,
		},
		{
			name:     "none waiting",
			actionID: "open",
			args:     []string{"open"},
			want:     []string{`notification.show ""`},
		},
		{
			name:     "one waiting",
			actionID: "open",
			args:     []string{"open"},
			reviews:  []string{"cart"},
			want:     []string{`plugin.pane.open popup as popup "cart"`},
		},
		{
			name:     "several waiting",
			actionID: "open",
			args:     []string{"open"},
			reviews:  []string{"older", "newer"},
			want:     []string{`plugin.pane.open popup as popup ""`},
		},
		{
			name:     "Ctrl+b E opens the tab",
			actionID: "open-tab",
			args:     []string{"open", "--tab"},
			reviews:  []string{"cart"},
			tabs:     map[string]string{"w1:t1": "1"},
			want: []string{
				`tab.list ""`,
				`plugin.pane.open tab as tab "cart"`,
				`tab.focus w1:t9 ""`,
				`tab.rename w1:t9 to comment review ""`,
			},
		},
		{
			name:     "Ctrl+b E again moves to the open tab",
			actionID: "open-tab",
			args:     []string{"open", "--tab"},
			reviews:  []string{"cart"},
			tabs:     map[string]string{"w1:t1": "1", "w2:t3": "comment review"},
			want: []string{
				`tab.list ""`,
				`tab.focus w2:t3 ""`,
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			t.Setenv("HERDR_PLUGIN_ACTION_ID", tt.actionID)
			socket, received := startFakeHerdr(t, tt.tabs)
			t.Setenv("HERDR_SOCKET_PATH", socket)
			repo := t.TempDir()
			writeFile(t, filepath.Join(repo, "cart.go"), cartGo)
			start := time.Now()
			for i, name := range tt.reviews {
				p := review.Proposals{Version: review.Version, Repo: repo, Items: []review.Item{cutCurrency}}
				if _, err := review.AddRound(name, p); err != nil {
					t.Fatal(err)
				}
				// Rounds saved in one test can share a timestamp, so set them apart.
				added := start.Add(time.Duration(i) * time.Minute)
				round := filepath.Join(review.FindStateRoot(), name, "round-1.json")
				if err := os.Chtimes(round, added, added); err != nil {
					t.Fatal(err)
				}
			}

			// The command adds its tools' folders and settings to these, so
			// put them back when the test ends.
			for _, name := range []string{
				"PATH",
				"AQUA_ROOT_DIR",
				"AQUA_GLOBAL_CONFIG",
				"AQUA_CHECKSUMS_PATH",
			} {
				t.Setenv(name, os.Getenv(name))
			}

			exit := Main(tt.args)

			if (exit != 0) != tt.wantRefused {
				t.Errorf("Main(%q) = %d, want refused %v", tt.args, exit, tt.wantRefused)
			}
			if got := received(); !slices.Equal(got, tt.want) {
				t.Errorf("requests = %q, want %q", got, tt.want)
			}
		})
	}
}

// Several sessions' reviews once sat side by side, and the owner left ten
// notes on the wrong one. The list groups reviews under the agent session
// that added them, and leaves out applied reviews and removed checkouts.
func TestPickerGroupsWaitingReviewsBySession(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	start := time.Now()
	// Oldest first; beta renamed its session after alpha.
	reviews := []struct{ name, session, agent string }{
		{"alpha", "s1", "OC | Old title"},
		{"gamma", "s2", "OC | Other work"},
		{"beta", "s1", "OC | New title"},
		{"delta", "", ""},
		{"applied", "s2", "OC | Other work"},
		{"gone", "s1", "OC | New title"},
	}
	repos := map[string]string{}
	for i, saved := range reviews {
		repo := t.TempDir()
		repos[saved.name] = repo
		writeFile(t, filepath.Join(repo, "cart.go"), cartGo)
		p := review.Proposals{Version: review.Version, Repo: repo, Items: []review.Item{cutCurrency}}
		if _, err := review.AddRound(saved.name, p); err != nil {
			t.Fatal(err)
		}
		r, err := review.OpenReview(saved.name)
		if err != nil {
			t.Fatal(err)
		}
		origin := review.Origin{RepoName: "shop", Branch: saved.name, Session: saved.session, Agent: saved.agent}
		if err := r.SetOrigin(origin); err != nil {
			t.Fatal(err)
		}
		added := start.Add(time.Duration(i) * time.Minute)
		if err := os.Chtimes(filepath.Join(r.Dir, "round-1.json"), added, added); err != nil {
			t.Fatal(err)
		}
		if saved.name == "applied" {
			if err := r.SetApplied(); err != nil {
				t.Fatal(err)
			}
		}
	}
	if err := os.RemoveAll(repos["gone"]); err != nil {
		t.Fatal(err)
	}

	choices, err := listWaitingChoices()

	if err != nil {
		t.Fatal(err)
	}
	picker := popup.NewPicker(choices)
	picker.Cols, picker.Rows = 37, 14
	colorCodes := regexp.MustCompile(`\x1b\[[0-9;]*m`)
	var lines []string
	for _, line := range picker.BuildScreen() {
		lines = append(lines, colorCodes.ReplaceAllString(line, ""))
	}
	want := []string{
		"Comment reviews waiting",
		"OC | New title",
		"> shop · beta",
		"    beta · 0/1 answered",
		"  shop · alpha",
		"    alpha · 0/1 answered",
		"OC | Other work",
		"  shop · gamma",
		"    gamma · 0/1 answered",
		"Unknown session",
		"  shop · delta",
		"    delta · 0/1 answered",
		"",
		"j/k move Enter open q quit",
	}
	if !slices.Equal(lines, want) {
		t.Errorf("list =\n%s\nwant\n%s", strings.Join(lines, "\n"), strings.Join(want, "\n"))
	}
}
