package cmd

import (
	"bytes"
	"os"
	"path/filepath"
	"reflect"
	"strconv"
	"syscall"
	"testing"
	"time"
	"unsafe"

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
		p := review.Proposals{
			Version: review.Version,
			Repo:    repo,
			Items:   []review.Item{cutCurrency, rewordTotal},
		}
		if _, err := review.AddRound(name, p); err != nil {
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

	want := map[int]review.Decision{1: {Action: review.ActionKeep, Round: 1}}
	if !reflect.DeepEqual(cart.Decisions, want) {
		t.Errorf("review cart saved %+v, want %+v", cart.Decisions, want)
	}
	other, err := review.OpenReview("other")
	if err != nil {
		t.Fatal(err)
	}
	if len(other.Decisions) != 0 {
		t.Errorf("review other saved %+v, want nothing", other.Decisions)
	}
}

// Apply from the screen writes the owner's answers into the code. An AI can
// add a round while the apply question is open; that round must open, or
// every y is refused and its item can never be answered.
func TestScreenAppliesNewRoundAfterRefusal(t *testing.T) {
	checker := findRepoChecker(t)
	home := t.TempDir()
	installed := filepath.Join(home, ".agents", "skills", "code-documentation", "scripts", "check_comments.py")
	if err := os.MkdirAll(filepath.Dir(installed), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(checker, installed); err != nil {
		t.Fatal(err)
	}
	t.Setenv("HOME", home)
	t.Setenv("HERDR_ENV", "")
	r := startReview(t, cartGo, `{"approved": [{"file": "cart.go", "name": "Valid", "text": "// Reports whether every price is above zero.\n"}], "avoid": {}}`, nil)
	setDecisions(t, r, map[int]review.Decision{1: {Action: review.ActionAccept}, 2: {Action: review.ActionAccept}})
	master, terminal := openFakeTerminal(t)

	done := make(chan error, 1)
	go func() { done <- answerReview("cart", terminal, terminal, 0) }()
	// Step 1: keep the last item, then go back with n and answer again.
	waitForScreen(t, master, "3/3 keep? cart.go")
	typeKeys(t, master, "o")
	waitForScreen(t, master, "all 3 answered")
	typeKeys(t, master, "n")
	waitForScreen(t, master, "1/3 cut? cart.go")
	typeKeys(t, master, "a")
	waitForScreen(t, master, "all 3 answered")
	// Step 2: an AI adds a round before the owner presses y.
	p := review.Proposals{Version: review.Version, Repo: r.Repo(), Items: []review.Item{{ID: 4, File: "cart.go",
		Proposal: review.ProposeCut, Old: "// Reports whether every price is above zero.\n"}}}
	if _, err := review.AddRound("cart", p); err != nil {
		t.Fatal(err)
	}
	typeKeys(t, master, "y")
	// Step 3: the new item opens; answer it and apply.
	waitForScreen(t, master, "4/4 cut? cart.go")
	typeKeys(t, master, "o")
	waitForScreen(t, master, "all 4 answered")
	typeKeys(t, master, "y")
	waitForScreen(t, master, "git -C")
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
	if applied.AppliedRound != 2 {
		t.Errorf("applied round = %d, want 2", applied.AppliedRound)
	}
}
