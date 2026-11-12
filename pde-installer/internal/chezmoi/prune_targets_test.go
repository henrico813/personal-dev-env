package chezmoi

import (
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"pde-installer/internal/testfile"
)

// Agents' scratch copies in /tmp each kept a target folder of up to 2.5 GB
// and filled the disk. Deleting a recent build or a folder cargo did not
// make would break live work or lose a user's files.
func TestPruneDeletesOnlyIdleScratchTargets(t *testing.T) {
	scratch := t.TempDir()
	idle := filepath.Join(
		scratch,
		"review.a1",
		"target",
	)
	writeCargoTarget(
		t,
		idle,
	)
	backdateFolder(
		t,
		idle,
	)
	recent := filepath.Join(
		scratch,
		"review.b2",
		"target",
	)
	writeCargoTarget(
		t,
		recent,
	)
	notCargo := filepath.Join(
		scratch,
		"notes",
		"target",
	)
	writeApplyFile(
		t,
		filepath.Join(
			notCargo,
			"plan.txt",
		),
		"keep\n",
	)
	backdateFolder(
		t,
		notCargo,
	)

	output := runPruneTargets(
		t,
		t.TempDir(),
		scratch,
	)

	if _, err := os.Stat(idle); !os.IsNotExist(err) {
		t.Errorf(
			"idle target %s still exists:\n%s",
			idle,
			output,
		)
	}
	for _, kept := range []string{recent, notCargo} {
		if _, err := os.Stat(kept); err != nil {
			t.Errorf(
				"%s was removed: %v\n%s",
				kept,
				err,
				output,
			)
		}
	}
}

// /proc lists resolved paths, so under a linked TMPDIR the script must find
// and compare resolved paths. Otherwise it skips idle targets or deletes a
// running test binary's folder.
func TestPruneKeepsBusyTargetsBehindLinks(t *testing.T) {
	scratch := t.TempDir()
	idle := filepath.Join(
		scratch,
		"review.a1",
		"target",
	)
	writeCargoTarget(
		t,
		idle,
	)
	backdateFolder(
		t,
		idle,
	)
	busy := filepath.Join(
		scratch,
		"review.c3",
		"work",
		"target",
	)
	writeCargoTarget(
		t,
		busy,
	)
	startProgramInside(
		t,
		busy,
	)
	backdateFolder(
		t,
		busy,
	)
	linkedScratch := filepath.Join(
		t.TempDir(),
		"tmp-link",
	)
	if err := os.Symlink(
		scratch,
		linkedScratch,
	); err != nil {
		t.Fatal(err)
	}

	output := runPruneTargets(
		t,
		t.TempDir(),
		linkedScratch,
	)

	if _, err := os.Stat(idle); !os.IsNotExist(err) {
		t.Errorf(
			"idle target %s still exists:\n%s",
			idle,
			output,
		)
	}
	if _, err := os.Stat(busy); err != nil {
		t.Errorf(
			"busy target %s was removed: %v\n%s",
			busy,
			err,
			output,
		)
	}
}

// A worktree whose pull requests are all merged or closed gets no more
// builds, but its target folder stayed until someone cleaned up by hand. An
// open pull request, or work with no pull request yet, keeps its build.
func TestPruneDeletesFinishedWorktreeTargets(t *testing.T) {
	home := t.TempDir()
	tests := []struct {
		branch      string
		worktree    string
		wantRemoved bool
	}{
		{
			branch:      "feat/merged",
			worktree:    "feat-merged",
			wantRemoved: true,
		},
		{
			branch:      "fix/closed",
			worktree:    "fix-closed",
			wantRemoved: true,
		},
		{
			branch:      "feat/closed-then-open",
			worktree:    "feat-closed-then-open",
			wantRemoved: false,
		},
		{
			branch:      "feat/no-pr",
			worktree:    "feat-no-pr",
			wantRemoved: false,
		},
	}
	targets := make(map[string]string)
	for _, test := range tests {
		worktree := filepath.Join(
			home,
			"Projects",
			"app",
			"worktrees",
			test.worktree,
		)
		runGit(
			t,
			".",
			"init",
			"--quiet",
			"--initial-branch",
			test.branch,
			worktree,
		)
		target := filepath.Join(
			worktree,
			"crate",
			"target",
		)
		writeCargoTarget(
			t,
			target,
		)
		backdateFolder(
			t,
			target,
		)
		targets[test.branch] = target
	}

	output := runPruneTargets(
		t,
		home,
		t.TempDir(),
	)

	for _, test := range tests {
		_, err := os.Stat(targets[test.branch])
		if removed := os.IsNotExist(err); removed != test.wantRemoved {
			t.Errorf(
				"%s target removed = %t, want %t:\n%s",
				test.branch,
				removed,
				test.wantRemoved,
				output,
			)
		}
	}
}

