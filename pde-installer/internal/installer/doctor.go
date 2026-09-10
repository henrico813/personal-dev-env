package installer

import (
	"fmt"
	"os"
	"path/filepath"
	"syscall"

	chezmoibackend "pde-installer/internal/chezmoi"
	"pde-installer/internal/direct"
	"pde-installer/internal/manifest"
	"pde-installer/internal/npm"
	"pde-installer/internal/profile"
	"pde-installer/internal/run"
	"pde-installer/internal/tmux"
	"pde-installer/internal/ubuntu"
)

func doctor(config config, runner run.Runner) error {
	return hostPreflight(config, runner, preflightReport)
}

type preflightMode uint8

const (
	preflightQuiet preflightMode = iota + 1
	preflightReport
)

func hostPreflight(config config, runner run.Runner, mode preflightMode) error {
	if err := config.validateProfile(); err != nil {
		return err
	}
	var failures []string
	if mode == preflightReport && os.Geteuid() == 0 {
		failures = append(failures, "UID 0: run doctor as an unprivileged user")
	}
	check := func(name string, command run.Command, remediation string) error {
		if _, err := runner.Query("check "+name, command); err != nil {
			failures = append(failures, name+": "+remediation+" ("+err.Error()+")")
		} else if mode == preflightReport {
			if _, err := fmt.Fprintf(runner.Out(), "ok      %s\n", name); err != nil {
				return fmt.Errorf("write preflight result: %w", err)
			}
		}
		return nil
	}
	if !runner.DryRun || mode == preflightReport {
		if config.Profile == profile.Full {
			cc := firstExecutable("cc", "gcc", "clang")
			if cc == "" {
				failures = append(failures, "C compiler: install a host C compiler and libc development headers")
			} else if err := check("C compile/link probe", run.Command{Name: cc, Args: []string{"-x", "c", "-", "-o", "/dev/null"}, Stdin: "#include <stdio.h>\n#include <stdlib.h>\n#include <sys/types.h>\nint main(void){puts(\"ok\");return 0;}\n"}, "install working C compiler and libc development headers"); err != nil {
				return err
			}
			cxx := firstExecutable("c++", "g++", "clang++")
			if cxx == "" {
				failures = append(failures, "C++ compiler: install a host C++ compiler and standard library headers")
			} else if err := check("C++ compile/link probe", run.Command{Name: cxx, Args: []string{"-x", "c++", "-", "-o", "/dev/null"}, Stdin: "#include <iostream>\nint main(){std::cout << \"ok\";}\n"}, "install working C++ compiler and standard library headers"); err != nil {
				return err
			}
			if err := check("make", run.Command{Name: "make", Args: []string{"-n", "-f", "-"}, Stdin: "all:\n\t@:\n"}, "install a host make implementation"); err != nil {
				return err
			}
			for _, tool := range []string{"bzip2", "patch"} {
				if err := check(tool, run.Command{Name: tool, Args: probeArgs(tool)}, "install host "+tool); err != nil {
					return err
				}
			}
		}
		for _, tool := range []string{"sh", "tar", "gzip", "xz", "unzip", "sed", "awk", "grep", "file"} {
			command := run.Command{Name: tool, Args: probeArgs(tool)}
			if tool == "grep" {
				command.Stdin = "pde\n"
			}
			if err := check(tool, command, "install host "+tool); err != nil {
				return err
			}
		}
		if firstExecutable("curl", "fetch") == "" {
			failures = append(failures, "fetcher: install curl or fetch")
		} else if mode == preflightReport {
			if _, err := fmt.Fprintln(runner.Out(), "ok      curl or fetch"); err != nil {
				return fmt.Errorf("write fetcher result: %w", err)
			}
		}
	}
	if err := manifest.Validate(); err != nil {
		failures = append(failures, "ownership: "+err.Error())
	}
	if err := ubuntu.New(config.Profile, runner).Validate(); err != nil {
		failures = append(failures, "Ubuntu release: "+err.Error())
	}
	if err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, config.Profile, runner).Validate(); err != nil {
		failures = append(failures, "chezmoi source: "+err.Error())
	}
	if config.Profile == profile.Full {
		if err := npm.New(config.Home, config.RepoRoot, runner).ValidateLock(); err != nil {
			failures = append(failures, "npm lock: "+err.Error())
		}
		if _, err := direct.Tools(); err != nil {
			failures = append(failures, "direct tools: "+err.Error())
		}
	}
	paths := []string{config.LocalBin, config.AquaRoot, tmux.New(config.Home, runner).ReleaseRoot()}
	if config.Profile == profile.Full {
		paths = append(paths, direct.New(config.Home, runner).ToolsRoot())
	}
	for _, path := range paths {
		if !within(config.Home, path) {
			failures = append(failures, "destination outside HOME: "+path)
		}
		if ancestor, ok := writableAncestor(path); !ok {
			failures = append(failures, "unwritable destination: "+path+" (fix ownership or permissions on "+ancestor+")")
		}
	}
	if len(failures) > 0 {
		for _, failure := range failures {
			if _, err := fmt.Fprintln(runner.Err(), "error   "+failure); err != nil {
				return fmt.Errorf("write doctor failure: %w", err)
			}
		}
		return fmt.Errorf("doctor found %d problem(s)", len(failures))
	}
	return nil
}

func probeArgs(tool string) []string {
	switch tool {
	case "sh":
		return []string{"-c", ":"}
	case "unzip":
		return []string{"-Z", "-h"}
	case "tar", "gzip", "bzip2", "xz", "patch", "file":
		return []string{"--version"}
	case "sed":
		return []string{"-n", "1p", "/dev/null"}
	case "grep":
		return []string{"-q", "pde"}
	default:
		return []string{"BEGIN { exit 0 }"}
	}
}

func writableAncestor(path string) (string, bool) {
	ancestor := path
	for {
		info, err := os.Stat(ancestor)
		if err == nil {
			stat, ok := info.Sys().(*syscall.Stat_t)
			if !ok {
				return ancestor, info.Mode().Perm()&0o222 != 0
			}
			mode := info.Mode().Perm()
			if int(stat.Uid) == os.Geteuid() {
				return ancestor, mode&0o300 == 0o300
			}
			for _, group := range currentGroups() {
				if int(stat.Gid) == group {
					return ancestor, mode&0o030 == 0o030
				}
			}
			return ancestor, mode&0o003 == 0o003
		}
		if !os.IsNotExist(err) {
			return ancestor, false
		}
		parent := filepath.Dir(ancestor)
		if parent == ancestor {
			return ancestor, false
		}
		ancestor = parent
	}
}

func currentGroups() []int {
	groups, err := os.Getgroups()
	if err != nil {
		return []int{os.Getegid()}
	}
	return append(groups, os.Getegid())
}

func firstExecutable(names ...string) string {
	path := os.Getenv("PATH")
	for _, name := range names {
		if executable, err := run.LookPath(name, path); err == nil {
			return executable
		}
	}
	return ""
}
