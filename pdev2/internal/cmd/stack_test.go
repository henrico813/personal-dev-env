package cmd

import (
	"encoding/json"
	"os"
	"reflect"
	"strings"
	"testing"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

// A fake gh that answers lookups with fixed JSON and logs every other call. It
// saves the merge-async body next to the call log, with .put added to the
// name.
func fakeStackGH(t *testing.T, prView, graphql, mergeStep, result string) (calls string) {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	approvaltest.Replace(t, &mergePoll, 10*time.Millisecond)
	approvaltest.Replace(t, &answerTimeout, 0)
	discardStderr(t)
	_, calls = approvaltest.FakeGH(t, `case "$*" in
"pr view"*) echo '`+prView+`' ;;
"api graphql"*) echo '`+graphql+`' ;;
"api --method PUT"*) cat > "$log.put"; `+mergeStep+` ;;
"api repos/"*) echo '`+result+`' ;;
*) echo "$*" >> "$log" ;;
esac`)
	return calls
}

// The lowest open PR of a GitHub stack, with the stacked PR 503 and the
// unstacked PR 509 based on its branch.
const (
	lowestView    = `{"number":502,"headRefName":"part-1"}`
	lowestGraphQL = `{"data":{"repository":{"pullRequest":{"state":"OPEN","baseRefName":"main","stack":{"baseRefName":"main"}},` +
		`"pullRequests":{"nodes":[{"number":503,"stack":{"size":4}},{"number":509,"stack":null}]}}}}`
	pendingMerge = `echo '{"status":"pending","details":{"uuid":"u1"}}'`
	mergedResult = `{"status":"merged","details":{"message":"merged","sha":"abc"}}`
)

// Returns the fake gh's call log, or an empty string when nothing was logged.
func readCalls(t *testing.T, calls string) string {
	t.Helper()
	data, err := os.ReadFile(calls)
	if err != nil && !os.IsNotExist(err) {
		t.Fatal(err)
	}
	return string(data)
}

// GitHub merges a stacked PR together with every open PR below it, so one
// approval of a mid-stack merge would merge PRs the owner never approved.
func TestMergeRefusesMidStackPR(t *testing.T) {
	fakeStackGH(t, `{"number":503,"headRefName":"part-2"}`,
		`{"data":{"repository":{"pullRequest":{"state":"OPEN","baseRefName":"part-1","stack":{"baseRefName":"main"}},"pullRequests":{"nodes":[]}}}}`,
		"", "")

	code := requestWrite([]string{"pr", "merge", "503", "--squash"})

	if code != exitRejected || len(approval.Queued()) != 0 {
		t.Fatalf("merge returned %d with %d requests waiting, want %d and none", code, len(approval.Queued()), exitRejected)
	}
}

// GitHub refuses gh pr merge for a PR in a stack, so approved stack merges
// failed. The merge must go to merge-async with the approved method and commit
// title.
func TestStackedMergeUsesMergeAsyncAPI(t *testing.T) {
	calls := fakeStackGH(t, lowestView, lowestGraphQL, pendingMerge, mergedResult)
	args := []string{"pr", "merge", "502", "--squash", "--subject", "Part 1 (#502)"}
	requestAndAnswer(t, args, approval.StateApproved)

	code := requestWrite(args)

	put, err := os.ReadFile(calls + ".put")
	if code != 0 || err != nil {
		t.Fatalf("merge returned %d, merge-async body %q (%v)", code, put, err)
	}
	var sent map[string]any
	if err := json.Unmarshal(put, &sent); err != nil {
		t.Fatal(err)
	}
	want := map[string]any{"merge_method": "squash", "merge_action": "direct_merge", "commit_title": "Part 1 (#502)"}
	if !reflect.DeepEqual(sent, want) {
		t.Errorf("merge-async body = %v, want %v", sent, want)
	}
	if log := readCalls(t, calls); strings.Contains(log, "pr merge") {
		t.Errorf("gh pr merge ran:\n%s", log)
	}
}

// A PR left on a merged PR's branch cannot merge into main. Only PRs outside
// the GitHub stack move, and only after the merge, since GitHub moves stacked
// PRs itself.
func TestMergeMovesUnstackedDependentsToBase(t *testing.T) {
	plainGraphQL := strings.Replace(lowestGraphQL, `"stack":{"baseRefName":"main"}`, `"stack":null`, 1)
	cases := []struct {
		name    string
		graphql string
		args    []string
		moved   bool
	}{
		{name: "stacked merge", graphql: lowestGraphQL, args: []string{"pr", "merge", "502", "--squash"}, moved: true},
		{name: "plain merge", graphql: plainGraphQL, args: []string{"pr", "merge", "502", "--squash"}, moved: true},
		{name: "auto merge not yet merged", graphql: plainGraphQL, args: []string{"pr", "merge", "502", "--squash", "--auto"}, moved: false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			calls := fakeStackGH(t, lowestView, tc.graphql, pendingMerge, mergedResult)
			requestAndAnswer(t, tc.args, approval.StateApproved)

			code := requestWrite(tc.args)

			log := readCalls(t, calls)
			moved := strings.Contains(log, "pr edit 509 --base main")
			if code != 0 || moved != tc.moved || strings.Contains(log, "pr edit 503") {
				t.Fatalf("merge returned %d with gh calls:\n%s\nwant #509 moved %v and #503 never", code, log, tc.moved)
			}
		})
	}
}

