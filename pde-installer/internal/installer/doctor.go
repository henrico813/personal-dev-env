package installer

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"syscall"

	"pde-installer/internal/aqua"
	"pde-installer/internal/builds"
	chezmoibackend "pde-installer/internal/chezmoi"
	"pde-installer/internal/direct"
	"pde-installer/internal/manifest"
	"pde-installer/internal/npm"
	"pde-installer/internal/pkgsrc"
	"pde-installer/internal/run"
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
	cc := firstExecutable("cc", "gcc", "clang")
	if cc == "" {
		failures = append(failures, "C compiler: install a host C compiler and libc development headers")
	} else {
		if err := check("C compile/link probe", run.Command{Name: cc, Args: []string{"-x", "c", "-", "-o", "/dev/null"}, Stdin: "#include <stdio.h>\n#include <stdlib.h>\n#include <sys/types.h>\nint main(void){puts(\"ok\");return 0;}\n"}, "install working C compiler and libc development headers"); err != nil {
			return err
		}
	}
	cxx := firstExecutable("c++", "g++", "clang++")
	if cxx == "" {
		failures = append(failures, "C++ compiler: install a host C++ compiler and standard library headers")
	} else {
		if err := check("C++ compile/link probe", run.Command{Name: cxx, Args: []string{"-x", "c++", "-", "-o", "/dev/null"}, Stdin: "#include <iostream>\nint main(){std::cout << \"ok\";}\n"}, "install working C++ compiler and standard library headers"); err != nil {
			return err
		}
	}
	if err := check("make", run.Command{Name: "make", Args: []string{"-n", "-f", "-"}, Stdin: "all:\n\t@:\n"}, "install a host make implementation"); err != nil {
		return err
	}
	for _, tool := range []string{"sh", "tar", "gzip", "bzip2", "xz", "patch", "sed", "awk", "grep", "file"} {
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
	if err := manifest.Validate(); err != nil {
		failures = append(failures, "ownership: "+err.Error())
	}
	if err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, runner).Validate(); err != nil {
		failures = append(failures, "chezmoi source: "+err.Error())
	}
	if err := npm.New(config.Home, config.RepoRoot, config.PkgPrefix, runner).ValidateLock(); err != nil {
		failures = append(failures, "npm lock: "+err.Error())
	}
	for _, path := range []string{config.PkgSource, config.PkgPrefix, config.LocalBin, config.AquaRoot} {
		if !within(config.Home, path) {
			failures = append(failures, "destination outside HOME: "+path)
		}
		if ancestor, ok := writableAncestor(path); !ok {
			failures = append(failures, "unwritable destination: "+path+" (fix ownership or permissions on "+ancestor+")")
		}
	}
	if mode == preflightReport {
		pkg := pkgsrc.New(config.Home, runner)
		if _, err := os.Stat(pkg.SourceRoot); err == nil {
			if err := pkg.ValidateBootstrap(); err != nil {
				failures = append(failures, "pkgsrc source: "+err.Error())
			}
		}
		if _, err := os.Stat(pkg.Bmake()); err == nil {
			if _, err := fmt.Fprintf(runner.Out(), "ok      pkgsrc prefix %s\n", config.PkgPrefix); err != nil {
				return fmt.Errorf("write pkgsrc result: %w", err)
			}
		} else {
			if _, err := fmt.Fprintf(runner.Out(), "pending pkgsrc prefix %s (run install)\n", config.PkgPrefix); err != nil {
				return fmt.Errorf("write pkgsrc result: %w", err)
			}
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

func list(config config, runner run.Runner) error {
	pkgManager := pkgsrc.New(config.Home, runner)
	pkgStatuses := map[string]pkgsrc.Status{}
	statuses, err := pkgManager.Statuses()
	if err != nil {
		return fmt.Errorf("read pkgsrc status: %w", err)
	}
	for _, status := range statuses {
		pkgStatuses[status.Path] = status
	}
	aquaManager := aqua.New(config.Home, config.RepoRoot, runner)
	aquaInstalled, aquaState, err := aquaManager.Probe()
	if err != nil {
		return fmt.Errorf("read Aqua status: %w", err)
	}
	npmManager := npm.New(config.Home, config.RepoRoot, config.PkgPrefix, runner)
	directManager := direct.New(config.Home, config.PkgPrefix, runner)
	buildManager := builds.New(config.Home, config.RepoRoot, config.PkgPrefix, runner)
	chezmoiState, err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, runner).Probe()
	if err != nil {
		return fmt.Errorf("read chezmoi status: %w", err)
	}
	if _, err := fmt.Fprintln(runner.Out(), "OWNER\tITEM\tREQUESTED\tINSTALLED\tSTATUS"); err != nil {
		return fmt.Errorf("write list heading: %w", err)
	}
	for _, item := range manifest.Items() {
		requested, installed, state := item.Version, "", "missing"
		switch item.Owner {
		case manifest.Pkgsrc:
			status := pkgStatuses[item.Name]
			requested, installed, state = status.Requested, status.Installed, status.State
		case manifest.Aqua:
			if item.Name == "aqua" {
				installed, state = aquaInstalled, aquaState
			} else {
				installed, state, err = aquaManager.ToolProbe(item.Name, requested)
				if err != nil {
					return fmt.Errorf("read %s status: %w", item.Name, err)
				}
			}
		case manifest.NPM:
			installed, err = npmManager.Version(item.Name)
			if err != nil && !os.IsNotExist(err) {
				return fmt.Errorf("read %s status: %w", item.Name, err)
			}
			if installed == requested {
				state = "current"
			} else if installed != "" {
				state = "outdated"
			}
		case manifest.Local:
			if item.Name == "blink.cmp" {
				state, err = buildManager.BlinkStatus()
				if err != nil {
					return fmt.Errorf("read blink.cmp status: %w", err)
				}
			} else {
				state, err = buildManager.Probe(item.Name)
				if err != nil {
					return fmt.Errorf("read %s status: %w", item.Name, err)
				}
			}
		case manifest.Direct:
			for _, font := range direct.Fonts() {
				if font.Name == item.Name {
					state, err = directManager.Probe(font)
					if err != nil {
						return fmt.Errorf("read %s status: %w", item.Name, err)
					}
				}
			}
		case manifest.Chezmoi:
			state = chezmoiState
		}
		if _, err := fmt.Fprintf(runner.Out(), "%s\t%s\t%s\t%s\t%s\n", item.Owner, item.Name, requested, strings.TrimSpace(installed), state); err != nil {
			return fmt.Errorf("write list item %s: %w", item.Name, err)
		}
	}
	return nil
}
