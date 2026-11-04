package cache

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"prs/internal/github"
)

// Returns a store in a new cache folder for the rest of the test, with the
// folder made, as a store has after its first write.
func openStore(t *testing.T) Store {
	t.Helper()
	t.Setenv(
		"XDG_CACHE_HOME",
		t.TempDir(),
	)
	store, err := Open()
	if err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(
		store.dir,
		0o700,
	); err != nil {
		t.Fatal(err)
	}
	return store
}

// Returns a pull request of example/repo updated at the given hour of
// 2026-10-31 UTC.
func makePullRequest(
	number,
	hour int,
) github.PullRequest {
	return github.PullRequest{
		Number:     number,
		Repository: github.Repository{NameWithOwner: "example/repo"},
		UpdatedAt: time.Date(
			2026,
			10,
			31,
			hour,
			0,
			0,
			0,
			time.UTC,
		),
	}
}

// A pull request that changed since its diff was saved must miss the
// cache, or the diff screen shows code that is no longer in the pull
// request.
func TestReadDiffMissesAfterUpdate(t *testing.T) {
	store := openStore(t)
	if err := store.WriteDiff(
		makePullRequest(
			12,
			9,
		),
		[]byte("old diff"),
	); err != nil {
		t.Fatal(err)
	}

	_, sameFound := store.ReadDiff(makePullRequest(
		12,
		9,
	))
	_, updatedFound := store.ReadDiff(makePullRequest(
		12,
		10,
	))

	if !sameFound || updatedFound {
		t.Errorf(
			"diff found for the same version %v and the updated one %v, want true and false",
			sameFound,
			updatedFound,
		)
	}
}

// Entries for closed pull requests and older update times must go, or the
// cache grows with every push. Entries at listed update times and the list
// itself must stay, or the next start fetches everything again.
func TestRemoveUnlistedKeepsOnlyCurrentFiles(t *testing.T) {
	store := openStore(t)
	listed := makePullRequest(
		12,
		10,
	)
	older := makePullRequest(
		12,
		9,
	)
	closed := makePullRequest(
		7,
		9,
	)
	for _, pr := range []github.PullRequest{
		listed,
		older,
		closed,
	} {
		if err := store.WriteDiff(
			pr,
			[]byte("diff"),
		); err != nil {
			t.Fatal(err)
		}
	}
	if err := store.WriteList(List{PullRequests: []github.PullRequest{listed}}); err != nil {
		t.Fatal(err)
	}

	store.RemoveUnlisted([]github.PullRequest{listed})

	list, err := store.ReadList()
	_, listedFound := store.ReadDiff(listed)
	_, olderFound := store.ReadDiff(older)
	_, closedFound := store.ReadDiff(closed)
	got := fmt.Sprintf(
		"list %v with %d, listed %v, older %v, closed %v",
		err == nil,
		len(list.PullRequests),
		listedFound,
		olderFound,
		closedFound,
	)
	want := "list true with 1, listed true, older false, closed false"
	if got != want {
		t.Errorf(
			"cache holds %s, want %s",
			got,
			want,
		)
	}
}

// A cache written by an earlier build of prs must still read after an
// update. If a field's name in the files changes, every saved list and
// entry reads as empty until each pull request changes again.
func TestStoreReadsCacheFromEarlierBuild(t *testing.T) {
	store := openStore(t)
	pr := makePullRequest(
		12,
		10,
	)
	// The files as an earlier build wrote them.
	for name, text := range map[string]string{
		"list.json": `{"fetchedAt": "2026-10-31T10:05:00Z", "pullRequests": [{"number": 12,
		  "title": "Fix login", "repository": {"nameWithOwner": "example/repo"},
		  "updatedAt": "2026-10-31T10:00:00Z"}]}`,
		nameEntry(pr) + ".json": `{"body": "Fixes login.", "files": [{"path": "a.go"}],
		  "feedback": {"comments": [{"body": "Looks good."}], "reviews": [{"state": "APPROVED"}],
		  "threads": [{"isResolved": true, "path": "b.go", "line": 3, "comments": [{"body": "Why?"}]}]}}`,
	} {
		if err := os.WriteFile(
			filepath.Join(
				store.dir,
				name,
			),
			[]byte(text),
			0o600,
		); err != nil {
			t.Fatal(err)
		}
	}

	list, listErr := store.ReadList()
	extras, found := store.ReadExtras(pr)

	feedback := extras.Feedback
	if listErr != nil || len(list.PullRequests) != 1 || !found || len(extras.Files) != 1 ||
		len(feedback.Comments) != 1 || len(feedback.Reviews) != 1 || len(feedback.Threads) != 1 ||
		len(feedback.Threads[0].Comments) != 1 {
		t.Fatalf(
			"cache reads as list %+v (%v) and entry %+v (%v)",
			list,
			listErr,
			extras,
			found,
		)
	}
	got := fmt.Sprint([]string{
		list.FetchedAt.UTC().Format(time.RFC3339),
		list.PullRequests[0].Title,
		extras.Body,
		extras.Files[0].Path,
		feedback.Comments[0].Body,
		feedback.Reviews[0].State,
		feedback.Threads[0].Path,
		feedback.Threads[0].Comments[0].Body,
		fmt.Sprint(feedback.Threads[0].IsResolved),
	})
	want := "[2026-10-31T10:05:00Z Fix login Fixes login. a.go Looks good. APPROVED b.go Why? true]"
	if got != want {
		t.Errorf(
			"cache reads as %s, want %s",
			got,
			want,
		)
	}
}
