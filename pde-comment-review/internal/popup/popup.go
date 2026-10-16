// Package popup shows one review item per screen, sized for Herdr's 40x16
// popup. It saves nothing.
package popup

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"

	"pde-comment-review/internal/comment"
	"pde-comment-review/internal/review"
)

const (
	red    = "\x1b[31m"
	green  = "\x1b[32m"
	yellow = "\x1b[33m"
	dim    = "\x1b[2m"
	bold   = "\x1b[1m"
	invert = "\x1b[7m"
	reset  = "\x1b[0m"
)

// How many lines of code the screen shows above and below a comment.
const codeLinesAround = 100

// How long answer and note keys are ignored after an item appears. On a
// phone a key is easily pressed twice, and the second press must not answer
// the next item.
const Settle = 600 * time.Millisecond

var decisionStatus = map[string]string{
	review.ActionKeep:   "kept",
	review.ActionCut:    "cut",
	review.ActionAccept: "accepted",
	review.ActionEdited: "edited",
}

var answerKeys = map[string]Action{
	"o": Keep,
	"c": Cut,
	"a": Accept,
}

// What a key asks the caller to do.
type Action int

const (
	None Action = iota // only the view changed
	Quit
	Keep
	Cut
	Accept
	SaveNote // save View.Typed as the item's note
	Open     // open the review the picker shows as chosen
	ApplyAll // write every answer into the code and approve the comments
)

type screen int

const (
	itemScreen   screen = iota
	renameScreen        // asks whether approvals move to the AI's renamed code
	applyScreen         // asks whether to apply and approve
	resultScreen        // shows what apply did
)

// What the screen shows: one entry, scrolled down scroll rows.
type View struct {
	Entries    []review.Entry
	Repo       string
	Origin     review.Origin
	Index      int
	scroll     int
	Cols, Rows int
	Settle     time.Duration
	notice     string // shown above the key hint until the next key
	typing     bool   // the owner is typing a note into Typed
	Typed      string
	shownAt    time.Time
	Renames    []string // the AI's "OLD=NEW" pairs, asked about before applying
	Files      []string // the files apply approves, listed on the apply screen
	screen     screen
	problem    string // why apply failed, shown under the question
	result     string // what apply did, shown on the result screen
}

func (v *View) Entry() review.Entry { return v.Entries[v.Index] }

// Shows entry index scrolled past the code above, so its proposal is in
// view first.
func (v *View) Show(index int, now time.Time) {
	v.Index = index
	v.typing = false
	v.screen = itemScreen
	v.problem = ""
	rows, focus := v.body()
	v.scroll = min(focus, max(0, len(rows)-v.room()))
	v.shownAt = now
}

// Shows the first entry from index on, wrapping around, that has no
// decision. An entry with only a note still waits for one. When every entry
// has one, it moves on to the questions before apply.
func (v *View) ShowNextUndecided(index int, now time.Time) {
	for i := range v.Entries {
		next := (index + i) % len(v.Entries)
		if v.Entries[next].Decision == nil {
			v.Show(next, now)
			return
		}
	}
	v.askToApply(now)
}

// Shows the questions after the last answer: the AI's renames, if any, then
// whether to apply and approve.
func (v *View) askToApply(now time.Time) {
	v.screen = applyScreen
	if len(v.Renames) > 0 {
		v.screen = renameScreen
	}
	v.problem = ""
	v.scroll = 0
	v.shownAt = now
}

// Shows why apply refused, under the question. If an item from a new round
// is on screen instead, a notice says to answer it first.
func (v *View) ShowProblem(problem string) {
	if v.screen == itemScreen {
		v.notice = "an AI added a round; answer it first"
		return
	}
	v.problem = problem
	v.scroll = 0
}

func (v *View) ShowResult(report string) {
	v.screen = resultScreen
	v.result = report
	v.scroll = 0
}

