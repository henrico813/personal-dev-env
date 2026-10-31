package cmd

import (
	"bytes"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
	"unsafe"

	"pde-comment-review/internal/popup"
	"pde-comment-review/internal/review"
)

// Opens a pseudo-terminal sized like the phone popup. The screen reads keys
// from and draws on terminal; the test types into and reads from master.
func openFakeTerminal(t *testing.T) (master, terminal *os.File) {
	t.Helper()
	master, err := os.OpenFile("/dev/ptmx", os.O_RDWR, 0)
	if err != nil {
		t.Fatal(err)
	}
	var number uint32
	var unlock int32
	_, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		master.Fd(), syscall.TIOCGPTN, uintptr(unsafe.Pointer(&number)))
	if errno != 0 {
		t.Fatal(errno)
	}
	_, _, errno = syscall.Syscall(syscall.SYS_IOCTL,
		master.Fd(), syscall.TIOCSPTLCK, uintptr(unsafe.Pointer(&unlock)))
	if errno != 0 {
		t.Fatal(errno)
	}
	terminal, err = os.OpenFile("/dev/pts/"+strconv.Itoa(int(number)), os.O_RDWR, 0)
	if err != nil {
		t.Fatal(err)
	}
	size := struct{ rows, cols, x, y uint16 }{rows: 14, cols: 37}
	_, _, errno = syscall.Syscall(syscall.SYS_IOCTL,
		terminal.Fd(), syscall.TIOCSWINSZ, uintptr(unsafe.Pointer(&size)))
	if errno != 0 {
		t.Fatal(errno)
	}
	t.Cleanup(func() { master.Close(); terminal.Close() })
	return master, terminal
}

func waitForScreen(t *testing.T, master *os.File, want string) {
	t.Helper()
	found := make(chan bool, 1)
	go func() {
		var screen bytes.Buffer
		buf := make([]byte, 256)
		for !bytes.Contains(screen.Bytes(), []byte(want)) {
			n, err := master.Read(buf)
			if err != nil {
				found <- false
				return
			}
			screen.Write(buf[:n])
		}
		found <- true
	}()
	select {
	case ok := <-found:
		if !ok {
			t.Fatalf("screen closed before showing %q", want)
		}
	case <-time.After(5 * time.Second):
		t.Fatalf("%q was never shown", want)
	}
}

func waitForFile(t *testing.T, path string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for {
		if _, err := os.Stat(path); err == nil {
			return
		}
		if time.Now().After(deadline) {
			t.Fatalf("%s never appeared", path)
		}
		time.Sleep(10 * time.Millisecond)
	}
}

func typeKeys(t *testing.T, master *os.File, keys string) {
	t.Helper()
	if _, err := master.WriteString(keys); err != nil {
		t.Fatal(err)
	}
}

