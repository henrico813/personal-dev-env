package cmd

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"

	"kanban/internal/card"
)

// How long card waits for herdr, as the Python card does.
const herdrTimeout = 10 * time.Second

// The end of each error that leaves the Log without an author.
const authorHint = "Pass --by NAME; the user passes --by user."

// One agent in herdr's JSON output.
type herdrAgent struct {
	Name    string `json:"name"`
	PaneID  string `json:"pane_id"`
	Agent   string `json:"agent"`
	Session *struct {
		Value string `json:"value"`
	} `json:"agent_session"`
}

// Returns who acts, for the Log, from --by or else this pane's Herdr agent.
// It fails rather than guess, since a guessed name could credit the user
// with an agent's move.
func findAuthor(e *env, by string) (string, error) {
	if by != "" {
		return by, nil
	}
	if e.paneID == "" {
		return "", fmt.Errorf("no Herdr pane, so no author for the Log. %s", authorHint)
	}
	agents, err := listHerdrAgents()
	if err != nil {
		return "", fmt.Errorf("herdr agent list failed (%w), so no author. %s", err, authorHint)
	}
	for _, agent := range agents {
		if agent.PaneID == e.paneID {
			if agent.Name == "" {
				break
			}
			return agent.Name, nil
		}
	}
	return "", fmt.Errorf("no named Herdr agent in pane %s. %s", e.paneID, authorHint)
}

// Asks `herdr agent get` for an agent's kind and session id, where an
// empty agent means this pane's agent. When herdr gives no name, the
// session takes the name asked for.
func findAgentSession(e *env, agent string) (card.AgentSession, error) {
	target := agent
	if target == "" {
		target = e.paneID
	}
	if target == "" {
		return card.AgentSession{}, errors.New("no Herdr pane, so pass --session AGENT")
	}
	shown := "herdr agent get " + target
	stdout, stderr, err := runHerdr("agent", "get", target)
	if err != nil {
		return card.AgentSession{}, fmt.Errorf("%s failed: %w", shown, err)
	}
	var reply struct {
		Result *struct {
			Agent *herdrAgent `json:"agent"`
		} `json:"result"`
	}
	found := json.Unmarshal([]byte(stdout), &reply) == nil && reply.Result != nil && reply.Result.Agent != nil
	if !found || reply.Result.Agent.Agent == "" || reply.Result.Agent.Session == nil {
		output := stderr
		if output == "" {
			output = stdout
		}
		return card.AgentSession{}, fmt.Errorf("%s gave no session: %s", shown, strings.TrimSpace(output))
	}
	got := reply.Result.Agent
	name := got.Name
	if name == "" {
		name = target
	}
	return card.AgentSession{Name: name, Entry: got.Agent + " " + got.Session.Value}, nil
}

// Runs "herdr agent list" for card rekey, which refuses cards that a
// running worker holds. An agent with no session gets an empty Entry.
func listLiveAgents() ([]card.AgentSession, error) {
	agents, err := listHerdrAgents()
	if err != nil {
		return nil, fmt.Errorf("herdr agent list failed (%w), so live workers are unknown", err)
	}
	live := make([]card.AgentSession, 0, len(agents))
	for _, agent := range agents {
		entry := ""
		if agent.Session != nil && agent.Session.Value != "" {
			entry = agent.Agent + " " + agent.Session.Value
		}
		live = append(live, card.AgentSession{Name: agent.Name, Entry: entry})
	}
	return live, nil
}

// Runs "herdr agent list" and reads its result.agents.
func listHerdrAgents() ([]herdrAgent, error) {
	stdout, _, err := runHerdr("agent", "list")
	if err != nil {
		return nil, err
	}
	var reply struct {
		Result *struct {
			Agents *[]herdrAgent `json:"agents"`
		} `json:"result"`
	}
	if err := json.Unmarshal([]byte(stdout), &reply); err != nil {
		return nil, err
	}
	// The Python card shows its KeyError here, which names the missing key.
	if reply.Result == nil {
		return nil, errors.New("'result'")
	}
	if reply.Result.Agents == nil {
		return nil, errors.New("'agents'")
	}
	return *reply.Result.Agents, nil
}

// Runs herdr with a 10 second limit and returns its output. Its exit code
// is not checked, as in the Python card, which reads only the JSON.
func runHerdr(args ...string) (stdout, stderr string, err error) {
	ctx, cancel := context.WithTimeout(context.Background(), herdrTimeout)
	defer cancel()
	var out, errOut bytes.Buffer
	herdr := exec.CommandContext(ctx, "herdr", args...)
	herdr.Stdout = &out
	herdr.Stderr = &errOut
	err = herdr.Run()
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) && ctx.Err() == nil {
		err = nil
	}
	return out.String(), errOut.String(), err
}
