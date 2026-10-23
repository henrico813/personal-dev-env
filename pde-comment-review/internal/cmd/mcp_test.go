package cmd

import (
	"encoding/json"
	"fmt"
	"maps"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"sync"
	"testing"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	"pde-comment-review/internal/review"
)

const testMCPCheckerEnv = "PDE_COMMENT_REVIEW_TEST_MCP_CHECKER"

// Runs this test binary as the MCP server when a server test starts it.
func TestMain(m *testing.M) {
	if checker := os.Getenv(testMCPCheckerEnv); checker != "" {
		if err := runMCPServer(checker); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		os.Exit(0)
	}
	os.Exit(m.Run())
}

// Starts the real MCP server and returns a function that calls one of its
// tools and returns the reply's text. Any other output on the server's
// stdout ends the session, so every server test fails on a stray print.
func startMCPServer(t *testing.T) func(tool string, args map[string]any) string {
	t.Helper()
	testBinary, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	server := exec.Command(testBinary)
	// Outside any checkout, so a relative dir can never reach a real one.
	server.Dir = t.TempDir()
	server.Env = append(os.Environ(), testMCPCheckerEnv+"="+findRepoChecker(t))
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	session, err := client.Connect(
		t.Context(),
		&mcp.CommandTransport{Command: server},
		nil,
	)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { session.Close() })
	return func(tool string, args map[string]any) string {
		result, err := session.CallTool(t.Context(), &mcp.CallToolParams{Name: tool, Arguments: args})
		// Tests call tools from goroutines too, so errors cannot stop the test.
		if err != nil {
			t.Error(err)
			return ""
		}
		content, ok := result.Content[0].(*mcp.TextContent)
		if !ok {
			t.Errorf(
				"%s replied with %T, want text",
				tool,
				result.Content[0],
			)
			return ""
		}
		return content.Text
	}
}

// Verifies that sets sent at once all reach the owner's round, since
// models send several in one turn.
func TestParallelSetsKeepEveryProposal(t *testing.T) {
	repo := newShopClone(t, "feat")
	numbers := "package shop\n"
	for n := 1; n <= 8; n++ {
		numbers += fmt.Sprintf(
			"\n// Returns %d.\nfunc N%d() int { return %d }\n",
			n,
			n,
			n,
		)
	}
	writeFile(
		t,
		filepath.Join(repo, "numbers.go"),
		numbers,
	)
	call := startMCPServer(t)
	call("review_draft", map[string]any{"dir": repo})

	var sets sync.WaitGroup
	for id := 1; id <= 8; id++ {
		sets.Go(func() {
			call("review_set", map[string]any{
				"dir":    repo,
				"id":     id,
				"answer": "reword",
				"text":   fmt.Sprintf("Item %d words.", id),
			})
		})
	}
	sets.Wait()
	call("review_submit", map[string]any{"dir": repo})

	r, err := review.OpenReview("feat")
	if err != nil {
		t.Fatal(err)
	}
	if len(r.Rounds) != 1 || len(r.Rounds[0].Items) != 8 {
		t.Fatalf("rounds = %+v, want one round of 8 items", r.Rounds)
	}
	for _, item := range r.Rounds[0].Items {
		if want := fmt.Sprintf("// Item %d words.\n", item.ID); item.New != want {
			t.Errorf(
				"item %d proposal = %q, want %q",
				item.ID,
				item.New,
				want,
			)
		}
	}
}

// Verifies that tools refuse a dir that is not absolute, since a harness
// may start the server outside the agent's checkout.
func TestToolsRequireAbsoluteDir(t *testing.T) {
	call := startMCPServer(t)
	tests := []struct {
		tool     string
		giveArgs map[string]any
	}{
		{
			tool:     "review_status",
			giveArgs: map[string]any{},
		},
		{
			tool:     "review_draft",
			giveArgs: map[string]any{},
		},
		{
			tool: "review_set",
			giveArgs: map[string]any{
				"id":     1,
				"answer": review.ProposeKeep,
			},
		},
		{
			tool:     "review_submit",
			giveArgs: map[string]any{},
		},
	}

	for _, tt := range tests {
		for _, dir := range []string{".", ""} {
			args := maps.Clone(tt.giveArgs)
			args["dir"] = dir
			text := call(tt.tool, args)

			var reply agentReply
			err := json.Unmarshal([]byte(text), &reply)
			if err != nil || !slices.Contains(reply.Problems, "dir must be the checkout's absolute path") {
				t.Errorf(
					"%s with dir %q = %+v, %v; want a refusal",
					tt.tool,
					dir,
					reply,
					err,
				)
			}
		}
	}
}

// Verifies that replies show & < > as the code has them, since a model
// reads a reply as raw text, not decoded JSON.
func TestToolRepliesKeepCommentTextUnescaped(t *testing.T) {
	repo := newShopClone(t, "feat")
	writeFile(
		t,
		filepath.Join(repo, "count.go"),
		"package shop\n\n// Reports whether n > 0 && n < 10.\nfunc Small(n int) bool { return n > 0 && n < 10 }\n",
	)
	call := startMCPServer(t)

	drafted := call("review_draft", map[string]any{"dir": repo})

	if !strings.Contains(drafted, "// Reports whether n > 0 && n < 10.") {
		t.Errorf("review_draft replied %s, want the comment as written", drafted)
	}
}
