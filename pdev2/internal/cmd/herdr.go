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
	} else if workspace := focusedWorkspace(); workspace != "" {
		// Herdr draws the popup only in the workspace focused when it opened,
		// which need not be the agent's.
		body = "Answer the popup in " + workspace
	}
	_ = exec.Command("herdr", "notification", "show", notificationTitle(), "--body", body, "--sound", "request").Run()
}

// A Herdr client attached to several computers shows each one's
// notifications without saying which computer sent it, and the popup is
// drawn only when that computer is selected.
func notificationTitle() string {
	host, err := os.Hostname()
	if err != nil || host == "" {
		return "PR approval"
	}
	return "PR approval on " + host
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

// Empty outside Herdr or when Herdr knows neither the session nor the pane.
// Herdr does not list every session, never a subagent's, so the pane Herdr
// started this process in comes next; a subagent's request then names its
// main agent's pane.
func agentContext(session string) (pane, workspace, title string) {
	if os.Getenv("HERDR_ENV") != "1" {
		return "", "", ""
	}
	agent := findAgent(session)
	if agent.Pane == "" {
		agent = findPane(os.Getenv("HERDR_PANE_ID"))
	}
	if agent.Pane == "" {
		return "", "", ""
	}
	return agent.Pane, workspaceLabel(agent.Workspace), agent.Title
}

// A pane moved to another workspace keeps answering to its old ID, so Herdr
// is asked for its current ID and workspace; HERDR_WORKSPACE_ID can be stale.
func findPane(id string) agentInfo {
	if id == "" {
		return agentInfo{}
	}
	output, err := herdrOutput("pane", "get", id)
	if err != nil {
		return agentInfo{}
	}
	var envelope struct {
		Result struct {
			Pane struct {
				PaneID        string `json:"pane_id"`
				WorkspaceID   string `json:"workspace_id"`
				TerminalTitle string `json:"terminal_title"`
			} `json:"pane"`
		} `json:"result"`
	}
	if json.Unmarshal(output, &envelope) != nil {
		return agentInfo{}
	}
	pane := envelope.Result.Pane
	return agentInfo{Pane: pane.PaneID, Workspace: pane.WorkspaceID, Title: pane.TerminalTitle}
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

type herdrWorkspace struct {
	ID      string `json:"workspace_id"`
	Label   string `json:"label"`
	Focused bool   `json:"focused"`
}

// Empty when Herdr cannot be asked.
func listWorkspaces() []herdrWorkspace {
	output, err := herdrOutput("workspace", "list")
	if err != nil {
		return nil
	}
	var envelope struct {
		Result struct {
			Workspaces []herdrWorkspace `json:"workspaces"`
		} `json:"result"`
	}
	if json.Unmarshal(output, &envelope) != nil {
		return nil
	}
	return envelope.Result.Workspaces
}

// Workspace IDs such as "wX" mean nothing on screen, so the label is preferred.
func workspaceLabel(id string) string {
	for _, item := range listWorkspaces() {
		if item.ID == id && item.Label != "" {
			return item.Label
		}
	}
	return id
}

// The label of the workspace Herdr has focused, where it opens a popup;
// empty when Herdr cannot say.
func focusedWorkspace() string {
	for _, item := range listWorkspaces() {
		if !item.Focused {
			continue
		}
		if item.Label != "" {
			return item.Label
		}
		return item.ID
	}
	return ""
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
