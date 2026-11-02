package app

import (
	"strings"
	"testing"
)

// A failed GitHub call must say which call failed and show gh's reason.
// Otherwise an empty list or a blank page looks like there is nothing to
// read.
func TestRunSaysWhatFailedAndWhy(t *testing.T) {
	tests := []struct {
		name       string
		answer     string
		args       []string
		want       string
		wantReason string
	}{
		{
			name:       "PR list with nothing saved",
			answer:     "fail",
			args:       nil,
			want:       "Could not get your pull requests from GitHub.",
			wantReason: failedReason,
		},
		{
			name:   "this folder's repository",
			answer: "refuse",
			args: []string{
				"12",
			},
			want:       "Could not find this folder's GitHub repository.",
			wantReason: refusedReason,
		},
		{
			name:   "PR summary screen",
			answer: "refuse",
			args: []string{
				"_summary",
				"example/repo",
				"12",
			},
			want:       "Could not get #12.",
			wantReason: refusedReason,
		},
		{
			name:   "comments screen",
			answer: "fail",
			args: []string{
				"_comments",
				"example/repo",
				"12",
			},
			want:       "Could not get the comments of #12.",
			wantReason: failedReason,
		},
		{
			name:   "diff screen",
			answer: "refuse",
			args: []string{
				"_diff",
				"example/repo",
				"12",
			},
			want:       "Could not get the diff of #12.",
			wantReason: refusedReason,
		},
		{
			name:   "one pull request",
			answer: "fail",
			args: []string{
				"example/repo",
				"12",
			},
			want:       "Could not get #12.",
			wantReason: failedReason,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output := runPRs(
				t,
				tt.answer,
				tt.args...,
			)

			for _, want := range []string{
				tt.want,
				tt.wantReason,
			} {
				if !strings.Contains(
					output,
					want,
				) {
					t.Errorf(
						"output omits %q:\n%s",
						want,
						output,
					)
				}
			}
		})
	}
}

// The installer checks each new build of prs by running prs _keys, which
// must work with no gh, HOME or cache folder. If it fails, every install
// rolls back all the tools it built.
func TestRunPrintsKeysInBareEnvironment(t *testing.T) {
	for name, value := range map[string]string{
		"PATH":           t.TempDir(),
		"HOME":           "",
		"XDG_CACHE_HOME": "",
	} {
		t.Setenv(
			name,
			value,
		)
	}

	code, output := runInProcess("_keys")

	if code != 0 || !strings.Contains(
		output,
		"PR summary screen",
	) {
		t.Errorf(
			"prs _keys exited %d and printed\n%s\nwant 0 and the PR list screen's help",
			code,
			output,
		)
	}
}
