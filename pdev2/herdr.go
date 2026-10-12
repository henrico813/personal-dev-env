package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
	"time"

	"pdev2/internal/approval"
)

type herdrRequest struct {
	ID     string `json:"id"`
	Method string `json:"method"`
	Params any    `json:"params"`
}

type agentInfo struct {
	Pane      string
	Workspace string
	Title     string
}

// notifyRequest marks the agent's pane in the sidebar and opens the approval
// popup and a notification when the agent runs inside Herdr. Errors are
// ignored; the request can still be answered on the phone or in a terminal.
func notifyRequest(rec approval.Record) {
	if os.Getenv("HERDR_ENV") != "1" {
		return
	}
	_ = reportMarker(rec.PaneID, true)
	// An answer can arrive, and clear the marker, before it was set.
	if current, err := approval.Load(rec.ID); err != nil || current.State != approval.StatePending {
		clearRecordMarker(rec)
		return
	}
	_ = openApproval(rec.ID)
	_ = exec.Command("herdr", "notification", "show", "PR approval", "--body", "Approval is waiting", "--sound", "request").Run()
}

func openApproval(id string) error {
	output, err := exec.Command("herdr", "status", "server").CombinedOutput()
	if err != nil {
		return err
	}
	socket := herdrSocket(string(output))
	if socket == "" {
		return errors.New("herdr socket unavailable")
	}
	// The herdr command has no option that opens a popup, so send the request
	// to Herdr's socket directly. plugin_id and entrypoint must match id and the
	// [[panes]] id in the plugin manifest that chezmoi deploys from
	// chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/.
	return sendHerdr(socket, herdrRequest{ID: id, Method: "plugin.pane.open", Params: map[string]any{
		"plugin_id":  "pde.approval",
		"entrypoint": "approval",
		"placement":  "popup",
		"focus":      true,
		"env":        map[string]string{"PDE_REQUEST_ID": id},
	}})
}

// Requests share one marker per pane, so it stays while another request from
// that pane still waits.
func clearRecordMarker(rec approval.Record) {
	if rec.PaneID == "" {
		return
	}
	for _, other := range approval.Queued() {
		if other.ID != rec.ID && other.PaneID == rec.PaneID {
			return
		}
	}
	_ = reportMarker(rec.PaneID, false)
}

// Shows or clears "Approval waiting" on the requesting agent's pane in the sidebar.
func reportMarker(pane string, pending bool) error {
	if pane == "" {
		return nil
	}
	args := []string{"pane", "report-metadata", "--source", "pde.approval", pane}
	if pending {
		args = append(args, "--title", "Approval waiting", "--ttl-ms", fmt.Sprint(approval.Expiry.Milliseconds()))
	} else {
		args = append(args, "--clear-title")
	}
	_, err := herdrOutput(args...)
	return err
}

// Empty outside Herdr or when no Herdr agent runs the session.
func agentContext(session string) (pane, workspace, title string) {
	if os.Getenv("HERDR_ENV") != "1" {
		return "", "", ""
	}
	agent := findAgent(session)
	if agent.Pane == "" {
		return "", "", ""
	}
	return agent.Pane, workspaceLabel(agent.Workspace), agent.Title
}

func findAgent(session string) agentInfo {
	if session == "" {
		return agentInfo{}
	}
	output, err := herdrOutput("agent", "list")
	if err != nil {
		return agentInfo{}
	}
	var envelope struct {
		Result struct {
			Agents []struct {
				AgentSession struct {
					Value string `json:"value"`
				} `json:"agent_session"`
				PaneID        string `json:"pane_id"`
				WorkspaceID   string `json:"workspace_id"`
				TerminalTitle string `json:"terminal_title"`
			} `json:"agents"`
		} `json:"result"`
	}
	if json.Unmarshal(output, &envelope) != nil {
		return agentInfo{}
	}
	for _, item := range envelope.Result.Agents {
		if item.AgentSession.Value == session {
			return agentInfo{Pane: item.PaneID, Workspace: item.WorkspaceID, Title: item.TerminalTitle}
		}
	}
	return agentInfo{}
}

// Workspace IDs such as "wX" mean nothing on screen, so the label is preferred.
func workspaceLabel(id string) string {
	output, err := herdrOutput("workspace", "list")
	if err != nil {
		return id
	}
	var envelope struct {
		Result struct {
			Workspaces []struct {
				ID    string `json:"workspace_id"`
				Label string `json:"label"`
			} `json:"workspaces"`
		} `json:"result"`
	}
	if json.Unmarshal(output, &envelope) != nil {
		return id
	}
	for _, item := range envelope.Result.Workspaces {
		if item.ID == id && item.Label != "" {
			return item.Label
		}
	}
	return id
}

// The writer holds the request lock during lookups, so a stuck Herdr server
// must not stall it.
func herdrOutput(args ...string) ([]byte, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
	defer cancel()
	return exec.CommandContext(ctx, "herdr", args...).Output()
}

func herdrSocket(output string) string {
	for _, line := range strings.Split(output, "\n") {
		if strings.HasPrefix(line, "socket:") {
			return strings.TrimSpace(strings.TrimPrefix(line, "socket:"))
		}
	}
	return ""
}

func sendHerdr(socket string, request herdrRequest) error {
	conn, err := net.Dial("unix", socket)
	if err != nil {
		return err
	}
	defer conn.Close()
	data, err := json.Marshal(request)
	if err != nil {
		return err
	}
	if _, err = fmt.Fprintf(conn, "%s\n", data); err != nil {
		return err
	}
	_, err = bufio.NewReader(conn).ReadBytes('\n')
	return err
}
