package cmd

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os/exec"
	"strings"
	"time"
)

// How long card waits for gh, as the Python card does.
const ghTimeout = 5 * time.Second

// Returns the PR's state, MERGED, OPEN or CLOSED, from
// `gh pr view URL --json state --jq .state`. When gh fails, the error
// holds gh's own message.
func readPRState(url string) (string, error) {
	args := []string{"pr", "view", url, "--json", "state", "--jq", ".state"}
	shown := "gh " + strings.Join(args, " ")
	ctx, cancel := context.WithTimeout(context.Background(), ghTimeout)
	defer cancel()
	var stdout, stderr bytes.Buffer
	gh := exec.CommandContext(ctx, "gh", args...)
	gh.Stdout = &stdout
	gh.Stderr = &stderr
	if err := gh.Run(); err != nil {
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) && ctx.Err() == nil {
			return "", fmt.Errorf("%s failed: %s", shown, strings.TrimSpace(stderr.String()))
		}
		return "", fmt.Errorf("%s failed: %w", shown, err)
	}
	return strings.TrimSpace(stdout.String()), nil
}
