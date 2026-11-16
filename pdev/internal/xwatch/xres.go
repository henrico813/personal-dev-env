package xwatch

import (
	"bytes"
	"encoding/binary"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"strings"
	"time"
)

// One X client connection, as the X-Resource extension reports it.
type client struct {
	base    uint32 // First resource ID of the client, such as 0x3a00000.
	windows int
	pid     int // 0 when the server does not know the PID.
}

// Numbers from the X11 core protocol, the X-Resource extension and Xauthority
// files.
const (
	badValue        = 2
	windowAtom      = 33
	queryExtension  = 98
	xresClients     = 1
	xresResources   = 2
	xresClientIDs   = 4
	xresPIDMask     = 2
	xauthLocal      = 256
	xauthWild       = 65535
	xauthCookieName = "MIT-MAGIC-COOKIE-1"
)

var le = binary.LittleEndian

type xError struct {
	code byte
}

func (e xError) Error() string {
	return fmt.Sprintf("X error %d", e.code)
}

// Returns the window count and PID of every client of a local X display.
// The server counts each client's windows in one request, so this stays fast
// with tens of thousands of windows, where xwininfo and xrestop take minutes.
func queryClients(socketDir, display, authFile string) ([]client, error) {
	conn, err := dialDisplay(socketDir, display, authFile)
	if err != nil {
		return nil, err
	}
	defer conn.Close()

	major, err := conn.findExtensionOpcode("X-Resource")
	if err != nil {
		return nil, err
	}
	reply, err := conn.request([]byte{major, xresClients, 1, 0})
	if err != nil {
		return nil, fmt.Errorf("list X clients: %w", err)
	}
	var clients []client
	for _, pair := range splitRecords(reply, 8) {
		base := le.Uint32(pair)
		windows, err := conn.countWindows(major, base)
		// The client exited after the server listed it.
		var xerr xError
		if errors.As(err, &xerr) && xerr.code == badValue {
			continue
		}
		if err != nil {
			return nil, err
		}
		clients = append(clients, client{base: base, windows: windows})
	}
	if err := conn.fillPIDs(major, clients); err != nil {
		return nil, err
	}
	return clients, nil
}

type xConn struct {
	net.Conn
}

func dialDisplay(socketDir, display, authFile string) (*xConn, error) {
	number, err := parseDisplayNumber(display)
	if err != nil {
		return nil, err
	}
	raw, err := net.DialTimeout("unix", socketDir+"/X"+number, 5*time.Second)
	if err != nil {
		return nil, fmt.Errorf("connect to X display %s: %w", display, err)
	}
	// A hung server must not stop the watch forever.
	if err := raw.SetDeadline(time.Now().Add(time.Minute)); err != nil {
		raw.Close()
		return nil, err
	}
	conn := &xConn{raw}
	if _, err := conn.sendSetup(readCookie(authFile, number)); err != nil {
		conn.Close()
		return nil, err
	}
	return conn, nil
}

// Accepts local displays such as ":0", ":0.1" and "unix:0".
func parseDisplayNumber(display string) (string, error) {
	host, rest, found := strings.Cut(display, ":")
	number, _, _ := strings.Cut(rest, ".")
	if !found || number == "" || (host != "" && host != "unix") {
		return "", fmt.Errorf("DISPLAY %q is not a local X display", display)
	}
	return number, nil
}

// Returns the MIT-MAGIC-COOKIE-1 for a display number from an Xauthority
// file, or nil when it has none.
func readCookie(path, number string) []byte {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	host, _ := os.Hostname()
	r := bytes.NewReader(data)
	for {
		var family uint16
		if binary.Read(r, binary.BigEndian, &family) != nil {
			return nil
		}
		// Address, display number, auth name and auth data.
		fields := make([]string, 4)
		for i := range fields {
			var size uint16
			if binary.Read(r, binary.BigEndian, &size) != nil {
				return nil
			}
			field := make([]byte, size)
			if _, err := io.ReadFull(r, field); err != nil {
				return nil
			}
			fields[i] = string(field)
		}
		local := family == xauthWild || (family == xauthLocal && fields[0] == host)
		if local && fields[1] == number && fields[2] == xauthCookieName {
			return []byte(fields[3])
		}
	}
}

