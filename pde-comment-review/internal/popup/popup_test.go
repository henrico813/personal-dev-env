package popup

import (
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"

	"pde-comment-review/internal/review"
)

// Herdr's 40x16 popup leaves 37x14 inside its border.
const popupCols, popupRows = 37, 14

func newView(t *testing.T, code string, requests ...review.FeedbackRequest) *View {
	t.Helper()
	repo := t.TempDir()
	if err := os.WriteFile(filepath.Join(repo, "wire.go"), []byte(code), 0o644); err != nil {
		t.Fatal(err)
	}
	v := &View{Repo: repo, Cols: popupCols, Rows: popupRows, Settle: Settle}
	for _, request := range requests {
		v.Requests = append(v.Requests, review.RequestWithFeedback{Request: request, Batch: 1})
	}
	return v
}

var colorCodes = regexp.MustCompile(`\x1b\[[0-9;]*m`)

// A row wider than the popup wraps and pushes the label off a phone
// screen. Row 1 names the review because the owner once left ten notes on
// another session's review. Agent text must not reach the terminal as
// control or right-to-left characters, which could redraw or reorder it.
func TestScreenFitsPhonePopup(t *testing.T) {
	code := "package wire\n\n" +
		strings.Repeat("var unrelatedVariableWithAVeryLongName = map[string]int{\"first\": 1, \"second\": 2}\n", 30) +
		"var docs = \"https://go.dev/doc/comment#doc-comments-for-constants-and-variables\"\n" +
		"// Shared reports whether code is one of the shared codes, which no tool may declare as its own.\n" +
		"func Shared() bool { return true }\n"
	v := newView(t, code, review.FeedbackRequest{
		ID: 11, File: "wire.go", Suggestion: review.SuggestReword,
		Old: "// Shared reports whether code is one of the shared codes, which no tool may declare as its own.\n",
		New: "// Reports whether no tool may declare code.\n",
		Why: "Restates the name\x1b[2J\u202e. 判定:✅名前と同じことを言っているだけなので、読み手には何も伝わりません。",
	})
	v.Origin = review.Origin{
		RepoName: "personal-dev-env-sandbox-copy",
		Branch:   "feat/review-apply-screen-画面表示",
	}

	checkFits := func(lines []string) {
		t.Helper()
		if len(lines) != popupRows {
			t.Fatalf("screen has %d lines, want %d:\n%s", len(lines), popupRows, strings.Join(lines, "\n"))
		}
		for i, line := range lines {
			plain := colorCodes.ReplaceAllString(line, "")
			if width := displayWidth(plain); width > popupCols {
				t.Errorf("line %d is %d columns, want at most %d: %q", i+1, width, popupCols, line)
			}
		}
		// Each of 面表示 takes two columns, so the label is 36 wide, not 33.
		if want := "personal-dev-env-sa… · review…面表示"; lines[0] != want {
			t.Errorf("row 1 = %q, want %q", lines[0], want)
		}
	}

	v.Show(0, time.Now())
	lines := v.BuildScreen()

	checkFits(lines)
	screen := colorCodes.ReplaceAllString(strings.Join(lines, "\n"), "")
	// ✅ takes two columns, so its row holds 13 more characters, not 14.
	for _, want := range []string{
		"1/1 reword? wire.go",
		"why: Restates the name^[[2J\\u202e.",
		"判定:✅名前と同じことを言っている\n",
		"+// Reports whether no tool may",
	} {
		if !strings.Contains(screen, want) {
			t.Errorf("screen does not show %q:\n%s", want, screen)
		}
	}
	if lines[popupRows-2] == "" {
		t.Errorf("row %d is blank: the screen opened past the end of the item", popupRows-1)
	}

	v.ShowNotWritten("nothing was written: wire.go: item 11 changed in the file after it was added\x1b[2J")
	resultRows := v.BuildScreen()

	checkFits(resultRows)
	resultText := colorCodes.ReplaceAllString(
		strings.Join(
			resultRows,
			"\n",
		),
		"",
	)
	for _, want := range []string{
		"not written",
		"was added^[[2J",
	} {
		if !strings.Contains(
			resultText,
			want,
		) {
			t.Errorf(
				"result screen does not show %q:\n%s",
				want,
				resultText,
			)
		}
	}
}

// On a phone a key is easily pressed twice. The second press must not
// answer, or start a note on, the item the first press moved to.
func TestAnswerKeysWaitAfterNewRequest(t *testing.T) {
	v := newView(t, "package wire\n\n// One.\nvar one = 1\n\n// Two.\nvar two = 2\n",
		review.FeedbackRequest{ID: 1, File: "wire.go", Suggestion: review.SuggestCut, Old: "// One.\n"},
		review.FeedbackRequest{ID: 2, File: "wire.go", Suggestion: review.SuggestCut, Old: "// Two.\n"})
	start := time.Now()
	v.ShowNextUnanswered(0, start)

	first := v.PressKey("o", start.Add(Settle))
	v.Requests[0].Feedback.Answer = &review.Answer{Action: review.ActionKeep, Batch: 1}
	v.ShowNextUnanswered(1, start.Add(Settle))
	quick := start.Add(Settle + 50*time.Millisecond)
	again := v.PressKey("o", quick)
	v.PressKey("m", quick)
	typing := v.typingFor == SaveNote
	later := v.PressKey("o", start.Add(2*Settle))

	if first != Keep {
		t.Errorf("press after the pause = %v, want Keep", first)
	}
	if again != None || typing || v.Index != 1 {
		t.Errorf("quick o = %v, m started a note = %v, on item %d; want None, false, item 2",
			again, typing, v.Index+1)
	}
	if later != Keep {
		t.Errorf("press after the next pause = %v, want Keep", later)
	}
}

// Verifies that removed code offers only drop, since keeping its comment
// would record a cut the owner never chose.
func TestRemovedCodeOffersOnlyDrop(t *testing.T) {
	v := newView(
		t,
		"package wire\n",
		review.FeedbackRequest{
			ID:          1,
			File:        "wire.go",
			Name:        "Dial",
			Old:         "// Dials the server.\n",
			Suggestion:  review.SuggestCut,
			CodeRemoved: true,
		},
	)
	start := time.Now()
	v.Show(0, start)

	screen := colorCodes.ReplaceAllString(strings.Join(v.BuildScreen(), "\n"), "")
	keep := v.PressKey("o", start.Add(Settle))
	cut := v.PressKey("c", start.Add(Settle))

	if want := "Code removed. Drop this comment?\n-// Dials the server."; !strings.Contains(screen, want) {
		t.Errorf(
			"screen does not show %q:\n%s",
			want,
			screen,
		)
	}
	if keep != None || cut != Cut {
		t.Errorf(
			"o = %v, c = %v; want None, Cut",
			keep,
			cut,
		)
	}
}
