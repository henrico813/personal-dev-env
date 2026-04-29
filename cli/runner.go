package main

import (
	"fmt"
	"io"
	"os"
	"os/exec"
)

type Runner struct {
	DryRun bool
	Stdout io.Writer
	Stderr io.Writer
}

func (r Runner) Do(desc string, fn func() error) error {
	if r.DryRun {
		fmt.Fprintf(r.stdout(), "DRY-RUN: %s\n", desc)
		return nil
	}
	return fn()
}

func (r Runner) Bash(desc, script string) error {
	if r.DryRun {
		fmt.Fprintf(r.stdout(), "DRY-RUN: %s\n", desc)
		fmt.Fprintf(r.stdout(), "  bash -lc %s\n", script)
		return nil
	}

	cmd := exec.Command("bash", "-lc", script)
	cmd.Stdout = r.stdout()
	cmd.Stderr = r.stderr()
	return cmd.Run()
}

func (r Runner) WriteFile(desc, path string, data []byte, perm os.FileMode) error {
	if r.DryRun {
		fmt.Fprintf(r.stdout(), "DRY-RUN: %s (%s)\n", desc, path)
		return nil
	}
	return os.WriteFile(path, data, perm)
}

func (r Runner) MkdirAll(desc, path string, perm os.FileMode) error {
	if r.DryRun {
		fmt.Fprintf(r.stdout(), "DRY-RUN: %s (%s)\n", desc, path)
		return nil
	}
	return os.MkdirAll(path, perm)
}

func (r Runner) RemoveAll(desc, path string) error {
	if r.DryRun {
		fmt.Fprintf(r.stdout(), "DRY-RUN: %s (%s)\n", desc, path)
		return nil
	}
	return os.RemoveAll(path)
}

func (r Runner) Rename(desc, oldPath, newPath string) error {
	if r.DryRun {
		fmt.Fprintf(r.stdout(), "DRY-RUN: %s (%s -> %s)\n", desc, oldPath, newPath)
		return nil
	}
	return os.Rename(oldPath, newPath)
}

func (r Runner) stdout() io.Writer {
	if r.Stdout != nil {
		return r.Stdout
	}
	return os.Stdout
}

func (r Runner) stderr() io.Writer {
	if r.Stderr != nil {
		return r.Stderr
	}
	return os.Stderr
}
