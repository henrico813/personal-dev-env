package cmd

import (
	"context"
	"path/filepath"
	"sync"

	"github.com/google/jsonschema-go/jsonschema"
	"github.com/modelcontextprotocol/go-sdk/mcp"

	"pde-comment-review/internal/review"
)

// A harness may start the server outside the agent's worktree, so each
// call names its checkout.
type checkoutInput struct {
	Dir string `json:"dir" jsonschema:"The folder of the checkout to review, as an absolute path."`
}

type setInput struct {
	Dir    string `json:"dir" jsonschema:"The folder of the checkout to review, as an absolute path."`
	ID     int    `json:"id" jsonschema:"The item's id, from review_draft or review_status."`
	Answer string `json:"answer" jsonschema:"keep the comment, cut it, or reword it with text."`
	Text   string `json:"text,omitempty" jsonschema:"For reword only, the new wording as plain sentences, without //."`
}

// Returns an MCP server with one tool per agent command.
func buildMCPServer(checker string) (*mcp.Server, error) {
	setSchema, err := jsonschema.For[setInput](nil)
	if err != nil {
		return nil, err
	}
	// Models see these three answers in the tool list, and the SDK refuses
	// any other.
	setSchema.Properties["answer"].Enum = []any{
		review.SuggestKeep,
		review.SuggestCut,
		review.SuggestReword,
	}
	// The SDK runs calls at the same time, and each set rewrites the draft.
	var oneAtATime sync.Mutex
	// Refuses a dir that is not absolute, or runs command alone, and returns
	// the reply as the tool's result.
	runInCheckoutOneAtATime := func(dir string, command func() agentReply) (*mcp.CallToolResult, any, error) {
		reply := agentReply{
			Problems: []string{"dir must be the checkout's absolute path"},
			Next:     "Call the tool again with dir set to the checkout's absolute path.",
		}
		if filepath.IsAbs(dir) {
			oneAtATime.Lock()
			defer oneAtATime.Unlock()
			reply = command()
		}
		return &mcp.CallToolResult{
			Content: []mcp.Content{&mcp.TextContent{Text: encodeReply(reply)}},
			IsError: len(reply.Problems) > 0,
		}, nil, nil
	}
	server := mcp.NewServer(&mcp.Implementation{Name: "pde-comment-review", Version: "1"}, nil)
	mcp.AddTool(
		server,
		&mcp.Tool{
			Name:        "review_status",
			Description: "Shows the state of the comment review for the branch in dir, the items or the user's notes to act on, and what to do next.",
		},
		func(_ context.Context, _ *mcp.CallToolRequest, in checkoutInput) (*mcp.CallToolResult, any, error) {
			return runInCheckoutOneAtATime(in.Dir, func() agentReply { return replyToStatus(in.Dir) })
		},
	)
	mcp.AddTool(
		server,
		&mcp.Tool{
			Name:        "review_draft",
			Description: "Finds every comment on the branch in dir that the user must answer and saves them as a draft. Change any item with review_set, then send the draft with review_submit.",
		},
		func(_ context.Context, _ *mcp.CallToolRequest, in checkoutInput) (*mcp.CallToolResult, any, error) {
			return runInCheckoutOneAtATime(in.Dir, func() agentReply { return replyToDraft(in.Dir, checker) })
		},
	)
	mcp.AddTool(
		server,
		&mcp.Tool{
			Name:        "review_set",
			Description: "Changes one item's proposal in the draft, or answers the user's note on an item.",
			InputSchema: setSchema,
		},
		func(_ context.Context, _ *mcp.CallToolRequest, in setInput) (*mcp.CallToolResult, any, error) {
			return runInCheckoutOneAtATime(in.Dir, func() agentReply {
				return replyToSet(
					in.Dir,
					in.ID,
					in.Answer,
					in.Text,
				)
			})
		},
	)
	mcp.AddTool(
		server,
		&mcp.Tool{
			Name:        "review_submit",
			Description: "Sends the draft to the user as the next round of the review. It writes no code.",
		},
		func(_ context.Context, _ *mcp.CallToolRequest, in checkoutInput) (*mcp.CallToolResult, any, error) {
			return runInCheckoutOneAtATime(in.Dir, func() agentReply { return replyToSubmit(in.Dir, checker) })
		},
	)
	return server, nil
}

// Serves the agent commands as MCP tools on stdin and stdout until stdin
// closes, so nothing else may print to stdout.
func runMCPServer(checker string) error {
	server, err := buildMCPServer(checker)
	if err != nil {
		return err
	}
	return server.Run(context.Background(), &mcp.StdioTransport{})
}
