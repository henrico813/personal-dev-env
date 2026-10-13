package approval

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

type Request struct {
	Operation string
	Title     string
	Body      string
	Repo      string // From --repo until loadOrCreate looks it up for display.
	Target    string // PR number or branch, used to fetch current values for pr edit.
	Command   string // Quoted command shown to the approver and hashed into the ID.
	ID        string
	Args      []string
	HasTitle  bool
	HasBody   bool
	BodyFile  bool // Body is sent to gh on standard input.
	Directory string
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

// gh pr close and reopen post --comment as a PR comment, so the approver must
// see it as the body. For pr review, -c is a switch.
var commentIsBody = map[string]bool{"close": true, "reopen": true}

var writeOperations = []string{"create", "edit", "comment", "review", "merge", "ready", "close", "reopen"}

// Body files are read once and sent to gh on standard input, so gh posts the
// approved text.
func Parse(args []string, stdin io.Reader) (Request, error) {
	if len(args) < 2 || args[0] != "pr" {
		return Request{}, fmt.Errorf("usage: pde-gh-write pr {%s} [gh arguments...]", strings.Join(writeOperations, "|"))
	}
	req := Request{Operation: args[1], Args: append([]string(nil), args...)}
	if !slices.Contains(writeOperations, req.Operation) {
		return Request{}, fmt.Errorf("refuses gh pr %s; it accepts only pr %s; run other gh pr commands with gh", req.Operation, strings.Join(writeOperations, ", "))
	}
	// gh gives an option that takes a value the next argument, even one
	// starting with -, so an unlisted option may have taken it as its value.
	afterUnlisted := false
	for index := 2; index < len(args); index++ {
		arg := args[index]
		switch {
		case arg == "--":
			return Request{}, fmt.Errorf("refuses arguments after --")
		case !strings.HasPrefix(arg, "-") || arg == "-":
			afterUnlisted = false
			continue
		case afterUnlisted:
			option := args[index-1]
			fix := "move " + option + " to the end"
			// A short -x=v is refused below, so only a long option can take =.
			if strings.HasPrefix(option, "--") {
				fix += ", or give its value with ="
			}
			return Request{}, fmt.Errorf("cannot tell whether %s is the value of %s, an option this program does not know; %s", arg, option, fix)
		case len(arg) > 2 && arg[1] != '-':
			// gh reads -bTEXT as -b TEXT and -db=x as -d -b x; refusing
			// them is simpler than reading them the same way.
			return Request{}, fmt.Errorf("refuses joined short option %s; give each option on its own, with a space before its value, such as -b TEXT", arg)
		}
		name, value, inline := strings.Cut(arg, "=")
		if name == "--web" || name == "-w" {
			return Request{}, fmt.Errorf("refuses browser-based PR writes")
		}
		kind, ok := valueFlags[name]
		if commentIsBody[req.Operation] && (name == "--comment" || name == "-c") {
			kind, ok = "body", true
		}
		takesValue, listed := targetFlags[req.Operation][name]
		if ok {
			takesValue, listed = true, true
		}
		if !listed {
			afterUnlisted = !inline
			continue
		}
		if !takesValue {
			continue
		}
		if !inline {
			index++
			if index == len(args) {
				return Request{}, fmt.Errorf("%s requires a value", name)
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
				return Request{}, err
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
	req.ID = RequestID(req)
	return req, nil
}

func readBodyFile(req *Request, path string, stdin io.Reader) error {
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

// The popup and the phone name the PR a request is for, such as "merge #12"
// with its current title, and for pr edit the approver sees the old title and
// body next to the new ones. All of this needs the PR number, and options may
// come first: in "gh pr edit --base main 12" the PR is 12, not main.
// targetFlags lists, per operation, the options from gh pr OPERATION --help
// that may come before the number and whether each takes a value that must be
// skipped. The same short option can differ: -m takes a milestone for edit but
// nothing for merge. Title, body, and repo options are in valueFlags. When gh
// adds any option to these commands, add it here, with true if it takes a
// value; otherwise commands that put it before the number lose the PR title
// in the popup and on the phone. Parse skips these values too, so in
// "--add-label --body=x" the label is --body=x, and it refuses an argument
// starting with - after an option missing here. create has no PR number and
// is listed for Parse only.
var targetFlags = map[string]map[string]bool{
	"create": {
		"--assignee": true, "-a": true, "--attach": true, "--base": true, "-B": true,
		"--head": true, "-H": true, "--label": true, "-l": true, "--milestone": true, "-m": true,
		"--project": true, "-p": true, "--recover": true, "--reviewer": true, "-r": true,
		"--template": true, "-T": true,
		"--draft": false, "-d": false, "--dry-run": false, "--editor": false, "-e": false,
		"--fill": false, "-f": false, "--fill-first": false, "--fill-verbose": false,
		"--no-maintainer-edit": false,
	},
	"edit": {
		"--add-assignee": true, "--add-label": true, "--add-project": true, "--add-reviewer": true,
		"--remove-assignee": true, "--remove-label": true, "--remove-project": true, "--remove-reviewer": true,
		"--attach": true, "--base": true, "-B": true, "--milestone": true, "-m": true,
		"--remove-milestone": false,
	},
	"merge": {
		"--admin": false, "--auto": false, "--delete-branch": false, "-d": false, "--disable-auto": false,
		"--merge": false, "-m": false, "--rebase": false, "-r": false, "--squash": false, "-s": false,
		"--author-email": true, "-A": true, "--match-head-commit": true,
	},
	"close":  {"--comment": true, "-c": true, "--delete-branch": false, "-d": false},
	"reopen": {"--comment": true, "-c": true},
	"review": {"--approve": false, "-a": false, "--comment": false, "-c": false, "--request-changes": false, "-r": false},
	"comment": {
		"--attach": true, "--create-if-none": false, "--delete-last": false, "--edit-last": false,
		"--editor": false, "-e": false, "--yes": false,
	},
	"ready": {"--undo": false},
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

// Only local inputs that stay the same on a rerun; a network lookup that
// failed once would otherwise turn the rerun into a new request.
func RequestID(req Request) string {
	fields, _ := json.Marshal([]string{req.Command, req.Title, req.Body, req.Directory})
	hash := sha256.Sum256(fields)
	return fmt.Sprintf("%x", hash)
}
