package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Verifies that pde-check gives each command a pass or fail line with its
// exit code and log, then the failure lines from failed logs, and prints
// nothing else. Agents read only this summary, so a wrong or extra line
// sends them to the wrong fix.
func TestPdeCheckReportsEachCommandResult(t *testing.T) {
	tests := map[string]struct {
		commands []string
		wantCode int
		want     []string
	}{
		"all pass": {
			commands: []string{
				"true",
				"echo ok",
			},
			wantCode: 0,
			want: []string{
				"pass (exit 0) OUT/test1.log: true",
				"pass (exit 0) OUT/test2.log: echo ok",
				"done",
			},
		},
		"failure line from the log": {
			commands: []string{
				"echo ok",
				`printf 'setup\n--- FAIL: TestSave\n'; exit 3`,
			},
			wantCode: 1,
			want: []string{
				"pass (exit 0) OUT/test1.log: echo ok",
				`fail (exit 3) OUT/test2.log: printf 'setup\n--- FAIL: TestSave\n'; exit 3`,
				"test2.log:2:--- FAIL: TestSave",
				"done",
			},
		},
		"last lines when nothing matches": {
			commands: []string{
				`printf 'a\nb\nc\nd\n'; exit 1`,
			},
			wantCode: 1,
			want: []string{
				`fail (exit 1) OUT/test1.log: printf 'a\nb\nc\nd\n'; exit 1`,
				"test1.log:2:b",
				"test1.log:3:c",
				"test1.log:4:d",
				"done",
			},
		},
		"error output goes to the log": {
			commands: []string{
				"echo oops >&2; exit 1",
			},
			wantCode: 1,
			want: []string{
				"fail (exit 1) OUT/test1.log: echo oops >&2; exit 1",
				"test1.log:1:oops",
				"done",
			},
		},
		"passing test lines are skipped": {
			commands: []string{
				`printf 'test parse_error ... ok\ntest save ... FAILED\n'; exit 101`,
			},
			wantCode: 1,
			want: []string{
				`fail (exit 101) OUT/test1.log: printf 'test parse_error ... ok\ntest save ... FAILED\n'; exit 101`,
				"test1.log:2:test save ... FAILED",
				"done",
			},
		},
		"binary bytes in the log": {
			commands: []string{
				`printf 'FAIL \0 here\nlast\n'; exit 1`,
			},
			wantCode: 1,
			want: []string{
				`fail (exit 1) OUT/test1.log: printf 'FAIL \0 here\nlast\n'; exit 1`,
				"test1.log:1:FAIL  here",
				"done",
			},
		},
		"multi-line command on one line": {
			commands: []string{
				"false\n#x",
			},
			wantCode: 1,
			want: []string{
				"fail (exit 1) OUT/test1.log: false #x",
				"done",
			},
		},
		"long command cut to 80 characters": {
			commands: []string{
				"echo 1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890",
			},
			wantCode: 0,
			want: []string{
				"pass (exit 0) OUT/test1.log: echo 123456789012345678901234567890123456789012345678901234567890123456789012345 [cut]",
				"done",
			},
		},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			dir := t.TempDir()
			out := filepath.Join(
				dir,
				"out",
			)

			args := append(
				[]string{"-o", out},
				test.commands...,
			)

			output, stderr, code := runPdeCheck(
				t,
				dir,
				args...,
			)

			want := strings.ReplaceAll(
				strings.Join(
					test.want,
					"\n",
				)+"\n",
				"OUT",
				out,
			)
			if code != test.wantCode || output != want || stderr != "" {
				t.Errorf(
					"pde-check = exit %d:\n%s\nstderr %q\nwant exit %d and no stderr:\n%s",
					code,
					output,
					stderr,
					test.wantCode,
					want,
				)
			}
		})
	}
}

