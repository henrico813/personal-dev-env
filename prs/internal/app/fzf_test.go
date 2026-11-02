package app

import (
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"
)

// A tap on a help text row word must run that word's key on every fzf
// screen, Esc and Enter included, and a word that is not a key must run
// nothing. Otherwise a tap runs the wrong key, cannot leave a search, or
// makes fzf fail on an unknown key.
func TestPrintTapActionRunsWordKey(t *testing.T) {
	tests := []struct {
		name   string
		screen string
		prompt string
		total  string
		failed bool
		column string
		want   string
	}{
		{
			name:   "Esc while a search is open",
			screen: "list",
			prompt: "/ ",
			total:  "1",
			column: "2",
			want:   "trigger(esc)\n",
		},
		{
			name:   "o pane on the comments screen",
			screen: "comments",
			total:  "1",
			column: "10",
			want:   "trigger(o)\n",
		},
		{
			name:   "GitHub in the failed empty list's row",
			screen: "list",
			total:  "0",
			failed: true,
			column: "3",
			want:   "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			if tt.failed {
				writeFile(
					t,
					filepath.Join(
						dir,
						refreshErrorFile,
					),
					failedReason,
					0o600,
				)
			}
			for name, value := range map[string]string{
				"FZF_COLUMNS":             "50",
				"FZF_LINES":               "40",
				"FZF_PROMPT":              tt.prompt,
				"FZF_TOTAL_COUNT":         tt.total,
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
				dir,
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
		name        string
		pressedOnce bool
		ago         time.Duration
		want        string
	}{
		{
			name:        "second g soon after",
			pressedOnce: true,
			ago:         300 * time.Millisecond,
			want:        "first\n",
		},
		{
			name:        "second g after a pause",
			pressedOnce: true,
			ago:         2 * time.Second,
			want:        "",
		},
		{
			name: "lone g",
			want: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			dir := t.TempDir()
			if tt.pressedOnce {
				writeFile(
					t,
					filepath.Join(
						dir,
						"g",
					),
					time.Now().Add(-tt.ago).Format(time.RFC3339Nano),
					0o600,
				)
			}

			output := runPRs(
				t,
				"works",
				"_g",
				dir,
			)

			if output != tt.want {
				t.Errorf(
					"g printed %q, want %q",
					output,
					tt.want,
				)
			}
		})
	}
}

// Each fzf screen's / must free every one-letter key the screen binds, and
// Esc must bind them all again. Otherwise a letter in a search runs its
// action instead of typing, or stays dead after the search ends.
func TestAddSearchKeysFreesEveryLetter(t *testing.T) {
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
