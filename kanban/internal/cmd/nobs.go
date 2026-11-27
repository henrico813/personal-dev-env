package cmd

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"kanban/internal/card"
)

// How long card waits for nobs, as the Python card does.
const nobsTimeout = 20 * time.Second

// Asks "nobs vault path [NAME]" for the vault, with a 20 second limit, and
// returns its Projects folder. When nobs fails, its own message is the error.
func askNobsForProjectsFolder(vault string) (string, error) {
	args := []string{"vault", "path"}
	if vault != "" {
		args = append(args, vault)
	}
	shown := "nobs " + strings.Join(args, " ")
	ctx, cancel := context.WithTimeout(context.Background(), nobsTimeout)
	defer cancel()
	var stdout, stderr bytes.Buffer
	nobs := exec.CommandContext(ctx, "nobs", args...)
	nobs.Stdout = &stdout
	nobs.Stderr = &stderr
	if err := nobs.Run(); err != nil {
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) && ctx.Err() == nil {
			return "", fmt.Errorf("%s failed: %s", shown, strings.TrimSpace(stderr.String()))
		}
		return "", fmt.Errorf("%s failed: %w", shown, err)
	}
	dir := filepath.Join(strings.TrimSpace(stdout.String()), card.ProjectsFolder)
	if info, err := os.Stat(dir); err != nil || !info.IsDir() {
		return "", fmt.Errorf("no folder %s", dir)
	}
	return dir, nil
}
