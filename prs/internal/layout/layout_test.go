package layout

import "testing"

// The phone keeps 50 columns when its keyboard comes up, so the PR list
// screen sizes its side pane by rows, and wide screens put the side pane
// beside the list. A wrong choice hides the checks or leaves rows empty.
func TestChooseListWindowFitsScreen(t *testing.T) {
	tests := []struct {
		name    string
		columns int
		lines   int
		matches int
		want    string
	}{
		{
			name:    "keyboard down splits the screen",
			columns: 50,
			lines:   40,
			matches: 18,
			want:    "change-preview-window(down,19,border-top,wrap-word)",
		},
		{
			name:    "two matches give the details their rows",
			columns: 50,
			lines:   40,
			matches: 2,
			want:    "change-preview-window(down,34,border-top,wrap-word)",
		},
		{
			name:    "keyboard up shows the status strip",
			columns: 50,
			lines:   25,
			matches: 18,
			want:    "change-preview-window(down,3,border-top,nowrap,noinfo)",
		},
		{
			name:    "keyboard up with two matches shows the longer details",
			columns: 50,
			lines:   25,
			matches: 2,
			want:    "change-preview-window(down,19,border-top,wrap-word)",
		},
		{
			name:    "phone held sideways puts the side pane beside",
			columns: 88,
			lines:   15,
			matches: 18,
			want:    "change-preview-window(right,45%,border-left,wrap-word)",
		},
		{
			name:    "desktop puts the side pane beside",
			columns: 255,
			lines:   58,
			matches: 18,
			want:    "change-preview-window(right,50%,border-left,wrap-word)",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			state := FzfState{
				Columns: tt.columns,
				Lines:   tt.lines,
				Matches: tt.matches,
			}

			got, _ := ChooseListWindow(
				state,
				Result,
				false,
			)

			if got != tt.want {
				t.Errorf(
					"window is %q, want %q",
					got,
					tt.want,
				)
			}
		})
	}
}

// A side pane closed with o must stay closed when the keyboard comes up or
// the phone turns. Otherwise it reopens over the list after each resize.
func TestChooseListWindowStaysClosed(t *testing.T) {
	tests := []struct {
		name    string
		columns int
		lines   int
		want    string
	}{
		{
			name:    "upright, below the list",
			columns: 50,
			lines:   40,
			want:    "change-preview-window(down,19,border-top,wrap-word,hidden)",
		},
		{
			name:    "sideways, beside the list",
			columns: 88,
			lines:   15,
			want:    "change-preview-window(right,45%,border-left,wrap-word,hidden)",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			state := FzfState{
				Columns: tt.columns,
				Lines:   tt.lines,
				Matches: 18,
			}

			got, _ := ChooseListWindow(
				state,
				Resize,
				true,
			)

			if got != tt.want {
				t.Errorf(
					"window is %q, want %q",
					got,
					tt.want,
				)
			}
		})
	}
}

// The side pane prints the status strip or the longer details for its size, so
// it must run again when a search or a resize makes it cross 3 rows, and not
// on every typed key. Otherwise a phone shows the first 3 rows of the details
// without the checks, or a status strip in a tall side pane.
func TestChooseListWindowRefreshesCrossingStrip(t *testing.T) {
	tests := []struct {
		name         string
		event        Event
		previewLines int
		matches      int
		want         bool
	}{
		{
			name:         "a search grows the status strip to the longer details",
			event:        Result,
			previewLines: 3,
			matches:      2,
			want:         true,
		},
		{
			name:         "a search keeps the status strip",
			event:        Result,
			previewLines: 3,
			matches:      18,
			want:         false,
		},
		{
			name:         "the keyboard comes up and the details shrink to the status strip",
			event:        Resize,
			previewLines: 19,
			matches:      18,
			want:         true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			state := FzfState{
				Columns:      50,
				Lines:        25,
				Matches:      tt.matches,
				PreviewLines: tt.previewLines,
			}

			_, refresh := ChooseListWindow(
				state,
				tt.event,
				false,
			)

			if refresh != tt.want {
				t.Errorf(
					"refresh is %v, want %v",
					refresh,
					tt.want,
				)
			}
		})
	}
}

// The help text row must name the keys that work now and fit the screen.
// Otherwise a 37-column popup cuts it off, a search names letters that now
// type text, or an empty list offers keys that do nothing.
func TestBuildListKeysShowsUsableKeys(t *testing.T) {
	tests := []struct {
		name          string
		state         FzfState
		refreshFailed bool
		want          string
	}{
		{
			name: "37x14 popup",
			state: FzfState{
				Columns: 37,
				Lines:   14,
				Total:   18,
			},
			want: "l open  d diff  c comments  ? help",
		},
		{
			name: "searching",
			state: FzfState{
				Columns: 50,
				Lines:   25,
				Total:   18,
				Prompt:  SearchPrompt,
			},
			want: "Esc cancel  Enter open",
		},
		{
			name: "failed refresh",
			state: FzfState{
				Columns: 50,
				Lines:   25,
			},
			refreshFailed: true,
			want:          "GitHub failed  r refresh  q quit",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := BuildListKeys(
				tt.state,
				tt.refreshFailed,
			)

			if got != tt.want {
				t.Errorf(
					"help text row is %q, want %q",
					got,
					tt.want,
				)
			}
		})
	}
}

// The file picker keeps its list small so the side pane below gets the
// rows, 5 of them or 3 below 30 rows. A wrong size hides either the files
// or the diff.
func TestChoosePaneWindowSizesPane(t *testing.T) {
	tests := []struct {
		name  string
		lines int
		want  string
	}{
		{
			name:  "keyboard down keeps 5 rows",
			lines: 40,
			want:  "change-preview-window(down,32,border-top,nowrap,noinfo)",
		},
		{
			name:  "keyboard up keeps 3 rows",
			lines: 25,
			want:  "change-preview-window(down,19,border-top,nowrap,noinfo)",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			state := FzfState{
				Columns: 50,
				Lines:   tt.lines,
			}

			got := ChoosePaneWindow(
				state,
				FilePicker,
				false,
			)

			if got != tt.want {
				t.Errorf(
					"window is %q, want %q",
					got,
					tt.want,
				)
			}
		})
	}
}

// A fingertip covers several columns and often lands on the gap between
// two words. A tap on a word must run it, and a tap on a gap must run the
// nearer word, or you get the wrong screen.
func TestFindTappedKeyPicksNearestItem(t *testing.T) {
	keys := "l open  d diff  c comments  / find  ? help"
	tests := []struct {
		name   string
		column int
		want   string
	}{
		{
			name:   "on the word comments",
			column: 22,
			want:   "c",
		},
		{
			name:   "on the gap just after comments",
			column: 27,
			want:   "c",
		},
		{
			name:   "on the gap just before / find",
			column: 28,
			want:   "/",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FindTappedKey(
				keys,
				tt.column,
			)

			if got != tt.want {
				t.Errorf(
					"tap on column %d ran %q, want %q",
					tt.column,
					got,
					tt.want,
				)
			}
		})
	}
}
