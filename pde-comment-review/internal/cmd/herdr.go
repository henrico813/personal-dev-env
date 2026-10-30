package cmd

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"os"
	"time"

	"pde-comment-review/internal/review"
)

// Must match the plugin manifest chezmoi deploys from
// chezmoi/dot_local/share/pde/herdr-plugins/pde-comment-review/.
const (
	pluginID    = "pde.comment-review"
	popupPaneID = "popup"
	tabPaneID   = "tab"
	editPaneID  = "edit"
	diffPaneID  = "diff"
)

// With no review name, the popup and the review tab list the waiting reviews.
const reviewNameEnv = "PDE_COMMENT_REVIEW"

// The label the review tab gets, and the one a second Ctrl+b E looks for.
const reviewTabLabel = "comment review"

// The ID of the item the edit tab edits.
const requestIDEnv = "PDE_COMMENT_REVIEW_ITEM"

// Herdr's answer to a refused request, such as ui_busy while another popup
// is open.
type herdrError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

func (e *herdrError) Error() string {
	return fmt.Sprintf("herdr: %s (%s)", e.Message, e.Code)
}

// Opens the one waiting review in Herdr's popup or review tab, or the list
// when several wait.
func openWaitingReview(placement string) error {
	if os.Getenv("HERDR_PLUGIN_ACTION_ID") == "" {
		return errors.New("only Herdr's Ctrl+b e or E opens the review; in a terminal, run open NAME")
	}
	err := openWaitingPane(placement)
	// Herdr keeps the action's error in its plugin log, out of sight, so
	// the owner also gets a notification.
	if err != nil {
		_ = showNotification(explainOpenFailure(err))
	}
	return err
}

func openWaitingPane(placement string) error {
	waiting, err := review.ListWaitingReviews()
	if err != nil {
		return err
	}
	if len(waiting) == 0 {
		return showNotification("No comment reviews are waiting")
	}
	name := ""
	if len(waiting) == 1 {
		name = waiting[0].Name
	}
	env := map[string]string{reviewNameEnv: name}
	if placement == "tab" {
		return openReviewTab(env)
	}
	_, err = openPluginPane(popupPaneID, placement, env)
	return err
}

// Opens the review tab, or moves to the one already open, so a second
// Ctrl+b E does not open a second review screen.
func openReviewTab(env map[string]string) error {
	var listed struct {
		Tabs []struct {
			TabID string `json:"tab_id"`
			Label string `json:"label"`
		} `json:"tabs"`
	}
	if err := sendHerdrRequest("tab.list", map[string]any{}, &listed); err != nil {
		return err
	}
	for _, tab := range listed.Tabs {
		if tab.Label == reviewTabLabel {
			return focusTab(tab.TabID)
		}
	}
	tabID, err := openPluginPane(tabPaneID, "tab", env)
	if err != nil {
		return err
	}
	return sendHerdrRequest(
		"tab.rename",
		map[string]any{
			"tab_id": tabID,
			"label":  reviewTabLabel,
		},
		nil,
	)
}

// Opens the popup, or the review, edit or diff tab.
func openPluginPane(pane, placement string, env map[string]string) (tabID string, err error) {
	var opened struct {
		PluginPane struct {
			Pane struct {
				TabID string `json:"tab_id"`
			} `json:"pane"`
		} `json:"plugin_pane"`
	}
	err = sendHerdrRequest(
		"plugin.pane.open",
		map[string]any{
			"plugin_id":  pluginID,
			"entrypoint": pane,
			"placement":  placement,
			"focus":      true,
			"env":        env,
		},
		&opened,
	)
	if err != nil || placement != "tab" {
		return "", err
	}
	tabID = opened.PluginPane.Pane.TabID
	return tabID, focusTab(tabID)
}

// Moves every screen attached to Herdr to the tab.
func focusTab(tabID string) error {
	return sendHerdrRequest("tab.focus", map[string]any{"tab_id": tabID}, nil)
}

// Closes the tab this program runs in, so none is left behind on the
// phone. Herdr keeps a tab open after its program exits, and ends the
// program when the tab closes, so call this last.
func closeOwnTab() error {
	return sendHerdrRequest("pane.close", map[string]any{"pane_id": os.Getenv("HERDR_PANE_ID")}, nil)
}

func explainOpenFailure(err error) string {
	var refused *herdrError
	if errors.As(err, &refused) && refused.Code == "ui_busy" {
		return "Another popup is open. Close it, then press Ctrl+b e"
	}
	return "Cannot open the comment review: " + err.Error()
}

func showNotification(title string) error {
	return sendHerdrRequest("notification.show", map[string]any{"title": title}, nil)
}

// Sends one request to the Herdr server that runs this command, since the
// herdr command cannot open a popup.
func sendHerdrRequest(method string, params, result any) error {
	socket := os.Getenv("HERDR_SOCKET_PATH")
	if socket == "" {
		return errors.New("not running inside Herdr")
	}
	conn, err := net.DialTimeout("unix", socket, 3*time.Second)
	if err != nil {
		return err
	}
	defer conn.Close()
	// A stuck server must not leave Ctrl+b e waiting.
	if err := conn.SetDeadline(time.Now().Add(5 * time.Second)); err != nil {
		return err
	}
	request, err := json.Marshal(map[string]any{
		"id":     "comment-review",
		"method": method,
		"params": params,
	})
	if err != nil {
		return err
	}
	if _, err := conn.Write(append(request, '\n')); err != nil {
		return err
	}
	reply, err := bufio.NewReader(conn).ReadBytes('\n')
	if err != nil {
		return err
	}
	// Herdr refuses with an error object, so a reply alone is not success.
	var answer struct {
		Error  *herdrError     `json:"error"`
		Result json.RawMessage `json:"result"`
	}
	if err := json.Unmarshal(reply, &answer); err != nil {
		return err
	}
	if answer.Error != nil {
		return answer.Error
	}
	if result == nil {
		return nil
	}
	return json.Unmarshal(answer.Result, result)
}