// Handles one key press; l and h wrap around at the last and first item.
func (v *View) PressKey(key string, now time.Time) Action {
	v.notice = ""
	if key == "\x03" { // Ctrl-c
		return Quit
	}
	if v.typing {
		return v.typeNote(key)
	}
	// q, j and k work on every screen; the other keys depend on the screen.
	switch key {
	case "q":
		return Quit
	case "j":
		rows, _ := v.body()
		if v.scroll < len(rows)-v.room() {
			v.scroll++
		}
	case "k":
		v.scroll = max(v.scroll-1, 0)
	}
	if v.screen != itemScreen {
		return v.answerQuestion(key, now)
	}
	switch key {
	case "l":
		v.Show((v.Index+1)%len(v.Entries), now)
	case "h":
		v.Show((v.Index+len(v.Entries)-1)%len(v.Entries), now)
	case "o", "c", "a", "m":
		if now.Sub(v.shownAt) < v.Settle {
			v.notice = "one moment, then press again"
			return None
		}
		if key == "m" {
			v.typing = true
			v.Typed = v.Entry().Note
			return None
		}
		return answerKeys[key]
	}
	return None
}

// Takes y or n on the rename and apply screens. Like answer keys, they are
// ignored at first, so a y pressed twice cannot answer both questions.
func (v *View) answerQuestion(key string, now time.Time) Action {
	if v.screen == resultScreen {
		return None
	}
	if key != "y" && key != "n" {
		return None
	}
	if now.Sub(v.shownAt) < v.Settle {
		v.notice = "one moment, then press again"
		return None
	}
	if key == "n" {
		// Without the renames, apply refuses the old names' approvals as
		// gone, so n on either question goes back to the items.
		v.Show(0, now)
		return None
	}
	if v.screen == renameScreen {
		v.screen = applyScreen
		v.shownAt = now
		return None
	}
	if v.screen == applyScreen {
		return ApplyAll
	}
	return None
}

// Handles a key while the owner types a note. The note starts as the saved
// one, so Enter alone changes nothing, and an empty note removes it.
func (v *View) typeNote(key string) Action {
	switch key {
	case "\r", "\n":
		v.typing = false
		return SaveNote
	case "\x7f", "\x08": // Backspace
		_, size := utf8.DecodeLastRuneInString(v.Typed)
		v.Typed = v.Typed[:len(v.Typed)-size]
	default:
		r, _ := utf8.DecodeRuneInString(key)
		if unicode.IsPrint(r) {
			v.Typed += key
		}
	}
	return None
}

// How many body rows fit between the two header rows and the key hint.
func (v *View) room() int { return max(1, v.Rows-3) }

// Returns every row of the screen. No row is wider than Cols, since a row
// the terminal wraps would push the header off a phone screen.
func (v *View) BuildScreen() []string {
	left, status := v.header()
	if v.scroll > 0 {
		left += fmt.Sprintf(" ↑%d", v.scroll) // rows above, reached with k
	}
	width := max(0, v.Cols-len(status)-1)
	title := pad(clip(escapeControlCharacters(left), width), width)
	lines := []string{
		fitLabel(v.Origin, v.Cols),
		bold + title + " " + invert + status + reset,
	}
	rows, _ := v.body()
	shown := rows[min(v.scroll, len(rows)):]
	shown = shown[:min(len(shown), v.room())]
	// The "more" line covers the last body row, so it counts as hidden.
	if below := len(rows) - v.scroll - v.room(); below > 0 {
		shown[len(shown)-1] = dim + clip(fmt.Sprintf("  ... %d more (j)", below+1), v.Cols) + reset
	}
	lines = append(lines, shown...)
	for len(lines) < v.Rows-1 {
		lines = append(lines, "")
	}
	if v.notice != "" {
		lines[len(lines)-1] = yellow + clip(v.notice, v.Cols) + reset
	}
	return append(lines, v.footer())
}

// Returns row 2, which says which item, question or result is on screen.
func (v *View) header() (left, status string) {
	switch v.screen {
	case resultScreen:
		return "applied", "done"
	case renameScreen, applyScreen:
		return fmt.Sprintf("all %d answered", len(v.Entries)), "y/n"
	}
	e := v.Entry()
	status = "?"
	switch {
	case e.Decision != nil:
		status = decisionStatus[e.Decision.Action]
	case e.Note != "":
		status = "note"
	}
	left = fmt.Sprintf("%d/%d %s? %s", v.Index+1, len(v.Entries), e.Item.Proposal, filepath.Base(e.Item.File))
	return left, status
}

