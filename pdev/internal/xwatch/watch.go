package xwatch

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"time"
)

// Sized on a desktop where a leaking terminal and input method held 33,000
// and 26,000 windows after 17 hours, while every other client held under 30.
// Xorg used 30% of a core there with picom stopped, and 60-100% with picom's
// xrender backend.
const (
	windowLimit  = 5000
	xorgCPULimit = 0.6 // Share of one core between two checks.
	checkEvery   = 10 * time.Minute
	repeatAfter  = 2 * time.Hour
)

// Linux reports process CPU time in /proc in ticks of 1/100 second.
const clockTick = 10 * time.Millisecond

// Checks the X server every 10 minutes and sends a desktop notification for
// each problem. Returns exit code 1 when the X server cannot be reached, as
// after logout.
func WatchDisplay(stderr io.Writer) int {
	authFile := os.Getenv("XAUTHORITY")
	if authFile == "" {
		home, _ := os.UserHomeDir()
		authFile = filepath.Join(home, ".Xauthority")
	}
	w := &watcher{
		socketDir: "/tmp/.X11-unix",
		display:   os.Getenv("DISPLAY"),
		authFile:  authFile,
		procRoot:  "/proc",
		notify:    sendDesktopNotification,
		warned:    map[string]time.Time{},
	}
	for {
		if err := w.checkOnce(time.Now()); err != nil {
			fmt.Fprintf(stderr, "pde-x-watch: %v\n", err)
			return 1
		}
		time.Sleep(checkEvery)
	}
}

type watcher struct {
	socketDir, display, authFile, procRoot string
	notify                                 func(title, body string) error
	warned                                 map[string]time.Time // When each problem was last sent.
	lastXorg                               xorgSample
}

type xorgSample struct {
	pid  int
	busy time.Duration // CPU time since Xorg started.
	at   time.Time
}

// Sends a notification for each problem, and again after repeatAfter while
// it lasts. Returns an error only when the X server cannot be reached.
func (w *watcher) checkOnce(now time.Time) error {
	clients, err := queryClients(w.socketDir, w.display, w.authFile)
	if err != nil {
		return err
	}
	slices.SortFunc(clients, func(a, b client) int { return b.windows - a.windows })
	for _, c := range clients {
		if c.windows < windowLimit {
			break
		}
		name := w.findClientName(c)
		w.notifyUnlessRecent(now, "windows "+name,
			fmt.Sprintf("%s holds %d X windows", name, c.windows),
			fmt.Sprintf("Restart %s to free them. They slow the whole desktop.", name))
	}

	sample, ok := readXorgSample(w.procRoot, now)
	if !ok {
		return nil
	}
	previous := w.lastXorg
	w.lastXorg = sample
	// The first check, or a new Xorg after a restart, has nothing to compare.
	if previous.pid != sample.pid {
		return nil
	}
	elapsed := sample.at.Sub(previous.at)
	share := float64(sample.busy-previous.busy) / float64(elapsed)
	if share >= xorgCPULimit && len(clients) > 0 {
		w.notifyUnlessRecent(now, "xorg",
			fmt.Sprintf("Xorg used %d%% of a CPU core", int(share*100)),
			fmt.Sprintf("Over %d minutes. Most X windows: %s with %d.",
				int(elapsed.Minutes()), w.findClientName(clients[0]), clients[0].windows))
	}
	return nil
}

func (w *watcher) notifyUnlessRecent(now time.Time, key, title, body string) {
	if last, ok := w.warned[key]; ok && now.Sub(last) < repeatAfter {
		return
	}
	// A failed notification is tried again at the next check.
	if w.notify(title, body) == nil {
		w.warned[key] = now
	}
}

// Returns the client's process name, such as wezterm-gui.
func (w *watcher) findClientName(c client) string {
	if c.pid != 0 {
		comm, err := os.ReadFile(filepath.Join(w.procRoot, strconv.Itoa(c.pid), "comm"))
		if err == nil {
			return strings.TrimSpace(string(comm))
		}
	}
	return fmt.Sprintf("X client %#x", c.base)
}

// Reads the CPU time of the one process named Xorg. Reports false when
// there is no Xorg or more than one.
func readXorgSample(procRoot string, now time.Time) (xorgSample, bool) {
	stats, _ := filepath.Glob(filepath.Join(procRoot, "[0-9]*", "stat"))
	var found []xorgSample
	for _, path := range stats {
		data, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		// The name sits in parentheses and may hold spaces or parentheses.
		open, end := bytes.IndexByte(data, '('), bytes.LastIndexByte(data, ')')
		if open < 0 || end < open || string(data[open+1:end]) != "Xorg" {
			continue
		}
		// After the name come state, then 10 fields, then utime and stime.
		fields := strings.Fields(string(data[end+1:]))
		if len(fields) < 13 {
			continue
		}
		utime, errU := strconv.ParseInt(fields[11], 10, 64)
		stime, errS := strconv.ParseInt(fields[12], 10, 64)
		pid, errP := strconv.Atoi(filepath.Base(filepath.Dir(path)))
		if errU != nil || errS != nil || errP != nil {
			continue
		}
		found = append(found, xorgSample{pid: pid, busy: time.Duration(utime+stime) * clockTick, at: now})
	}
	if len(found) != 1 {
		return xorgSample{}, false
	}
	return found[0], true
}

func sendDesktopNotification(title, body string) error {
	return exec.Command("notify-send", "--app-name=pde-x-watch", title, body).Run()
}
