package cmd

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

// herdrError is Herdr's answer to a refused socket request, such as ui_busy
// while another popup is open.
type herdrError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func (e *herdrError) Error() string {
	return fmt.Sprintf("herdr: %s (%s)", e.Message, e.Code)
}

// notifyRequest marks the agent's pane in the sidebar and opens the approval
// popup and a notification when the agent runs inside Herdr. Errors are not
// returned; when the popup does not open, the notification says why. The
// request can be answered on the phone or in a terminal instead.
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
	body := "Approval is waiting"
	if err := openApproval(rec.ID); err != nil {
		// Only the inbox moves to the open popup; a new request must not move
		// the screen while the user types elsewhere.
		body, _ = popupFailure(err)
	}
	_ = exec.Command("herdr", "notification", "show", "PR approval", "--body", body, "--sound", "request").Run()
}

// popupFailure turns a popup that did not open into a notification body.
// Herdr shows one popup at a time and answers ui_busy while any popup or menu
// is open. A user in another workspace does not see an approval popup, so
// when one is open the body names its workspace and its ID is returned.
func popupFailure(err error) (body, workspace string) {
	var refused *herdrError
	if !errors.As(err, &refused) || refused.Code != "ui_busy" {
		return "Cannot open the approval popup: " + err.Error(), ""
	}
	// A PID of 0 or less would make processAlive signal a process group.
	if popup, err := approval.LoadPopup(); err == nil && popup.Workspace != "" && popup.PID > 0 && processAlive(popup.PID) {
		return "Approval waiting: answer the popup in " + workspaceLabel(popup.Workspace), popup.Workspace
	}
	return "Close the open Herdr popup or menu, then press Ctrl+b a", ""
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
	reply, err := bufio.NewReader(conn).ReadBytes('\n')
	if err != nil {
		return err
	}
	// Herdr refuses with an error object on an open connection, so a reply
	// alone does not mean the request worked.
	var answer struct {
		Error *herdrError `json:"error"`
	}
	if err := json.Unmarshal(reply, &answer); err != nil {
		return err
	}
	if answer.Error != nil {
		return answer.Error
	}
	return nil
}

func focusRequest(rec approval.Record) {
	pane := findAgent(rec.SessionID).Pane
	if pane == "" {
		pane = rec.PaneID
	}
	if pane != "" {
		_ = exec.Command("herdr", "agent", "focus", pane).Run()
	}
}
