package app

import (
	"slices"
	"strings"
	"testing"
	"time"
)

// A tap on a help text row word must run that word's key on each fzf
// screen, with Esc and Enter under fzf's names. Otherwise a tap runs the
// wrong key or cannot leave a search.
func TestPrintTapActionRunsWordKey(t *testing.T) {
	tests := []struct {
		name   string
		screen string
		prompt string
		column string
		want   string
	}{
		{
			name:   "Esc while a search is open",
			screen: "list",
			prompt: "/ ",
			column: "2",
			want:   "trigger(esc)\n",
		},
		{
			name:   "Enter while a search is open",
			screen: "list",
			prompt: "/ ",
			column: "13",
			want:   "trigger(enter)\n",
		},
		{
			name:   "o pane on the comments screen",
			screen: "comments",
			column: "10",
			want:   "trigger(o)\n",
		},
		{
			name:   "o close in the file picker",
			screen: "picker",
			column: "10",
			want:   "trigger(o)\n",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			for name, value := range map[string]string{
				"FZF_COLUMNS":             "50",
				"FZF_LINES":               "40",
				"FZF_PROMPT":              tt.prompt,
				"FZF_TOTAL_COUNT":         "1",
				"FZF_CLICK_FOOTER_COLUMN": tt.column,
			} {
				t.Setenv(
					name,
					value,
				)
			}

			output := runPRs(
				t,
				"works",
				"_tap",
				tt.screen,
				t.TempDir(),
			)

			if output != tt.want {
				t.Errorf(
					"tap printed %q, want %q",
					output,
					tt.want,
				)
			}
		})
	}
}

// fzf binds single keys, so gg is two presses of g. A second g within a
// second must go to the first row, and a lone or late g must do nothing,
// or one stray g jumps to the top.
func TestPrintFirstOnQuickSecondG(t *testing.T) {
	tests := []struct {
		name    string
		presses []time.Duration
		want    string
	}{
		{
			name: "second g soon after",
			presses: []time.Duration{
				0,
				300 * time.Millisecond,
			},
			want: "first\n",
		},
		{
			name: "second g after a pause",
			presses: []time.Duration{
				0,
				2 * time.Second,
			},
			want: "",
		},
		{
			name: "lone g",
			presses: []time.Duration{
				0,
			},
			want: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			start := time.Now()
			var output strings.Builder

			// Each press runs _g on the same folder, at its time after the first.
			for _, after := range tt.presses {
				output.Reset()
				a := &prsApp{
					stdout: &output,
					now:    start.Add(after),
				}
				a.printFirstOnSecondG([]string{
					dir,
				})
			}

			if output.String() != tt.want {
				t.Errorf(
					"the last g printed %q, want %q",
					output.String(),
					tt.want,
				)
			}
		})
	}
}

// Each fzf screen's / must free every one-character key the screen binds,
// and Esc must bind them all again. Otherwise a letter in a search runs its
// action instead of typing, or stays dead after the search ends.
func TestAddSearchKeysUnbindsUntilEsc(t *testing.T) {
	a := &prsApp{self: "prs"}
	for name, actions := range map[string]map[string]string{
		"PR list screen": a.buildListActions("dir"),
		"comments screen": a.buildCommentActions(
			"dir",
			"12",
		),
		"file picker": a.buildFilePickerActions(
			"dir",
			"12",
		),
	} {
		t.Run(name, func(t *testing.T) {
			freed := findBetween(
				actions["/"],
				"unbind(",
			)
			bound := findBetween(
				actions["esc"],
				"rebind(",
			)

			letters := strings.Split(
				freed,
				",",
			)
			for _, keys := range []map[string]string{
				sharedActions,
				actions,
			} {
				for key := range keys {
					if len(key) == 1 && !slices.Contains(
						letters,
						key,
					) {
						t.Errorf(
							"/ frees %q, which leaves out %q",
							freed,
							key,
						)
					}
				}
			}
			if bound != freed {
				t.Errorf(
					"Esc binds %q, want %q, the letters / freed",
					bound,
					freed,
				)
			}
		})
	}
}

// Returns the text of action between start and the next ")".
func findBetween(
	action,
	start string,
) string {
	_, after, _ := strings.Cut(
		action,
		start,
	)
	inside, _, _ := strings.Cut(
		after,
		")",
	)
	return inside
}
