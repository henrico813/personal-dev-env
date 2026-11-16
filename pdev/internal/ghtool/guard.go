package ghtool

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
)

// The guard puts its pid here for the real gh it starts. A guard whose parent
// pid matches was started by its own gh, as aqua's shim does when its config
// has no gh, and would otherwise loop forever.
const guardPIDVariable = "PDE_GH_GUARD_PID"

const (
	prWriteMessage = "Use pde-gh-write for pull-request writes; it asks the user for approval."
	prNewMessage   = "Use pde-gh-write pr create instead of gh pr new; it asks the user for approval."
	apiMessage     = "gh api cannot change pull requests or comments here; use the matching pde-gh-write pr command, such as pr comment, or ask the user."
)

// Runs as gh and pde-gh-guard. It refuses PR writes unless PDE_GH_WRITE=1,
// which pde-gh-write sets for an approved command, and runs the real gh for the rest.
func guardGH(args []string) int {
	if os.Getenv(guardPIDVariable) == strconv.Itoa(os.Getppid()) {
		fmt.Fprintln(stderr, "gh guard was started by its own gh; the real gh found is not gh, such as an aqua shim whose config has no gh")
		return exitFailed
	}
	if os.Getenv("PDE_GH_WRITE") != "1" {
		if message := findRefusal(args); message != "" {
			fmt.Fprintln(stderr, message)
			return exitRejected
		}
	}
	gh := findRealGH()
	if gh == "" {
		fmt.Fprintln(stderr, "gh guard cannot find the real gh executable")
		return exitFailed
	}
	return runChildGH(gh, args)
}

// Returns the message for a gh command the guard refuses, or "" for one it
// lets through.
func findRefusal(args []string) string {
	if len(args) < 2 {
		return ""
	}
	switch args[0] {
	case "pr":
		switch args[1] {
		case "create", "edit", "comment", "review", "merge", "ready", "close", "reopen":
			return prWriteMessage
		case "new":
			return prNewMessage
		}
	case "api":
		if isAPIWrite(args[1:]) {
			return apiMessage
		}
	}
	return ""
}

// Reports whether gh api arguments change a pull request or a comment. A
// field or input makes gh send POST, so it counts as a write.
func isAPIWrite(args []string) bool {
	path, method, hasField := "", "", false
	for i := 0; i < len(args); i++ {
		argument := args[i]
		switch {
		case argument == "-X" || argument == "--method":
			if i+1 < len(args) {
				method = args[i+1]
			}
			i++
		case strings.HasPrefix(argument, "-X=") || strings.HasPrefix(argument, "--method="):
			method = argument[strings.Index(argument, "=")+1:]
		case isFieldFlag(argument):
			hasField = true
			i++
		case isFieldFlagWithValue(argument):
			hasField = true
		case strings.HasPrefix(argument, "--"):
		default:
			if path == "" {
				path = argument
			}
		}
	}
	if path == "graphql" {
		for _, argument := range args {
			if strings.Contains(argument, "mutation") {
				return true
			}
		}
		return false
	}
	if !strings.Contains(path, "pulls") && !isIssueCommentPath(path) {
		return false
	}
	switch strings.ToUpper(method) {
	case "POST", "PATCH", "PUT", "DELETE":
		return true
	}
	return hasField
}

var fieldFlags = []string{"-f", "-F", "--field", "--raw-field", "--input"}

func isFieldFlag(argument string) bool {
	for _, flag := range fieldFlags {
		if argument == flag {
			return true
		}
	}
	return false
}

func isFieldFlagWithValue(argument string) bool {
	for _, flag := range fieldFlags {
		if strings.HasPrefix(argument, flag+"=") {
			return true
		}
	}
	return false
}

// Matches a path with "comments" after "issues/", such as
// repos/o/r/issues/7/comments, which pull request comments also use.
func isIssueCommentPath(path string) bool {
	index := strings.Index(path, "issues/")
	return index >= 0 && strings.Contains(path[index:], "comments")
}

// Runs gh as a child rather than replacing this process, so the child's
// parent pid marks a loop back into the guard. It passes on signals and returns gh's exit code.
func runChildGH(gh string, args []string) int {
	child := exec.Command(gh, args...)
	child.Stdin, child.Stdout, child.Stderr = os.Stdin, os.Stdout, os.Stderr
	child.Env = append(os.Environ(), guardPIDVariable+"="+strconv.Itoa(os.Getpid()))
	signals := make(chan os.Signal, 1)
	signal.Notify(signals, syscall.SIGINT, syscall.SIGHUP, syscall.SIGTERM)
	defer signal.Stop(signals)
	if err := child.Start(); err != nil {
		fmt.Fprintf(stderr, "gh guard cannot run %s: %v\n", gh, err)
		return exitFailed
	}
	go func() {
		for received := range signals {
			_ = child.Process.Signal(received)
		}
	}()
	err := child.Wait()
	var exitErr *exec.ExitError
	switch {
	case err == nil:
		return 0
	case errors.As(err, &exitErr) && exitErr.ExitCode() >= 0:
		return exitErr.ExitCode()
	}
	// gh died from a signal, so it has no exit code to pass on.
	return exitFailed
}