// Verifies that six failing commands, the first with one failure line,
// give 10 short lines ending in "done", and that summary.txt holds the same
// lines. The cap keeps test output out of agents' context, and auto runs
// trust summary.txt only when its last line is "done".
func TestPdeCheckSavesCappedSummary(t *testing.T) {
	dir := t.TempDir()
	out := filepath.Join(
		dir,
		"out",
	)
	longCommand := "true " + strings.Repeat(
		"x",
		1000,
	) + `; for i in 1 2 3 4 5; do printf 'FAIL %01000d\n' "$i"; done; exit 1`
	// The first log has one failure line, so the next log gets two of the three slots.
	args := []string{
		"-o",
		out,
		"echo FAIL one; exit 1",
	}
	for i := 0; i < 5; i++ {
		args = append(
			args,
			longCommand,
		)
	}

	output, _, code := runPdeCheck(
		t,
		dir,
		args...,
	)

	lines := strings.Split(
		strings.TrimSuffix(
			output,
			"\n",
		),
		"\n",
	)
	if code != 1 || len(lines) != 10 || lines[9] != "done" {
		t.Errorf(
			"pde-check with six failing commands = exit %d, %d lines:\n%s\nwant exit 1 and 10 lines ending in done",
			code,
			len(lines),
			output,
		)
	}
	for _, line := range lines {
		if len(line) > len(out)+170 {
			t.Errorf(
				"pde-check line has %d characters, want at most %d:\n%s",
				len(line),
				len(out)+170,
				line,
			)
		}
	}
	summary, err := os.ReadFile(filepath.Join(
		out,
		"summary.txt",
	))
	if err != nil {
		t.Fatal(err)
	}
	if string(summary) != output {
		t.Errorf(
			"summary.txt = %q, want the printed summary %q",
			summary,
			output,
		)
	}
}

// Verifies that pde-check exits 2 and prints nothing to stdout for zero
// commands or for more than six. A call with no commands must not print
// "done" as a pass, and seven command lines would push the summary past 10
// lines.
func TestPdeCheckRejectsBadUsage(t *testing.T) {
	tests := map[string]struct {
		args []string
	}{
		"no commands": {
			args: nil,
		},
		"seven commands": {
			args: []string{
				"true",
				"true",
				"true",
				"true",
				"true",
				"true",
				"true",
			},
		},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			output, _, code := runPdeCheck(
				t,
				t.TempDir(),
				test.args...,
			)

			if code != 2 || output != "" {
				t.Errorf(
					"pde-check %q = exit %d, stdout %q\nwant exit 2 and no stdout",
					test.args,
					code,
					output,
				)
			}
		})
	}
}

// Verifies that without -o, pde-check writes its logs to a new folder
// inside TMPDIR and the summary names a log that holds the command's
// output. Agents open that log to read a failure in full, and a log
// written to the current folder would overwrite files in a checkout.
func TestPdeCheckMakesNewLogFolder(t *testing.T) {
	dir := t.TempDir()

	output, _, code := runPdeCheck(
		t,
		dir,
		"echo hello",
	)

	first, _, _ := strings.Cut(
		output,
		"\n",
	)
	log := strings.TrimSuffix(
		strings.TrimPrefix(
			first,
			"pass (exit 0) ",
		),
		": echo hello",
	)
	got, err := os.ReadFile(log)
	parent := filepath.Dir(filepath.Dir(log))
	if code != 0 || err != nil || string(got) != "hello\n" || parent != dir {
		t.Errorf(
			"pde-check without -o = exit %d, log %q holds %q, %v:\n%s\nwant exit 0 and a log in a new folder in %s that holds hello",
			code,
			log,
			got,
			err,
			output,
			dir,
		)
	}
}

// Runs pde-check from dir, with dir as TMPDIR, and returns its stdout,
// stderr and exit code.
func runPdeCheck(t *testing.T, dir string, args ...string) (string, string, int) {
	t.Helper()
	script := filepath.Join(
		repoRoot(t),
		"chezmoi",
		"dot_local",
		"bin",
		"executable_pde-check",
	)
	command := exec.Command(
		"bash",
		append(
			[]string{script},
			args...,
		)...,
	)
	command.Dir = dir
	command.Env = append(
		os.Environ(),
		"TMPDIR="+dir,
	)
	var stderr strings.Builder
	command.Stderr = &stderr
	output, err := command.Output()
	if _, exited := err.(*exec.ExitError); err != nil && !exited {
		t.Fatal(err)
	}
	return string(output), stderr.String(), command.ProcessState.ExitCode()
}
