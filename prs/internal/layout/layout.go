// Package layout turns fzf's screen size into each fzf screen's side pane
// and help text row, starting at ChooseListWindow and ChoosePaneWindow.
// The help text row is fzf's footer, one line of key names such as
// "l open" that a tap can run.
package layout

import (
	"fmt"
	"os"
	"strconv"
	"strings"
	"unicode/utf8"
)

// Holds what fzf tells a command about its screen in FZF_ variables.
type FzfState struct {
	Lines        int
	Columns      int
	Matches      int
	Total        int
	PreviewLines int
	// fzf counts it from 1 at the help text row's first character, after its
	// 2-column indent, and sends nothing for a tap on the indent.
	ClickColumn int
	Prompt      string
}

// Says which fzf event asked for a layout.
type Event string

const (
	Result Event = "result"
	Resize Event = "resize"
)

// Says which fzf screen a layout is for.
type Screen int

const (
	// The zero Screen is no screen, so a Screen left unset never passes as one.
	prListScreen Screen = iota + 1
	FilePicker
	CommentsScreen
)

// The prompt a list shows while a search is open.
const SearchPrompt = "/ "

// The rows an fzf screen gives neither the list nor the side pane below
// it, which are the prompt, the help text row and the side pane's top
// border.
const frameRows = 3

// The fewest rows a short screen gives the longer details in place of the
// status strip.
const minLongDetailsRows = 8

// The help text row while a search is open, when letters type text.
const SearchKeys = "Esc cancel  Enter open"

// The long forms of the fzf screens' help text rows. Each screen starts
// with its own, until its first layout fits the row to the screen.
const (
	ListKeys       = "l open  d diff  c comments  / find  ? help"
	CommentsKeys   = "l open  o pane  h back  / find  ? help"
	FilePickerKeys = "l open  o close  / find  ? help"
)

// Returns the screen fzf describes in the environment.
func ReadFzfState() FzfState {
	number := func(name string) int {
		value, _ := strconv.Atoi(os.Getenv(name))
		return value
	}
	return FzfState{
		Lines:        number("FZF_LINES"),
		Columns:      number("FZF_COLUMNS"),
		Matches:      number("FZF_MATCH_COUNT"),
		Total:        number("FZF_TOTAL_COUNT"),
		PreviewLines: number("FZF_PREVIEW_LINES"),
		ClickColumn:  number("FZF_CLICK_FOOTER_COLUMN"),
		Prompt:       os.Getenv("FZF_PROMPT"),
	}
}

// Returns whether a search is open.
func (s FzfState) IsSearching() bool {
	return s.Prompt == SearchPrompt
}

// Returns the percent of columns the side pane takes beside the list, or
// 0 when it goes below. Over 100 columns it goes beside, and so does the
// PR list screen's below 30 rows and over 80 columns, as on a phone held
// sideways.
func (s FzfState) choosePreviewPercent(screen Screen) int {
	switch {
	case s.Columns > 100 && screen == prListScreen:
		return 50
	case s.Columns > 100:
		return 60
	case s.Lines < 30 && s.Columns > 80 && screen == prListScreen:
		return 45
	}
	return 0
}

// Returns the list's width, which is narrower when the side pane is beside
// it. Help text rows and taps both use it, so a tap matches the row it
// lands on.
func (s FzfState) measureListWidth(screen Screen) int {
	return s.Columns - s.Columns*s.choosePreviewPercent(screen)/100
}

