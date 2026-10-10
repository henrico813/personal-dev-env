package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"slices"
	"strconv"
	"strings"
)

type request struct {
	Operation string
	Title     string
	Body      string
	Repo      string
	Target    string // PR number or branch, used to fetch current values for pr edit.
	Command   string // Quoted command shown to the approver and hashed into the ID.
	ID        string
	Args      []string
	HasTitle  bool
	HasBody   bool
	BodyFile  bool // Body is sent to gh on standard input.
	// An unrecognized flag came before the target and may have taken it as
	// its value, so Target is empty and current values cannot be shown.
	TargetUnknown bool
}

var valueFlags = map[string]string{
	"--title": "title", "-t": "title", "--subject": "title",
	"--body": "body", "-b": "body",
	"--body-file": "file", "-F": "file",
	"--repo": "repo", "-R": "repo",
}

var writeOperations = []string{"create", "edit", "comment", "review", "merge", "ready", "close", "reopen"}

// Body files are read once and sent to gh on standard input, so gh posts the
// approved text.
func parse(args []string, stdin io.Reader) (request, error) {
	if len(args) < 2 || args[0] != "pr" {
		return request{}, fmt.Errorf("usage: pde-gh-write pr {%s} [gh arguments...]", strings.Join(writeOperations, "|"))
	}
	req := request{Operation: args[1], Args: append([]string(nil), args...)}
	if !slices.Contains(writeOperations, req.Operation) {
		return request{}, fmt.Errorf("refuses gh pr %s; use a read-only gh command or an allowlisted write", req.Operation)
	}
	for index := 2; index < len(args); index++ {
		name, value, inline := strings.Cut(args[index], "=")
		if name == "--web" || name == "-w" {
			return request{}, fmt.Errorf("refuses browser-based PR writes")
		}
		kind, ok := valueFlags[name]
		if !ok {
			continue
		}
		if !inline {
			index++
			if index == len(args) {
				return request{}, fmt.Errorf("%s requires a value", name)
			}
			value = args[index]
		}
		switch kind {
		case "title":
			req.Title, req.HasTitle = value, true
		case "body":
			req.Body, req.HasBody = value, true
		case "repo":
			req.Repo = value
		case "file":
			if err := readBodyFile(&req, value, stdin); err != nil {
				return request{}, err
			}
			if inline {
				req.Args[index] = "--body-file=-"
			} else {
				req.Args[index-1], req.Args[index] = "--body-file", "-"
			}
		}
	}
	req.Target, req.TargetUnknown = findTarget(req.Operation, args[2:])
	req.Command = commandText(args)
	req.ID = requestID(req)
	return req, nil
}

func readBodyFile(req *request, path string, stdin io.Reader) error {
	var body []byte
	var err error
	if path == "-" {
		body, err = io.ReadAll(stdin)
	} else {
		body, err = os.ReadFile(path)
	}
	if err != nil {
		return fmt.Errorf("read PR body file: %w", err)
	}
	req.Body, req.HasBody, req.BodyFile = string(body), true, true
	return nil
}

// For pr edit, the approver sees the PR's old title and body next to the new
// ones. Fetching the old ones needs the PR number, and options may come first:
// in "gh pr edit --base main 12" the PR is 12, not main. targetFlags lists,
// per operation, the options that may come before the number and whether each
// takes a value that must be skipped. Title, body, and repo options are in
// valueFlags. When gh adds any option to these commands, add it here, with
// true if it takes a value; otherwise commands that put it before the number
// lose the old title on the approval screen.
var targetFlags = map[string]map[string]bool{
	"edit": {
		"--add-assignee": true, "--add-label": true, "--add-project": true, "--add-reviewer": true,
		"--remove-assignee": true, "--remove-label": true, "--remove-project": true, "--remove-reviewer": true,
		"--attach": true, "--base": true, "-B": true, "--milestone": true, "-m": true,
		"--remove-milestone": false,
	},
}

// Returns the first argument that is neither an option nor an option's value.
// An option missing from the lists may take the next argument as its value, so
// meeting one first reports unknown instead of guessing.
func findTarget(operation string, args []string) (target string, unknown bool) {
	for index := 0; index < len(args); index++ {
		name, _, inline := strings.Cut(args[index], "=")
		takesValue, known := targetFlags[operation][name]
		if _, ok := valueFlags[name]; ok {
			takesValue, known = true, true
		}
		switch {
		case !strings.HasPrefix(name, "-"):
			return args[index], false
		case !known:
			return "", true
		case takesValue && !inline:
			index++
		}
	}
	return "", false
}

func commandText(args []string) string {
	quoted := make([]string, len(args))
	for index, arg := range args {
		quoted[index] = strconv.Quote(arg)
	}
	return "gh " + strings.Join(quoted, " ")
}

func requestID(req request) string {
	fields, _ := json.Marshal([]string{req.Command, req.Title, req.Body})
	hash := sha256.Sum256(fields)
	return fmt.Sprintf("%x", hash)
}