func (v *View) footer() string {
	if v.typing {
		prompt := "note for AI: "
		// The end of a long note stays visible, so what was just typed shows.
		width := max(1, v.Cols-len(prompt))
		typed := lastColumns(escapeControlCharacters(v.Typed), width)
		return prompt + typed
	}
	switch v.screen {
	case renameScreen:
		return dim + clip("y move approvals  n stop", v.Cols) + reset
	case applyScreen:
		return dim + clip("y apply and approve  n back", v.Cols) + reset
	case resultScreen:
		return dim + clip("j/k scroll q close", v.Cols) + reset
	}
	return dim + clip("a accept o keep c cut m note q quit", v.Cols) + reset
}

// Returns the entry's rows with its code around it, and the index of the
// first row after the code above, which Show scrolls to.
func (v *View) body() ([]string, int) {
	if v.screen != itemScreen {
		return v.questionRows(), 0
	}
	e := v.Entry()
	width := max(10, v.Cols-1) // one column for the -, + or > mark
	var rows []string
	add := func(color, mark, text string) {
		text = escapeControlCharacters(strings.ReplaceAll(text, "\t", "  "))
		prefix := mark
		for _, row := range fold(text, width) {
			rows = append(rows, color+prefix+row+reset)
			prefix = " "
		}
	}
	addComment := func(color, mark, text string) {
		for _, row := range comment.WrapForScreen(text, width) {
			add(color, mark, row)
		}
	}
	before, after, err := review.ReadCodeAround(v.Repo, e.Item, codeLinesAround)
	for _, line := range before {
		add(dim, " ", line)
	}
	focus := len(rows)
	if err != nil {
		add(yellow, ">", "cannot show the code: "+err.Error())
	}
	if e.Note != "" {
		add(yellow+bold, ">", "note: "+e.Note)
	}
	if len(e.Item.AlsoAt) > 0 {
		add(yellow, ">", "also at: "+strings.Join(e.Item.AlsoAt, ", "))
	}
	if e.Item.Why != "" {
		add(yellow, ">", "why: "+e.Item.Why)
	}
	switch e.Item.Proposal {
	case review.ProposeReword:
		addComment(red, "-", e.Item.Old)
		addComment(green, "+", e.Item.New)
	case review.ProposeCut:
		addComment(red, "-", e.Item.Old)
		add(green, "+", "(cut)")
	default:
		add(yellow, ">", "no change proposed")
		addComment("", " ", e.Item.Old)
	}
	for _, line := range after {
		add(dim, " ", line)
	}
	return rows, focus
}

// Returns the rows of the rename, apply or result screen, with what the owner
// must read first at the top.
func (v *View) questionRows() []string {
	var rows []string
	add := func(color, text string) {
		for _, row := range fold(escapeControlCharacters(text), max(10, v.Cols-1)) {
			rows = append(rows, color+" "+row+reset)
		}
	}
	addProblem := func() {
		if v.problem != "" {
			for _, line := range strings.Split(v.problem, "\n") {
				add(red, line)
			}
		}
	}
	switch v.screen {
	case renameScreen:
		add(bold, "These functions were renamed: keep your approvals on the new names? y/n")
		addProblem()
		for _, pair := range v.Renames {
			before, after, _ := strings.Cut(pair, "=")
			add("", before+" -> "+after)
		}
	case applyScreen:
		add(bold, "Apply and approve these comments? y/n")
		addProblem()
		add(yellow, "Comments inside function bodies, and comments that share a name, can't be approved or protected.")
		add("", "y writes your answers, then approves every comment in these files. It writes nothing if an agent changed a comment you have not seen:")
		for _, file := range v.Files {
			add(dim, file)
		}
	case resultScreen:
		for _, line := range strings.Split(strings.TrimSuffix(v.result, "\n"), "\n") {
			add("", line)
		}
	}
	return rows
}

