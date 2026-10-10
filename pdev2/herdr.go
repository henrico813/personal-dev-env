package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"strings"
)

type herdrRequest struct {
	ID     string `json:"id"`
	Method string `json:"method"`
	Params any    `json:"params"`
}

func notifyRequest(rec record) {
	if os.Getenv("HERDR_ENV") != "1" {
		return
	}
	output, err := exec.Command("herdr", "status", "server").CombinedOutput()
	if err != nil {
		return
	}
	socket := herdrSocket(string(output))
	if socket == "" {
		return
	}
	// The CLI cannot request popup placement, so use its socket API for this method.
	// plugin_id and entrypoint must match id and [[panes]] id in the plugin manifest.
	_ = sendHerdr(socket, herdrRequest{ID: rec.ID, Method: "plugin.pane.open", Params: map[string]any{
		"plugin_id":  "pde.approval",
		"entrypoint": "approval",
		"placement":  "popup",
		"focus":      true,
		"env":        map[string]string{"PDE_REQUEST_ID": rec.ID},
	}})
	_ = exec.Command("herdr", "notification", "show", "PR approval", "--body", "Approval is waiting", "--sound", "request").Run()
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