// Returns the fzf action that places the PR list screen's side pane, closed
// or not, and whether the side pane must run again. Below 30 rows and at 80
// columns or fewer the side pane holds the status strip, unless
// minLongDetailsRows or more fit below the matches.
func ChooseListWindow(
	s FzfState,
	event Event,
	closed bool,
) (
	string,
	bool,
) {
	hidden := ""
	if closed {
		hidden = ",hidden"
	}
	if percent := s.choosePreviewPercent(prListScreen); percent > 0 {
		return fmt.Sprintf(
			"change-preview-window(right,%d%%,border-left,wrap-word%s)",
			percent,
			hidden,
		), event == Resize
	}
	var preview int
	if s.Lines < 30 {
		preview = s.Lines - frameRows - max(
			s.Matches,
			3,
		)
		if preview < minLongDetailsRows {
			preview = 3
		}
	} else {
		list := min(
			(s.Lines-frameRows)/2,
			max(
				s.Matches,
				3,
			),
		)
		preview = s.Lines - frameRows - list
	}
	// The side pane prints the status strip or the longer details for its
	// size, so it runs again when it crosses 3 rows, but not on every key.
	crossed := (s.PreviewLines <= 3) != (preview <= 3)
	refresh := event == Resize || (event == Result && crossed)
	if preview <= 3 {
		return "change-preview-window(down,3,border-top,nowrap,noinfo" + hidden + ")", refresh
	}
	return fmt.Sprintf(
		"change-preview-window(down,%d,border-top,wrap-word%s)",
		preview,
		hidden,
	), refresh
}

// Returns the fzf action that places the side pane of the file picker or
// the comments screen, closed or not. At 100 columns or fewer the side
// pane goes below a list of 5 rows, or 3 below 30 rows.
func ChoosePaneWindow(
	s FzfState,
	screen Screen,
	closed bool,
) string {
	hidden := ""
	if closed {
		hidden = ",hidden"
	}
	// nowrap, since the text comes wrapped.
	if percent := s.choosePreviewPercent(screen); percent > 0 {
		return fmt.Sprintf(
			"change-preview-window(right,%d%%,border-left,nowrap%s)",
			percent,
			hidden,
		)
	}
	rows := 5
	if s.Lines < 30 {
		rows = 3
	}
	// noinfo, since the scroll position would cover the file's path.
	return fmt.Sprintf(
		"change-preview-window(down,%d,border-top,nowrap,noinfo%s)",
		s.Lines-frameRows-rows,
		hidden,
	)
}

// Returns the PR list screen's help text row. When the saved list is empty
// and a refresh failed, the row offers just r and q.
func BuildListKeys(
	s FzfState,
	refreshFailed bool,
) string {
	width := s.measureListWidth(prListScreen)
	switch {
	case s.IsSearching():
		return SearchKeys
	case s.Total > 0:
		return chooseFittingKeys(
			width,
			ListKeys,
			"l open  d diff  c comments  ? help",
		)
	case refreshFailed:
		return "GitHub failed  r refresh  q quit"
	}
	return chooseFittingKeys(
		width,
		"No open pull requests  r refresh  q quit",
		"No pull requests  r refresh  q quit",
	)
}

// Returns the help text row of the comments screen or the file picker,
// whose short form drops / find. The file picker's h closes it as o does,
// so its row names o and leaves h out.
func BuildPaneKeys(
	s FzfState,
	screen Screen,
) string {
	if s.IsSearching() {
		return SearchKeys
	}
	long := CommentsKeys
	if screen == FilePicker {
		long = FilePickerKeys
	}
	return chooseFittingKeys(
		s.measureListWidth(screen),
		long,
		strings.Replace(
			long,
			"  / find",
			"",
			1,
		),
	)
}

// Returns the key of the help text row word that a tap on column col hit,
// or "" for none. A tap on a gap counts for the nearer word, since a
// fingertip covers several columns.
func FindTappedKey(
	keys string,
	col int,
) string {
	if col < 1 {
		return ""
	}
	start := 1
	for _, item := range strings.Split(
		keys,
		"  ",
	) {
		end := start + utf8.RuneCountInString(item) - 1
		if col <= end+1 {
			key, _, _ := strings.Cut(
				item,
				" ",
			)
			return key
		}
		start = end + 3
	}
	return ""
}

// Returns long if it fits a list width columns wide, or else short. fzf
// starts the help text row 2 columns in.
func chooseFittingKeys(
	width int,
	long,
	short string,
) string {
	if width >= utf8.RuneCountInString(long)+2 {
		return long
	}
	return short
}
