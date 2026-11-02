package view

import (
	"testing"
	"time"

	"prs/internal/github"
)

// The comments screen's rows must show open code threads first, then the
// conversation oldest first, then resolved threads, so threads that need a
// reply are not lost among settled ones. An approval with no text is still
// a decision and must show, and your own unsent draft review must not,
// since nobody else can see it yet.
func TestRenderItemRowsOrdersAndFilters(t *testing.T) {
	now := time.Date(
		2026,
		10,
		31,
		12,
		0,
		0,
		0,
		time.UTC,
	)
	feedback := github.Feedback{
		Comments: []github.Comment{
			{
				Author:    github.Actor{Login: "carol"},
				CreatedAt: now.Add(-4 * time.Hour),
				Body:      "Looks good",
			},
		},
		Reviews: []github.Review{
			{
				Author:      github.Actor{Login: "bob"},
				State:       "APPROVED",
				SubmittedAt: now.Add(-2 * time.Hour),
			},
			{
				Author: github.Actor{Login: "alice"},
				State:  "PENDING",
				Body:   "unsent",
			},
		},
		Threads: []github.Thread{
			{
				IsResolved: true,
				Path:       "a.go",
				Line:       1,
				Comments: []github.Comment{
					{
						Author:    github.Actor{Login: "bob"},
						CreatedAt: now.Add(-3 * time.Hour),
					},
				},
			},
			{
				Path: "b.go",
				Line: 3,
				Comments: []github.Comment{
					{
						Author:    github.Actor{Login: "bob"},
						CreatedAt: now.Add(-time.Hour),
					},
				},
			},
		},
	}

	got := RenderItemRows(
		CollectItems(feedback),
		Style{
			Width: 50,
			Now:   now,
		},
	)

	want := "0\topen · 1h ago · bob · b.go:3\n" +
		"1\tcomment · 4h ago · carol  Looks good\n" +
		"2\tapproved · 2h ago · bob\n" +
		"3\tresolved · 3h ago · bob · a.go:1\n"
	if got != want {
		t.Errorf(
			"rows are\n%s\nwant\n%s",
			got,
			want,
		)
	}
}
