package internal

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
)

func generateUnifiedDiff(filename, oldText, newText string) (string, error) {
	if err := ValidateFilenameShape(filename); err != nil {
		return "", err
	}
	if oldText == newText {
		return "", fmt.Errorf("replacement leaves %s unchanged", filename)
	}
	return renderUnifiedDiff(filename, oldText, newText)
}

func renderUnifiedDiff(filename, beforeText, afterText string) (string, error) {
	tmpDir, err := os.MkdirTemp("", "planner-generate-diff-")
	if err != nil {
		return "", err
	}
	defer os.RemoveAll(tmpDir)

	beforePath := filepath.Join(tmpDir, "before.txt")
	afterPath := filepath.Join(tmpDir, "after.txt")
	if err := os.WriteFile(beforePath, []byte(beforeText), 0o644); err != nil {
		return "", err
	}
	if err := os.WriteFile(afterPath, []byte(afterText), 0o644); err != nil {
		return "", err
	}

	cleanName := filepath.ToSlash(filepath.Clean(filename))
	cmd := exec.Command(
		"diff",
		"-u",
		"--label", "a/"+cleanName,
		"--label", "b/"+cleanName,
		beforePath,
		afterPath,
	)
	out, err := cmd.CombinedOutput()
	if err == nil {
		return "", fmt.Errorf("diff produced no output for %s", cleanName)
	}
	var exitErr *exec.ExitError
	if !errors.As(err, &exitErr) || exitErr.ExitCode() != 1 {
		return "", fmt.Errorf("run diff for %s: %w: %s", cleanName, err, string(out))
	}
	return "diff --git a/" + cleanName + " b/" + cleanName + "\n" + string(out), nil
}