// A PR moved to main before its parent merged shows the parent's changes
// as its own. When GitHub refuses or fails the stack merge, the agent must
// see why and no PR may move.
func TestMergeStackedFailureReportedWithoutMoves(t *testing.T) {
	cases := []struct {
		name      string
		mergeStep string
		result    string
		reason    string
	}{
		{name: "refused", mergeStep: `echo '{"status":"failed","details":{"message":"Pull request is a draft"}}'; exit 1`, result: "", reason: "Pull request is a draft"},
		{name: "failed after pending", mergeStep: pendingMerge, result: `{"status":"failed","details":{"message":"Required status check failed"}}`, reason: "Required status check failed"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			calls := fakeStackGH(t, lowestView, lowestGraphQL, tc.mergeStep, tc.result)
			args := []string{"pr", "merge", "502", "--squash"}
			id := requestAndAnswer(t, args, approval.StateApproved)

			code := requestWrite(args)

			rec, err := approval.Load(id)
			if err != nil {
				t.Fatal(err)
			}
			if code == 0 || !strings.Contains(rec.Output, tc.reason) {
				t.Errorf("merge returned %d with output %q, want a failure with GitHub's reason", code, rec.Output)
			}
			if log := readCalls(t, calls); strings.Contains(log, "pr edit") {
				t.Errorf("a PR moved after the failed merge:\n%s", log)
			}
		})
	}
}

// Parts 2 to 5 of one stack were opened on part 1's branch before part 1 had a
// PR, and each needed a fix. A PR based on a branch with no open PR must be
// refused before the owner is asked.
func TestCreateRefusesBaseWithoutOpenPR(t *testing.T) {
	cases := []struct {
		name string
		base string
		open string
		want int
	}{
		{name: "base without PR", base: "part-1", open: "0", want: exitRejected},
		{name: "base with PR", base: "part-1", open: "1", want: exitWaiting},
		{name: "default branch", base: "main", open: "0", want: exitWaiting},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			graphql := `{"data":{"repository":{"defaultBranchRef":{"name":"main"},"pullRequests":{"totalCount":` + tc.open + `}}}}`
			fakeStackGH(t, "", graphql, "", "")

			code := requestWrite([]string{"pr", "create", "--base", tc.base, "--title", "Part 2"})

			if code != tc.want {
				t.Errorf("create on %s returned %d, want %d", tc.base, code, tc.want)
			}
		})
	}
}

// gh pr merge prints nothing after a merge when its output is not a terminal,
// so the agent saw only the moved PRs. The output must say the PR merged, but
// never when --auto only queued the merge.
func TestMergePlainReportsMergeUnlessAuto(t *testing.T) {
	plainGraphQL := strings.Replace(lowestGraphQL, `"stack":{"baseRefName":"main"}`, `"stack":null`, 1)
	cases := []struct {
		name   string
		args   []string
		merged bool
	}{
		{name: "merged", args: []string{"pr", "merge", "502", "--squash"}, merged: true},
		{name: "auto merge queued", args: []string{"pr", "merge", "502", "--squash", "--auto"}, merged: false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			fakeStackGH(t, lowestView, plainGraphQL, "", "")
			id := requestAndAnswer(t, tc.args, approval.StateApproved)

			requestWrite(tc.args)

			rec, err := approval.Load(id)
			merged := strings.Contains(rec.Output, "pde-gh-write: merged #502")
			if err != nil || merged != tc.merged {
				t.Errorf("output %q (%v), want merged line %v", rec.Output, err, tc.merged)
			}
		})
	}
}