// A stand-in for gh pr list that prints the PR states of the branch after
// --head, one per line, as the script's --jq asks.
const fakePRStatesGH = `#!/bin/sh
while [ $# -gt 0 ] && [ "$1" != --head ]; do shift; done
case "$2" in
feat/merged) echo MERGED ;;
fix/closed) echo CLOSED ;;
feat/closed-then-open) printf 'CLOSED\nOPEN\n' ;;
esac
`

// Writes a target folder as cargo leaves it, with cargo's cache tag and one
// build file.
func writeCargoTarget(t *testing.T, dir string) {
	t.Helper()
	tag := "Signature: 8a477f597d28d172789f06886806bc55\n" +
		"# This file is a cache directory tag created by cargo.\n"
	writeApplyFile(
		t,
		filepath.Join(
			dir,
			"CACHEDIR.TAG",
		),
		tag,
	)
	writeApplyFile(
		t,
		filepath.Join(
			dir,
			"debug",
			"app",
		),
		"build output\n",
	)
}

// Makes every file and folder under dir three hours old, past the script's
// two-hour idle limit.
func backdateFolder(t *testing.T, dir string) {
	t.Helper()
	old := time.Now().Add(-3 * time.Hour)
	err := filepath.WalkDir(
		dir,
		func(
			path string,
			_ fs.DirEntry,
			err error,
		) error {
			if err != nil {
				return err
			}
			return os.Chtimes(
				path,
				old,
				old,
			)
		},
	)
	if err != nil {
		t.Fatal(err)
	}
}

// Runs a copy of sleep from inside a target, as a long test binary runs from
// target/debug/deps.
func startProgramInside(t *testing.T, target string) {
	t.Helper()
	sleep, err := exec.LookPath("sleep")
	if err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(sleep)
	if err != nil {
		t.Fatal(err)
	}
	program := filepath.Join(
		target,
		"debug",
		"deps",
		"sleep",
	)
	testfile.WriteExecutable(
		t,
		program,
		string(data),
	)
	running := exec.Command(
		program,
		"60",
	)
	if err := running.Start(); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = running.Process.Kill()
		_ = running.Wait()
	})
}

// Runs pde-prune-targets with test folders as HOME and TMPDIR and the fake
// gh first on PATH, so it never sees the real ones. Returns its output.
func runPruneTargets(t *testing.T, home, scratch string) string {
	t.Helper()
	fakeBin := t.TempDir()
	testfile.WriteExecutable(
		t,
		filepath.Join(
			fakeBin,
			"gh",
		),
		fakePRStatesGH,
	)
	script := filepath.Join(
		repoRoot(t),
		"chezmoi",
		"dot_local",
		"bin",
		"executable_pde-prune-targets",
	)
	command := exec.Command(
		"bash",
		script,
	)
	command.Env = append(
		isolatedGitEnv(os.Environ()),
		"HOME="+home,
		"TMPDIR="+scratch,
		"PATH="+fakeBin+string(os.PathListSeparator)+os.Getenv("PATH"),
	)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf(
			"pde-prune-targets: %v\n%s",
			err,
			output,
		)
	}
	return string(output)
}
