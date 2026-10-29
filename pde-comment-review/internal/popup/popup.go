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
	Edit     // edit the item's comment in nvim
	ViewFile // open the item's file read-only at its comment
	Search   // search the repository for View.Typed
	ShowDiff // show the uncommitted changes in the review's files
)

type screen int

const (
	itemScreen   screen = iota
	resultScreen        // shows what writing the answers did
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
	typingFor  Action // SaveNote or Search while the owner types into Typed
	Typed      string
	shownAt    time.Time
	screen     screen
	result     string // what writing the answers did, or why it did not
	written    bool   // whether the answers reached the code
	// The first and last file line of each comment under review, by its ID. A
	// comment the checker did not find exactly once has no entry.
	CommentLinesByID map[int]review.CommentLines
	// Each comment's text split by the checker into parts the screen wraps,
	// keyed by that text. Text with no entry shows as its own lines.
	CommentRows map[string][]ScreenRow
}

// One row of the checker's show reply. A row with Words is wrapped after
// its marks; any other row is drawn as Fixed.
type ScreenRow struct {
	First string `json:"first"`
	Rest  string `json:"rest"`
	Words string `json:"words"`
	Fixed string `json:"fixed"`
}

// Wraps comment sentences with their marks, preserving code examples.
func wrapScreenRows(rows []ScreenRow, width int) []string {
	var lines []string
	for _, row := range rows {
		if row.Words == "" {
			lines = append(
				lines,
				row.Fixed,
			)
			continue
		}
		marks := max(
			len(row.First),
			len(row.Rest),
		)
		mark := row.First
		words := wrapWords(
			strings.Fields(row.Words),
			width-marks,
		)
		for _, line := range words {
			lines = append(
				lines,
				mark+line,
			)
			mark = row.Rest
		}
	}
	return lines
}

func (v *View) Entry() review.Entry { return v.Entries[v.Index] }

// Shows entry index scrolled past the code above, so its proposal is in
// view first.
func (v *View) Show(index int, now time.Time) {
	v.Index = index
	v.typingFor = None
	v.screen = itemScreen
	rows, focus := v.body()
	v.scroll = min(focus, max(0, len(rows)-v.room()))
	v.shownAt = now
}

// Shows the first entry from index on, wrapping around, that has no
// decision. An entry with only a note still waits for one. When every
// entry has one, it leaves the screen as it is, for the caller to
// write the answers.
func (v *View) ShowNextUndecided(index int, now time.Time) {
	for i := range v.Entries {
		next := (index + i) % len(v.Entries)
		if v.Entries[next].Decision == nil {
			v.Show(next, now)
			return
		}
	}
}

func (v *View) IsAllAnswered() bool {
	for _, e := range v.Entries {
		if e.Decision == nil {
			return false
		}
	}
	return true
}

func (v *View) ShowNotice(notice string) { v.notice = notice }

func (v *View) ShowDone() {
	v.screen = resultScreen
	v.result = "Done. Your answers are in the code. d shows the changes."
	v.written = true
	v.scroll = 0
}

func (v *View) ShowNotWritten(reason string) {
	v.screen = resultScreen
	v.result = reason
	v.written = false
	v.scroll = 0
}

