package xwatch

import (
	"bufio"
	"encoding/binary"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

const testCookie = "xwatch-test-cookie"

// Starts Xvfb, which refuses a connection without the cookie in its -auth
// file, and returns a watcher for it. The watcher reads procRoot as /proc and
// saves each notification title in titles.
func startXvfb(t *testing.T, procRoot string, titles *[]string) *watcher {
	t.Helper()
	if _, err := exec.LookPath("Xvfb"); err != nil {
		t.Skip("Xvfb is not installed")
	}
	displayRead, displayWrite, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	defer displayRead.Close()
	// Xvfb takes every cookie in its -auth file, whatever the display number.
	server := exec.Command("Xvfb", "-displayfd", "3", "-auth", writeXauthority(t, "0"), "-nolisten", "tcp")
	server.ExtraFiles = []*os.File{displayWrite}
	if err := server.Start(); err != nil {
		t.Fatal(err)
	}
	displayWrite.Close()
	t.Cleanup(func() {
		server.Process.Kill()
		server.Wait()
	})

	// Xvfb picks a free display and writes its number once it accepts
	// connections.
	number, err := bufio.NewReader(displayRead).ReadString('\n')
	if err != nil {
		t.Fatalf("Xvfb did not start: %v", err)
	}
	number = strings.TrimSpace(number)
	return &watcher{
		socketDir: "/tmp/.X11-unix",
		display:   ":" + number,
		authFile:  writeXauthority(t, number),
		procRoot:  procRoot,
		notify: func(title, body string) error {
			*titles = append(*titles, title)
			return nil
		},
		warned: map[string]time.Time{},
	}
}

// An Xauthority file that holds testCookie for one display number.
func writeXauthority(t *testing.T, number string) string {
	t.Helper()
	data := binary.BigEndian.AppendUint16(nil, xauthWild)
	for _, field := range []string{"", number, xauthCookieName, testCookie} {
		data = binary.BigEndian.AppendUint16(data, uint16(len(field)))
		data = append(data, field...)
	}
	path := filepath.Join(t.TempDir(), "Xauthority")
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

// Opens a second connection to the watcher's X server that holds n windows
// until the test ends.
func createWindows(t *testing.T, w *watcher, n int) {
	t.Helper()
	number := strings.TrimPrefix(w.display, ":")
	raw, err := net.Dial("unix", filepath.Join(w.socketDir, "X"+number))
	if err != nil {
		t.Fatal(err)
	}
	conn := &xConn{raw}
	t.Cleanup(func() { conn.Close() })
	setup, err := conn.sendSetup(readCookie(w.authFile, number))
	if err != nil {
		t.Fatal(err)
	}

	// The root window follows the vendor name and the pixmap formats.
	base := le.Uint32(setup[4:])
	vendor := padTo4(make([]byte, le.Uint16(setup[16:])))
	root := le.Uint32(setup[32+len(vendor)+8*int(setup[21]):])
	var reqs []byte
	for i := 1; i <= n; i++ {
		// CreateWindow of a 1x1 window at 0,0 with the parent's depth and
		// visual.
		req := []byte{1, 0, 8, 0}
		req = le.AppendUint32(req, base+uint32(i))
		req = le.AppendUint32(req, root)
		req = append(req, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0)
		req = le.AppendUint32(req, 0)
		req = le.AppendUint32(req, 0)
		reqs = append(reqs, req...)
	}
	if _, err := conn.Write(reqs); err != nil {
		t.Fatal(err)
	}
	// GetInputFocus returns after the server has made every window, or the
	// first error.
	if _, err := conn.request([]byte{43, 0, 1, 0}); err != nil {
		t.Fatalf("create %d windows: %v", n, err)
	}
}

// A /proc folder whose only process is Xorg, with PID 900, which has used
// xorgSeconds of CPU time.
func writeXorgProc(t *testing.T, root string, xorgSeconds int) {
	t.Helper()
	dir := filepath.Join(root, "900")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	// utime holds all the CPU time and stime none.
	stat := fmt.Sprintf("900 (Xorg) S 1 1 1 0 -1 0 0 0 0 0 %d 0 0 0 20 0 1 0 100\n", xorgSeconds*100)
	if err := os.WriteFile(filepath.Join(dir, "stat"), []byte(stat), 0o644); err != nil {
		t.Fatal(err)
	}
}

// The user learns which program leaks windows, by its process name, so they
// know what to restart. The test process holds the windows, so its name is
// the one the notice shows.
func TestCheckOnceNamesLeakingClient(t *testing.T) {
	var titles []string
	w := startXvfb(t, "/proc", &titles)
	createWindows(t, w, 5000)

	err := w.checkOnce(time.Now())

	if err != nil {
		t.Fatal(err)
	}
	want := "xwatch.test holds 5000 X windows"
	if len(titles) != 1 || titles[0] != want {
		t.Fatalf("titles = %q, want [%q]", titles, want)
	}
}

// A leak lasts until the user restarts the program, so the warning comes
// back, but at most every two hours instead of at every check.
func TestCheckOnceRepeatsAfterTwoHours(t *testing.T) {
	var titles []string
	w := startXvfb(t, "/proc", &titles)
	createWindows(t, w, 5000)
	start := time.Now()

	for _, at := range []time.Duration{0, 10 * time.Minute, 110 * time.Minute, 2 * time.Hour} {
		if err := w.checkOnce(start.Add(at)); err != nil {
			t.Fatal(err)
		}
	}

	if len(titles) != 2 {
		t.Fatalf("sent %d warnings over two hours, want 2: %q", len(titles), titles)
	}
}

// Xorg above 60% of a core between two checks is the slowdown the user
// felt, so the user gets a warning then and not below the limit.
func TestCheckOnceWarnsAboutBusyXorg(t *testing.T) {
	cases := []struct {
		name        string
		busySeconds int // Xorg CPU time used in the 10 minutes between checks.
		want        []string
	}{
		{name: "busy", busySeconds: 420, want: []string{"Xorg used 70% of a CPU core"}},
		{name: "normal", busySeconds: 180},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			procRoot := t.TempDir()
			writeXorgProc(t, procRoot, 1000)
			var titles []string
			w := startXvfb(t, procRoot, &titles)
			start := time.Now()
			if err := w.checkOnce(start); err != nil {
				t.Fatal(err)
			}

			writeXorgProc(t, procRoot, 1000+tc.busySeconds)
			err := w.checkOnce(start.Add(10 * time.Minute))

			if err != nil {
				t.Fatal(err)
			}
			if strings.Join(titles, "|") != strings.Join(tc.want, "|") {
				t.Fatalf("titles = %q, want %q", titles, tc.want)
			}
		})
	}
}