// Returns the server's setup reply after its 8-byte header, which holds the
// client's first resource ID and the root window.
func (c *xConn) sendSetup(cookie []byte) ([]byte, error) {
	name := xauthCookieName
	if cookie == nil {
		name = ""
	}
	req := []byte{'l', 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0}
	le.PutUint16(req[6:], uint16(len(name)))
	le.PutUint16(req[8:], uint16(len(cookie)))
	req = append(req, padTo4([]byte(name))...)
	req = append(req, padTo4(cookie)...)
	if _, err := c.Write(req); err != nil {
		return nil, fmt.Errorf("X setup: %w", err)
	}
	head := make([]byte, 8)
	if _, err := io.ReadFull(c, head); err != nil {
		return nil, fmt.Errorf("X setup: %w", err)
	}
	body := make([]byte, 4*int(le.Uint16(head[6:])))
	if _, err := io.ReadFull(c, body); err != nil {
		return nil, fmt.Errorf("X setup: %w", err)
	}
	if head[0] != 1 {
		reason := body[:min(int(head[1]), len(body))]
		return nil, fmt.Errorf("X server refused the connection: %s", strings.TrimSpace(string(reason)))
	}
	return body, nil
}

func (c *xConn) findExtensionOpcode(name string) (byte, error) {
	req := []byte{queryExtension, 0, 0, 0, 0, 0, 0, 0}
	req = append(req, padTo4([]byte(name))...)
	le.PutUint16(req[2:], uint16(len(req)/4))
	le.PutUint16(req[4:], uint16(len(name)))
	reply, err := c.request(req)
	if err != nil {
		return 0, fmt.Errorf("find %s: %w", name, err)
	}
	if reply[8] == 0 {
		return 0, fmt.Errorf("the X server has no %s extension", name)
	}
	return reply[9], nil
}

func (c *xConn) countWindows(major byte, base uint32) (int, error) {
	req := []byte{major, xresResources, 2, 0}
	req = le.AppendUint32(req, base)
	reply, err := c.request(req)
	if err != nil {
		return 0, fmt.Errorf("count windows of X client %#x: %w", base, err)
	}
	for _, pair := range splitRecords(reply, 8) {
		if le.Uint32(pair) == windowAtom {
			return int(le.Uint32(pair[4:])), nil
		}
	}
	return 0, nil
}

func (c *xConn) fillPIDs(major byte, clients []client) error {
	req := []byte{major, xresClientIDs, 0, 0}
	req = le.AppendUint32(req, uint32(len(clients)))
	for _, cl := range clients {
		req = le.AppendUint32(req, cl.base)
		req = le.AppendUint32(req, xresPIDMask)
	}
	le.PutUint16(req[2:], uint16(len(req)/4))
	reply, err := c.request(req)
	if err != nil {
		return fmt.Errorf("find X client PIDs: %w", err)
	}
	// Each ID is the client, the ID kind, a byte length and the value.
	pids := map[uint32]int{}
	values := reply[32:]
	for len(values) >= 12 {
		size := int(le.Uint32(values[8:]))
		if size > len(values)-12 {
			break
		}
		if le.Uint32(values[4:]) == xresPIDMask && size == 4 {
			pids[le.Uint32(values)] = int(le.Uint32(values[12:]))
		}
		values = values[12+size:]
	}
	for i := range clients {
		clients[i].pid = pids[clients[i].base]
	}
	return nil
}

// The reply is at least 32 bytes long.
func (c *xConn) request(req []byte) ([]byte, error) {
	if _, err := c.Write(req); err != nil {
		return nil, err
	}
	for {
		head := make([]byte, 32)
		if _, err := io.ReadFull(c, head); err != nil {
			return nil, err
		}
		switch head[0] {
		case 0:
			return nil, xError{code: head[1]}
		case 1:
			rest := make([]byte, 4*int(le.Uint32(head[4:])))
			if _, err := io.ReadFull(c, rest); err != nil {
				return nil, err
			}
			return append(head, rest...), nil
		}
		// Anything else is an event, which this connection never asked for.
	}
}

// The list length is the reply's 32-bit number at offset 8.
func splitRecords(reply []byte, size int) [][]byte {
	var list [][]byte
	data := reply[32:]
	for i := uint32(0); i < le.Uint32(reply[8:]) && len(data) >= size; i++ {
		list = append(list, data[:size])
		data = data[size:]
	}
	return list
}

func padTo4(data []byte) []byte {
	return append(bytes.Clone(data), make([]byte, (4-len(data)%4)%4)...)
}