// The owner once left ten notes on another session's review. Each answer
// must reach disk when its key is pressed, only in the review that was
// opened, and never from a key typed before the screen drew.
func TestAnswerSavedOnlyInChosenReview(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// Keeps a live Herdr out of the test.
	t.Setenv("HERDR_ENV", "")
	repo := t.TempDir()
	writeFile(t, filepath.Join(repo, "cart.go"), cartGo)
	for _, name := range []string{"other", "cart"} {
		p := review.Batch{
			Version:  review.Version,
			Repo:     repo,
			Requests: []review.FeedbackRequest{cutCurrency, rewordTotal},
		}
		if _, err := review.AddBatch(name, p); err != nil {
			t.Fatal(err)
		}
	}
	// A slow fake git, so a key can be typed while the screen gets ready.
	bin := t.TempDir()
	started := filepath.Join(bin, "started")
	script := "#!/bin/sh\ntouch " + started + "\nsleep 0.3\nexit 1\n"
	writeFile(t, filepath.Join(bin, "git"), script)
	if err := os.Chmod(filepath.Join(bin, "git"), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	master, terminal := openFakeTerminal(t)

	done := make(chan error, 1)
	go func() { done <- answerReview("cart", terminal, terminal, 0) }()
	// Step 1: while git runs, type a key meant for another program.
	waitForFile(t, started)
	typeKeys(t, master, "c")
	// Step 2: keep the first item.
	waitForScreen(t, master, "1/2 cut? cart.go")
	typeKeys(t, master, "o")
	// Step 3: read the review before quitting, to show o saved it at once.
	waitForScreen(t, master, "2/2 reword? cart.go")
	cart, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	typeKeys(t, master, "q")
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("answerReview() error = %v", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("the screen did not close on q")
	}

	want := map[int]review.Answer{1: {Action: review.ActionKeep, Batch: 1}}
	if !reflect.DeepEqual(cart.Answers, want) {
		t.Errorf("review cart saved %+v, want %+v", cart.Answers, want)
	}
	other, err := review.OpenReview("other")
	if err != nil {
		t.Fatal(err)
	}
	if len(other.Answers) != 0 {
		t.Errorf("review other saved %+v, want nothing", other.Answers)
	}
}

// A note saved in the popup must show and stay in a Herdr tab open on the
// same review.
func TestConsistentTabPopupStateOnOpen(t *testing.T) {
	t.Setenv("HERDR_ENV", "")
	inPopup := startReview(t, cartGo, validApproval, nil)
	master, terminal := openFakeTerminal(t)
	done := make(chan error, 1)
	go func() { done <- answerReview("cart", terminal, terminal, 0) }()
	waitForScreen(t, master, "1/3 cut? cart.go")

	// The owner saves notes on two requests in the popup.
	if err := inPopup.SaveNote(1, "keep Currency"); err != nil {
		t.Fatal(err)
	}
	if err := inPopup.SaveNote(2, "shorter"); err != nil {
		t.Fatal(err)
	}
	// In the tab, the owner adds " for now" to the first note.
	typeKeys(t, master, "m")
	waitForScreen(t, master, "note for AI: keep Currency")
	typeKeys(t, master, " for now\r")
	waitForScreen(t, master, "note: keep Currency for now")
	typeKeys(t, master, "q")
	if err := <-done; err != nil {
		t.Fatalf("answerReview() error = %v", err)
	}

	saved, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	want := map[int]string{1: "keep Currency for now", 2: "shorter"}
	got := map[int]string{}
	for id, n := range saved.Notes {
		got[id] = n.Text
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("saved notes = %q, want %q", got, want)
	}
}

// The AI moves a finished review's folder aside to start the next one.
// Pressing q must still close the old screen without an error.
func TestQuitWorksAfterReviewIsMoved(t *testing.T) {
	t.Setenv("HERDR_ENV", "")
	r := startReview(t, cartGo, validApproval, nil)
	master, terminal := openFakeTerminal(t)
	done := make(chan error, 1)
	go func() { done <- answerReview("cart", terminal, terminal, 0) }()
	waitForScreen(t, master, "1/3 cut? cart.go")

	if err := os.Rename(r.Dir, r.Dir+".old"); err != nil {
		t.Fatal(err)
	}
	typeKeys(t, master, "q")

	select {
	case err := <-done:
		if err != nil {
			t.Errorf("answerReview() error = %v, want nil", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("the screen did not close on q")
	}
}

// The last answer writes every answer into the code, with no question.
// An AI can add a round while the screen is open; that round must
// open, or its item could never be answered and nothing would be
// written.
func TestScreenAppliesNewBatchAfterRefusal(t *testing.T) {
	installRepoChecker(t)
	t.Setenv("HERDR_ENV", "")
	r := startReview(t, cartGo, `{"approved": [{"file": "cart.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"}], "avoid": {}}`, nil)
	saveAnswers(t, r, map[int]review.Answer{1: {Action: review.ActionAccept}, 2: {Action: review.ActionAccept}})
	master, terminal := openFakeTerminal(t)

	done := make(chan error, 1)
	go func() { done <- answerReview("cart", terminal, terminal, 0) }()
	waitForScreen(t, master, "3/3 keep? cart.go")
	p := review.Batch{Version: review.Version, Repo: r.Repo(), Requests: []review.FeedbackRequest{{ID: 4, File: "cart.go",
		Suggestion: review.SuggestCut, Old: "// Reports whether every price is above zero.\n"}}}
	if _, err := review.AddBatch("cart", p); err != nil {
		t.Fatal(err)
	}
	typeKeys(
		t,
		master,
		"o",
	)
	waitForScreen(t, master, "4/4 cut? cart.go")
	// The screen shows the comment the AI just sent wrapped like this only
	// after it asks the checker again.
	waitForScreen(t, master, "-// above zero.")
	typeKeys(t, master, "o")
	waitForScreen(
		t,
		master,
		"Done.",
	)
	typeKeys(t, master, "q")
	select {
	case err := <-done:
		if err != nil {
			t.Fatalf("answerReview() error = %v", err)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("the screen did not close on q")
	}

	want := `package cart

const (
	Max      = 50
	Currency = "USD"
)

// Returns the sum of prices, in cents.
func Total(prices []int) int { return 0 }

// Reports whether a price is too high.
func TooHigh(price int) bool { return price > 1000 }

// Reports whether every price is above zero.
func Valid(prices []int) bool { return true }
`
	if got, _ := os.ReadFile(filepath.Join(r.Repo(), "cart.go")); string(got) != want {
		t.Errorf("cart.go =\n%s\nwant\n%s", got, want)
	}
	applied, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	if applied.AppliedBatch != 2 {
		t.Errorf("applied round = %d, want 2", applied.AppliedBatch)
	}
}

// The Herdr popup closes when the screen exits, so a missing checker must
// stay on screen with its pde-installer hint instead of closing it.
func TestMissingCheckerKeepsScreenOpen(t *testing.T) {
	t.Setenv(
		"HERDR_ENV",
		"",
	)
	r := startReview(
		t,
		cartGo,
		validApproval,
		nil,
	)
	saveAnswers(
		t,
		r,
		map[int]review.Answer{
			1: {Action: review.ActionAccept},
			2: {Action: review.ActionAccept},
		},
	)
	t.Setenv(
		"PATH",
		t.TempDir(),
	)
	master, terminal := openFakeTerminal(t)
	done := make(
		chan error,
		1,
	)
	go func() {
		done <- answerReview(
			"cart",
			terminal,
			terminal,
			0,
		)
	}()
	waitForScreen(
		t,
		master,
		"3/3 keep? cart.go",
	)

	typeKeys(
		t,
		master,
		"o",
	)

	waitForScreen(
		t,
		master,
		"not written",
	)
	// Scrolling up at the top draws the same screen again, with the reason.
	typeKeys(
		t,
		master,
		"k",
	)
	waitForScreen(
		t,
		master,
		"run pde-installer",
	)
	typeKeys(
		t,
		master,
		"q",
	)
	if err := <-done; err != nil {
		t.Fatalf(
			"answerReview() error = %v",
			err,
		)
	}
	if got, _ := os.ReadFile(filepath.Join(r.Repo(), "cart.go")); string(got) != cartGo {
		t.Errorf(
			"cart.go =\n%s\nwant it unchanged",
			got,
		)
	}
}

// The owner switches between the review and their session, so reviews
// must save state.
func TestReopenWritesFullyAnsweredReview(t *testing.T) {
	installRepoChecker(t)
	t.Setenv(
		"HERDR_ENV",
		"",
	)
	r := startReview(
		t,
		cartGo,
		validApproval,
		nil,
	)
	saveAnswers(
		t,
		r,
		map[int]review.Answer{
			1: {Action: review.ActionKeep},
			2: {Action: review.ActionAccept},
			3: {Action: review.ActionKeep},
		},
	)

	openUntilDone(t)
	// Reopening once more must find the answers already written.
	openUntilDone(t)

	written, err := review.OpenReview("cart")
	if err != nil {
		t.Fatal(err)
	}
	if written.AppliedBatch != 1 {
		t.Errorf(
			"written round = %d, want 1",
			written.AppliedBatch,
		)
	}
}

func openUntilDone(t *testing.T) {
	t.Helper()
	master, terminal := openFakeTerminal(t)
	done := make(
		chan error,
		1,
	)
	go func() {
		done <- answerReview(
			"cart",
			terminal,
			terminal,
			0,
		)
	}()
	waitForScreen(
		t,
		master,
		"Done.",
	)
	typeKeys(
		t,
		master,
		"q",
	)
	if err := <-done; err != nil {
		t.Fatalf(
			"answerReview() error = %v",
			err,
		)
	}
}

// Puts this checkout's comment checker first on PATH, since the screen runs
// the installed one.
func installRepoChecker(t *testing.T) {
	t.Helper()
	buildRepoCheckerOnPath(t)
}

// Two functions with the same comment must each show their own code.
func TestScreenLocatesCodeOrExplainsWhy(t *testing.T) {
	buildRepoCheckerOnPath(t)
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	const code = "package cart\n\n// Same.\nfunc First() {\n\t// Inside.\n}\n" +
		"\n\n\n\n\n\n// Same.\nfunc Second() {\n\t// Inside.\n}\n"
	const missing = "the checker cannot find this comment exactly once in cart.go"
	const stale = "cart.go changed after the screen read it"
	tests := []struct {
		name         string
		names        [2]string
		old, changed string
		want         [2]string
	}{
		{
			name:  "named duplicates show their own code",
			names: [2]string{"First", "Second"},
			old:   "// Same.\n",
			want:  [2]string{"func First() {", "func Second() {"},
		},
		{
			name: "unnamed duplicates show an error",
			old:  "\t// Inside.\n",
			want: [2]string{missing, missing},
		},
		{
			name:    "file shortened after the screen read it",
			names:   [2]string{"First", "Second"},
			old:     "// Same.\n",
			changed: "package cart\n",
			want:    [2]string{stale, stale},
		},
	}
	for _, tt := range tests {
		t.Run(
			tt.name,
			func(t *testing.T) {
				repo := t.TempDir()
				writeFile(t, filepath.Join(repo, "cart.go"), code)
				p := review.Batch{
					Version: review.Version,
					Repo:    repo,
				}
				for i, name := range tt.names {
					request := review.FeedbackRequest{
						ID:         i + 1,
						File:       "cart.go",
						Name:       name,
						Old:        tt.old,
						Suggestion: review.SuggestKeep,
					}
					p.Requests = append(p.Requests, request)
				}
				r, err := review.BuildReviewWithBatch("twins", p)
				if err != nil {
					t.Fatal(err)
				}
				v := &popup.View{
					Requests: r.ListFeedbackRequests(),
					Repo:     repo,
					Cols:     100,
					Rows:     40,
				}
				askCheckerForCommentLayout(r, v)
				if tt.changed != "" {
					writeFile(t, filepath.Join(repo, "cart.go"), tt.changed)
				}
				for i, want := range tt.want {
					v.Show(i, time.Now())
					screen := strings.Join(v.BuildScreen(), "\n")

					if !strings.Contains(screen, want) {
						t.Errorf("item %d screen lacks %q:\n%s", i+1, want, screen)
					}
				}
			},
		)
	}
}

// The screen opened past the code above a comment, which hid the def line
// that names a Python docstring's function. Notes sit above the def, so the
// def stays right on top of its docstring.
func TestScreenShowsDefAboveDocstring(t *testing.T) {
	codeAbove := strings.Repeat("RATE = 1\n", 20) + "\n\n"
	codeBelow := strings.Repeat("    total += 1\n", 20)
	const docstring = "    \"\"\"Return the sum of prices.\"\"\"\n"
	tests := []struct {
		name string
		// The checker's name for the comment, which a draft sets. Without it
		// the screen finds the comment by its text.
		commentName string
		why         string
		code        string
		wantShown   string
	}{
		{
			name:        "why, then def of a docstring found by name",
			commentName: "total",
			why:         "Says the unit.",
			code:        codeAbove + "def total(prices):\n" + docstring + codeBelow,
			// The header row ends with ?, so the why note is the first row.
			wantShown: "?\n>why: Says the unit.\n def total(prices):\n-\"\"\"Return the sum of prices.\"\"\"\n",
		},
		{
			name:      "def over several lines found by text",
			code:      codeAbove + "def total(\n    prices,\n    tax,\n):\n" + docstring + codeBelow,
			wantShown: "\n def total(\n     prices,\n     tax,\n ):\n-\"\"\"Return",
		},
		{
			name:      "long note above a tall def",
			why:       "Says the unit, the rounding, and what an empty list returns.",
			code:      codeAbove + "def total(\n    prices,\n    tax,\n    rounding,\n):\n" + docstring + codeBelow,
			wantShown: "?\n>why: Says the unit, the rounding,",
		},
		{
			name: "def taller than the screen",
			code: codeAbove + "def total(\n" +
				strings.Repeat("    price,\n", 12) +
				"):\n" + docstring + codeBelow,
			wantShown: "\n ):\n-\"\"\"Return the sum of prices.\"\"\"\n+(cut)\n",
		},
	}
	for _, tt := range tests {
		t.Run(
			tt.name,
			func(t *testing.T) {
				screen := showCutOnPhoneScreen(
					t,
					"cart.py",
					tt.code,
					review.FeedbackRequest{
						ID:         1,
						File:       "cart.py",
						Name:       tt.commentName,
						Old:        docstring,
						Why:        tt.why,
						Suggestion: review.SuggestCut,
					},
				)

				if !strings.Contains(screen, tt.wantShown) {
					t.Errorf(
						"screen lacks %q:\n%s",
						tt.wantShown,
						screen,
					)
				}
			},
		)
	}
}

// A Go comment's code sits below it. Showing the code above, as for a
// Python docstring, would push the Go comment down the screen.
func TestGoScreenStartsAtComment(t *testing.T) {
	code := "package cart\n\n" +
		strings.Repeat("var rate = 1\n", 20) +
		"\n// Returns the sum.\nfunc Total() int {\n" +
		strings.Repeat("\ttotal++\n", 20) +
		"}\n"

	screen := showCutOnPhoneScreen(
		t,
		"cart.go",
		code,
		review.FeedbackRequest{
			ID:         1,
			File:       "cart.go",
			Old:        "// Returns the sum.\n",
			Suggestion: review.SuggestCut,
		},
	)

	// The header row ends with ?, so the comment must be the next row.
	if want := "?\n-// Returns the sum.\n+(cut)\n func Total"; !strings.Contains(screen, want) {
		t.Errorf(
			"screen lacks %q:\n%s",
			want,
			screen,
		)
	}
}

// Returns the 37x14 phone screen, without colors, for a review that asks to
// cut one comment in file. The real checker says where the comment sits.
func showCutOnPhoneScreen(
	t *testing.T,
	file string,
	code string,
	request review.FeedbackRequest,
) string {
	t.Helper()
	buildRepoCheckerOnPath(t)
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	repo := t.TempDir()
	writeFile(
		t,
		filepath.Join(
			repo,
			file,
		),
		code,
	)
	r, err := review.BuildReviewWithBatch(
		"cart",
		review.Batch{
			Version:  review.Version,
			Repo:     repo,
			Requests: []review.FeedbackRequest{request},
		},
	)
	if err != nil {
		t.Fatal(err)
	}
	v := &popup.View{
		Requests: r.ListFeedbackRequests(),
		Repo:     repo,
		Cols:     37,
		Rows:     14,
	}
	askCheckerForCommentLayout(
		r,
		v,
	)
	v.Show(0, time.Now())
	return regexp.MustCompile(`\x1b\[[0-9;]*m`).ReplaceAllString(
		strings.Join(v.BuildScreen(), "\n"),
		"",
	)
}
