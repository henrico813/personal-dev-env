package cmd

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/modelcontextprotocol/go-sdk/mcp"

	"pde-comment-review/internal/review"
)

const testMCPCheckerEnv = "PDE_COMMENT_REVIEW_TEST_MCP_CHECKER"

// Runs this test binary as the MCP server when the MCP test starts it.
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

// Models send several sets at once, which lose proposals unless calls run
// one at a time.
func TestMCPToolsMatchCommands(t *testing.T) {
	repo := newShopClone(t, "feat")
	numbers := "package shop\n"
	for n := 1; n <= 8; n++ {
		numbers += fmt.Sprintf(
			"\n// Returns %d & no more.\nfunc N%d() int { return %d }\n",
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
	ctx, cancel := context.WithTimeout(t.Context(), time.Minute)
	defer cancel()
	server := exec.Command(os.Args[0])
	server.Env = append(os.Environ(), testMCPCheckerEnv+"="+findRepoChecker(t))
	client := mcp.NewClient(&mcp.Implementation{Name: "test", Version: "1"}, nil)
	session, err := client.Connect(
		ctx,
		&mcp.CommandTransport{Command: server},
		nil,
	)
	if err != nil {
		t.Fatal(err)
	}
	defer session.Close()
	// Returns the tool's reply, and the text an agent reads.
	call := func(name string, args map[string]any) (agentReply, string) {
		result, err := session.CallTool(ctx, &mcp.CallToolParams{Name: name, Arguments: args})
		if err != nil {
			t.Error(err)
			return agentReply{}, ""
		}
		content, ok := result.Content[0].(*mcp.TextContent)
		if !ok {
			t.Errorf("%s replied with %T, want text", name, result.Content[0])
			return agentReply{}, ""
		}
		var reply agentReply
		// A call the SDK refused has plain text, not a reply.
		_ = json.Unmarshal([]byte(content.Text), &reply)
		return reply, content.Text
	}

	// The agent drafts, makes two mistakes, keeps item 1, rewords every
	// item at once, and submits.
	_, drafted := call("review_draft", map[string]any{"dir": repo})
	relative, _ := call("review_status", map[string]any{"dir": "."})
	runScript(
		t,
		filepath.Dir(repo),
		"git clone -q origin other && git -C other switch -q -c feat",
	)
	other, _ := call("review_status", map[string]any{"dir": filepath.Join(filepath.Dir(repo), "other")})
	kept, _ := call("review_set", map[string]any{
		"dir":    repo,
		"id":     1,
		"answer": "keep",
	})
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
	status, _ := call("review_status", map[string]any{"dir": repo})

	if !strings.Contains(drafted, "// Returns 1 & no more.") {
		t.Errorf("review_draft replied %s, want & as it is", drafted)
	}
	if !slices.Contains(relative.Problems, "dir must be the checkout's absolute path") {
		t.Errorf("review_status with dir . = %+v, want a refusal", relative)
	}
	if len(other.Problems) == 0 || !strings.Contains(other.Problems[0], "tell the owner if that is another checkout") {
		t.Errorf("review_status in another clone = %+v, want a refusal", other)
	}
	if len(kept.Items) != 1 {
		t.Errorf("review_set reply items = %+v, want item 1 only", kept.Items)
	}
	if want := replyToStatus(repo); !reflect.DeepEqual(status, want) || want.State != "waiting_for_user" {
		t.Errorf(
			"review_status reply = %+v, want the status command's %+v, waiting for the user",
			status,
			want,
		)
	}
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
