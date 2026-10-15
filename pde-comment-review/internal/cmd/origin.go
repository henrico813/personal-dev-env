package cmd

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"pde-comment-review/internal/review"
)

// Unknown fields stay empty, but the name falls back to repo's folder.
func findReviewOrigin(repo string) review.Origin {
	session := os.Getenv("OPENCODE_SESSION_ID")
	origin := review.Origin{RepoName: filepath.Base(repo), Session: session, Agent: findAgentTitle(session)}
	// Worktree folders are named after branches, so use the main checkout.
	shared := readCommandOutput("git", "-C", repo, "rev-parse", "--path-format=absolute", "--git-common-dir")
	if filepath.Base(shared) == ".git" {
		origin.RepoName = filepath.Base(filepath.Dir(shared))
	}
	origin.Branch = readCommandOutput("git", "-C", repo, "branch", "--show-current")
	if origin.Branch == "" {
		origin.Branch = readCommandOutput("git", "-C", repo, "rev-parse", "--short", "HEAD")
	}
	return origin
}

// Not HERDR_PANE_ID: under OpenCode it is often another agent's pane.
func findAgentTitle(session string) string {
	if os.Getenv("HERDR_ENV") != "1" || session == "" {
		return ""
	}
	var list struct {
		Result struct {
			Agents []struct {
				AgentSession struct {
					Value string `json:"value"`
				} `json:"agent_session"`
				TerminalTitle string `json:"terminal_title"`
			} `json:"agents"`
		} `json:"result"`
	}
	if json.Unmarshal([]byte(readCommandOutput("herdr", "agent", "list")), &list) != nil {
		return ""
	}
	for _, agent := range list.Result.Agents {
		if agent.AgentSession.Value == session {
			return agent.TerminalTitle
		}
	}
	return ""
}

// The time limit keeps a stuck Herdr from hanging add.
func readCommandOutput(name string, args ...string) string {
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, name, args...)
	// Without this, a child holding the output open outlasts the time limit.
	cmd.WaitDelay = time.Second
	output, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(output))
}
