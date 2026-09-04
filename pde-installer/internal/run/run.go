package run

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// Command describes a subprocess invocation.
type Command struct {
	Name  string
	Args  []string
	Dir   string
	Env   []string
	Stdin string
}

// Runner executes commands and routes their output.
type Runner struct {
	DryRun         bool
	ReadOnlyDryRun bool
	Stdout, Stderr io.Writer
}

// Out returns the configured standard output destination.
func (r Runner) Out() io.Writer {
	if r.Stdout != nil {
		return r.Stdout
	}
	return os.Stdout
}

// Err returns the configured standard error destination.
func (r Runner) Err() io.Writer {
	if r.Stderr != nil {
		return r.Stderr
	}
	return os.Stderr
}

// Plan writes a dry-run action without executing it.
func (r Runner) Plan(description string, command *Command) error {
	if _, err := fmt.Fprintf(r.Out(), "DRY-RUN: %s\n", description); err != nil {
		return fmt.Errorf("write dry-run plan: %w", err)
	}
	if command != nil {
		if _, err := fmt.Fprintf(r.Out(), "  %s %s\n", command.Name, strings.Join(command.Args, " ")); err != nil {
			return fmt.Errorf("write dry-run command: %w", err)
		}
	}
	return nil
}

// Run executes a command or reports it during a dry run.
func (r Runner) Run(description string, command Command) error {
	if r.DryRun {
		return r.Plan(description, &command)
	}
	cmd, err := prepare(command)
	if err != nil {
		return fmt.Errorf("%s: %w", description, err)
	}
	cmd.Stdout, cmd.Stderr = r.Out(), r.Err()
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("%s: %w", description, err)
	}
	return nil
}

// RunOutput executes a command while retaining its combined output.
func (r Runner) RunOutput(description string, command Command) ([]byte, error) {
	cmd, err := prepare(command)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", description, err)
	}
	var output bytes.Buffer
	cmd.Stdout = io.MultiWriter(r.Out(), &output)
	cmd.Stderr = io.MultiWriter(r.Err(), &output)
	if err := cmd.Run(); err != nil {
		return output.Bytes(), fmt.Errorf("%s: %w", description, err)
	}
	return output.Bytes(), nil
}

// Query executes a command and returns its standard output.
func (r Runner) Query(description string, command Command) ([]byte, error) {
	cmd, err := prepare(command)
	if err != nil {
		return nil, fmt.Errorf("%s: %w", description, err)
	}
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	out, err := cmd.Output()
	if err != nil {
		if message := strings.TrimSpace(stderr.String()); message != "" {
			return nil, fmt.Errorf("%s: %s: %w", description, message, err)
		}
		return nil, fmt.Errorf("%s: %w", description, err)
	}
	return out, nil
}

// Retry repeats an operation with incremental delays.
func (r Runner) Retry(description string, attempts int, fn func() error) error {
	var err error
	for attempt := 1; attempt <= attempts; attempt++ {
		if err = fn(); err == nil {
			return nil
		}
		if attempt < attempts {
			if _, writeErr := fmt.Fprintf(r.Err(), "retry %d/%d for %s: %v\n", attempt, attempts, description, err); writeErr != nil {
				return fmt.Errorf("write retry notice: %w", writeErr)
			}
			time.Sleep(time.Duration(attempt) * time.Second)
		}
	}
	return err
}

func prepare(command Command) (*exec.Cmd, error) {
	name := command.Name
	env := environment(command.Env...)
	if !strings.ContainsRune(name, filepath.Separator) {
		var err error
		name, err = LookPath(name, envValue(env, "PATH"))
		if err != nil {
			return nil, err
		}
	}
	cmd := exec.Command(name, command.Args...)
	cmd.Dir, cmd.Env = command.Dir, env
	if command.Stdin != "" {
		cmd.Stdin = strings.NewReader(command.Stdin)
	}
	return cmd, nil
}

func environment(overrides ...string) []string {
	env := append([]string(nil), os.Environ()...)
	for _, override := range overrides {
		key, value, ok := strings.Cut(override, "=")
		if !ok {
			continue
		}
		next := make([]string, 0, len(env)+1)
		for _, entry := range env {
			if existing, _, _ := strings.Cut(entry, "="); existing != key {
				next = append(next, entry)
			}
		}
		env = append(next, key+"="+value)
	}
	return env
}

func envValue(env []string, key string) string {
	for _, entry := range env {
		if current, value, ok := strings.Cut(entry, "="); ok && current == key {
			return value
		}
	}
	return ""
}

// LookPath finds an executable using the supplied search path.
func LookPath(name, path string) (string, error) {
	for _, dir := range filepath.SplitList(path) {
		candidate := filepath.Join(dir, name)
		if info, err := os.Stat(candidate); err == nil && info.Mode().IsRegular() && info.Mode()&0o111 != 0 {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("executable %q not found in PATH", name)
}
