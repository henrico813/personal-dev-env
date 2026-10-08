package installer

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"syscall"

	"pde-installer/internal/run"
)

const openCodeServerRestartPrompt = "Restart the OpenCode server now so agents use the gh guard? [y/N] "

type openCodeServer struct {
	pid int
}

func refreshOpenCodeServers(home string, runner run.Runner) error {
	if runner.DryRun {
		return runner.Plan("refresh OpenCode server environment", nil)
	}
	servers, err := staleOpenCodeServers(home)
	if err != nil {
		return err
	}
	fmt.Fprintln(runner.Out(), "Open new shells or tmux panes to use the updated PATH.")
	if len(servers) == 0 {
		return nil
	}
	for _, server := range servers {
		fmt.Fprintf(runner.Err(), "OpenCode server PID %d has the old PATH; restart it with: kill -TERM %d\n", server.pid, server.pid)
	}
	fmt.Fprintln(runner.Out(), "OpenCode clients start the server again on demand after it stops.")
	terminal, err := os.OpenFile("/dev/tty", os.O_RDWR, 0)
	if err != nil {
		return nil
	}
	defer terminal.Close()
	fmt.Fprint(terminal, openCodeServerRestartPrompt)
	answer, err := bufio.NewReader(terminal).ReadString('\n')
	if err != nil {
		return fmt.Errorf("read OpenCode restart prompt: %w", err)
	}
	if !strings.EqualFold(strings.TrimSpace(answer), "y") {
		return nil
	}
	for _, server := range servers {
		if err := syscall.Kill(server.pid, syscall.SIGTERM); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("stop OpenCode server %d: %w", server.pid, err)
		}
	}
	return nil
}

func staleOpenCodeServers(home string) ([]openCodeServer, error) {
	entries, err := os.ReadDir("/proc")
	if err != nil {
		return nil, fmt.Errorf("scan /proc: %w", err)
	}
	guard := filepath.Join(home, ".local", "bin", "gh")
	if resolved, err := filepath.EvalSymlinks(guard); err == nil {
		guard = resolved
	}
	servers := make([]openCodeServer, 0)
	for _, entry := range entries {
		pid, err := strconv.Atoi(entry.Name())
		if err != nil || !processOwnedByCurrentUser(pid) {
			continue
		}
		cmdline, err := os.ReadFile(filepath.Join("/proc", entry.Name(), "cmdline"))
		if err != nil || !isOpenCodeServer(cmdline) {
			continue
		}
		environ, err := os.ReadFile(filepath.Join("/proc", entry.Name(), "environ"))
		if err != nil {
			continue
		}
		path := environmentValue(environ, "PATH")
		firstGH := firstOnPath(path, "gh")
		if firstGH == "" {
			servers = append(servers, openCodeServer{pid: pid})
			continue
		}
		if resolved, err := filepath.EvalSymlinks(firstGH); err == nil {
			firstGH = resolved
		}
		if firstGH != guard {
			servers = append(servers, openCodeServer{pid: pid})
		}
	}
	sort.Slice(servers, func(i, j int) bool { return servers[i].pid < servers[j].pid })
	return servers, nil
}

func processOwnedByCurrentUser(pid int) bool {
	status, err := os.ReadFile(filepath.Join("/proc", strconv.Itoa(pid), "status"))
	if err != nil {
		return false
	}
	for _, line := range strings.Split(string(status), "\n") {
		if strings.HasPrefix(line, "Uid:") {
			fields := strings.Fields(line)
			return len(fields) > 1 && fields[1] == strconv.Itoa(os.Getuid())
		}
	}
	return false
}

func isOpenCodeServer(cmdline []byte) bool {
	parts := strings.Split(string(cmdline), "\x00")
	if len(parts) == 0 || filepath.Base(parts[0]) != "opencode" {
		return false
	}
	for _, part := range parts[1:] {
		if part == "serve" {
			return true
		}
	}
	return false
}

func environmentValue(environ []byte, key string) string {
	prefix := key + "="
	for _, entry := range strings.Split(string(environ), "\x00") {
		if strings.HasPrefix(entry, prefix) {
			return strings.TrimPrefix(entry, prefix)
		}
	}
	return ""
}

func firstOnPath(path, name string) string {
	for _, directory := range filepath.SplitList(path) {
		if directory == "" {
			directory = "."
		}
		candidate := filepath.Join(directory, name)
		if info, err := os.Stat(candidate); err == nil && info.Mode().IsRegular() && info.Mode()&0o111 != 0 {
			return candidate
		}
	}
	return ""
}