// Returns "repo · branch" in at most width columns, cutting the middle of a
// long branch so both its ends stay readable.
func fitLabel(o review.Origin, width int) string {
	label := escapeControlCharacters(o.Label())
	if o.Branch == "" || displayWidth(label) <= width {
		return clip(label, width)
	}
	repo := escapeControlCharacters(o.RepoName)
	branch := escapeControlCharacters(o.BranchWithoutPrefix())
	const maxRepoColumns = 20
	if displayWidth(repo) > maxRepoColumns {
		repo = clip(repo, maxRepoColumns-1) + "…"
	}
	room := width - displayWidth(repo+" · ")
	return clip(repo+" · "+cutMiddle(branch, room), width)
}

// The popup record's name, which a refused PR approval shows on a phone, has
// at most 19 characters.
const recordNameColumns = 19

// Returns the name the popup record gives a review: its label after
// "Review: ", or only its branch when the label is too long.
func BuildRecordName(o review.Origin) string {
	const prefix = "Review: "
	room := recordNameColumns - len(prefix)
	name := escapeControlCharacters(o.Label())
	if displayWidth(name) > room && o.Branch != "" {
		name = escapeControlCharacters(o.BranchWithoutPrefix())
	}
	return prefix + cutMiddle(name, room)
}

// Returns s in at most width columns, cutting its middle so both ends stay
// readable.
func cutMiddle(s string, width int) string {
	if displayWidth(s) <= width || width < 3 {
		return clip(s, width)
	}
	head := clip(s, (width-1)/2)
	return head + "…" + lastColumns(s, width-1-displayWidth(head))
}

// Writes out control and format characters, such as ^[ for Esc, because
// agent-written text could otherwise clear the screen or reorder itself.
func escapeControlCharacters(text string) string {
	var out strings.Builder
	for _, r := range text {
		switch {
		case r < 0x20:
			out.WriteString("^" + string('@'+r))
		case r == 0x7f:
			out.WriteString("^?")
		case unicode.IsControl(r) || unicode.Is(unicode.Cf, r):
			fmt.Fprintf(&out, "\\u%04x", r)
		default:
			out.WriteRune(r)
		}
	}
	return out.String()
}

// Breaks one line into rows of at most width columns; rows after the first
// start with two spaces.
func fold(line string, width int) []string {
	if displayWidth(line) <= width {
		return []string{line}
	}
	var rows []string
	for _, row := range comment.WrapWords(strings.Fields(line), width-2) {
		// A row still too wide, such as a URL, is cut into pieces.
		for row != "" {
			piece := clip(row, width-2)
			rows = append(rows, "  "+piece)
			row = strings.TrimLeft(row[len(piece):], " ")
		}
	}
	if len(rows) == 0 {
		return []string{""} // only spaces
	}
	rows[0] = strings.TrimPrefix(rows[0], "  ")
	return rows
}

func displayWidth(s string) int {
	width := 0
	for _, r := range s {
		width += runeWidth(r)
	}
	return width
}

// Returns 2 for characters a terminal draws two columns wide, such as 面
// and ✅. A few narrow symbols in these ranges count as 2 too, which only
// leaves a row one column short.
func runeWidth(r rune) int {
	switch {
	case r >= 0x1100 && r <= 0x115f, r >= 0x231a && r <= 0x27bf, r >= 0x2b1b && r <= 0x2b55,
		r >= 0x2e80 && r <= 0xa4cf, r >= 0xac00 && r <= 0xd7a3, r >= 0xf900 && r <= 0xfaff,
		r >= 0xfe30 && r <= 0xfe4f, r >= 0xff00 && r <= 0xff60, r >= 0xffe0 && r <= 0xffe6,
		r >= 0x1f000 && r <= 0x1f2ff, r >= 0x1f300 && r <= 0x1faff, r >= 0x20000 && r <= 0x3fffd:
		return 2
	}
	return 1
}

// Returns the start of s that fits in width columns.
func clip(s string, width int) string {
	used := 0
	for i, r := range s {
		if used += runeWidth(r); used > width {
			return s[:i]
		}
	}
	return s
}

func lastColumns(s string, width int) string {
	used := 0
	for i := len(s); i > 0; {
		r, size := utf8.DecodeLastRuneInString(s[:i])
		if used += runeWidth(r); used > width {
			return s[i:]
		}
		i -= size
	}
	return s
}

func pad(s string, width int) string {
	return s + strings.Repeat(" ", max(0, width-displayWidth(s)))
}