// Handles one key press; l and h wrap around at the last and first item.
func (v *View) PressKey(key string, now time.Time) Action {
	v.notice = ""
	if key == "\x03" { // Ctrl-c
		return Quit
	}
	if v.typingFor != None {
		return v.typeText(key)
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
	if v.screen == resultScreen && key == "d" {
		return ShowDiff
	}
	if v.screen == resultScreen {
		return None
	}
	// Keeping, editing or viewing a comment needs its code.
	if v.Entry().Item.CodeRemoved && (key == "o" || key == "e" || key == "v") {
		v.notice = "The code is gone. c or a drops the comment, and m asks the AI for the code back."
		return None
	}
	switch key {
	case "l":
		v.Show((v.Index+1)%len(v.Entries), now)
	case "h":
		v.Show((v.Index+len(v.Entries)-1)%len(v.Entries), now)
	case "e":
		return Edit
	case "v":
		return ViewFile
	case "s":
		v.typingFor = Search
		v.Typed = ""
	case "o", "c", "a", "m":
		if now.Sub(v.shownAt) < v.Settle {
			v.notice = "one moment, then press again"
			return None
		}
		if key == "m" {
			v.typingFor = SaveNote
			v.Typed = v.Entry().Note
			return None
		}
		return answerKeys[key]
	}
	return None
}

// Handles a key while the owner types a note or a search. A note starts as
// the saved one, so Enter alone changes nothing, and an empty note removes
// it.
func (v *View) typeText(key string) Action {
	switch key {
	case "\r", "\n":
		action := v.typingFor
		v.typingFor = None
		return action
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
func (v *View) room() int { return max(1, v.Rows-2-len(v.footer())) }

// Returns every row of the screen. No row is wider than Cols, since a row
// the terminal wraps would push the header off a phone screen.
func (v *View) BuildScreen() []string {
	left, status := v.buildHeader()
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
	for len(lines) < v.Rows-len(v.footer()) {
		lines = append(lines, "")
	}
	if v.notice != "" {
		// A long notice, such as why nvim failed, wraps upward over the body.
		notice := fold(escapeControlCharacters(v.notice), v.Cols)
		rowsBelowHeader := len(lines) - 2
		notice = notice[max(0, len(notice)-rowsBelowHeader):]
		for i, row := range notice {
			lines[len(lines)-len(notice)+i] = yellow + row + reset
		}
	}
	return append(lines, v.footer()...)
}

// Returns row 2, which says which item or result is on screen.
func (v *View) buildHeader() (left, status string) {
	if v.screen == resultScreen {
		status = "not written"
		if v.written {
			status = "done"
		}
		left = fmt.Sprintf(
			"all %d answered",
			len(v.Entries),
		)
		return left, status
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

// Returns the key hint: two rows on the item screen, since its keys do not
// fit in one row of the phone popup.
func (v *View) footer() []string {
	hint := func(keys string) string { return dim + clip(keys, v.Cols) + reset }
	if v.screen == resultScreen {
		return []string{hint("d diff j/k scroll q close")}
	}
	if v.typingFor != None {
		prompt := "note for AI: "
		if v.typingFor == Search {
			prompt = "search: "
		}
		// The end of a long note stays visible, so what was just typed shows.
		width := max(1, v.Cols-len(prompt))
		typed := lastColumns(escapeControlCharacters(v.Typed), width)
		return []string{hint("type, then Enter"), prompt + typed}
	}
	return []string{hint("a accept o keep c cut e edit"), hint("m note v view s search q quit")}
}

// Returns the entry's rows with its code around it, and the index of the
// first row after the code above, which Show scrolls to.
func (v *View) body() ([]string, int) {
	width := max(10, v.Cols-1) // one column for the -, + or > mark
	var rows []string
	if v.screen == resultScreen {
		for _, line := range strings.Split(
			v.result,
			"\n",
		) {
			for _, row := range fold(
				escapeControlCharacters(line),
				width,
			) {
				rows = append(
					rows,
					" "+row,
				)
			}
		}
		return rows, 0
	}
	e := v.Entry()
	add := func(color, mark, text string) {
		text = escapeControlCharacters(strings.ReplaceAll(text, "\t", "  "))
		prefix := mark
		for _, row := range fold(text, width) {
			rows = append(rows, color+prefix+row+reset)
			prefix = " "
		}
	}
	addComment := func(color, mark, text string) {
		rows, ok := v.CommentRows[text]
		var lines []string
		if ok {
			lines = wrapScreenRows(rows, width)
		} else {
			// Without the checker's split, the screen shows the comment's own
			// lines and folds any that are too wide.
			lines = strings.Split(
				strings.TrimSuffix(text, "\n"),
				"\n",
			)
		}
		for _, row := range lines {
			add(
				color,
				mark,
				row,
			)
		}
	}
	var before, after []string
	var err error
	if !e.Item.CodeRemoved {
		before, after, err = review.ReadCodeAround(
			v.Repo,
			e.Item,
			v.CommentLinesByID,
			codeLinesAround,
		)
	}
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
	// The owner's own wording, once saved, takes the place of the AI's.
	switch {
	case e.Item.CodeRemoved:
		add(
			yellow,
			">",
			"Code removed. Drop this comment?",
		)
		addComment(
			red,
			"-",
			e.Item.Old,
		)
	case e.Decision != nil && e.Decision.Action == review.ActionEdited:
		addComment(red, "-", e.Item.Old)
		addComment(green, "+", e.Decision.Text)
	case e.Item.Proposal == review.ProposeReword:
		addComment(red, "-", e.Item.Old)
		addComment(green, "+", e.Item.New)
	case e.Item.Proposal == review.ProposeCut:
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

// Joins words with single spaces into lines of at most width characters.
// A word longer than width goes on a line of its own, so that line is
// wider than width.
func wrapWords(words []string, width int) []string {
	var lines []string
	line := ""
	for _, word := range words {
		if line != "" && utf8.RuneCountInString(line)+1+utf8.RuneCountInString(word) > width {
			lines = append(lines, line)
			line = ""
		}
		if line == "" {
			line = word
		} else {
			line += " " + word
		}
	}
	if line != "" {
		lines = append(lines, line)
	}
	return lines
}

// Breaks one line into rows of at most width columns; rows after the first
// start with two spaces.
func fold(line string, width int) []string {
	if displayWidth(line) <= width {
		return []string{line}
	}
	var rows []string
	for _, row := range wrapWords(strings.Fields(line), width-2) {
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
