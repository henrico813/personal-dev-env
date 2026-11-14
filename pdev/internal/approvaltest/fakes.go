package approvaltest

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// WriteScript writes body as an executable sh script named name in dir and
// returns its path.
func WriteScript(t *testing.T, dir, name, body string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte("#!/bin/sh\n"+body+"\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	return path
}

// FakeGH puts a fake gh on PATH that runs steps in order, and returns its
// path and call log. Steps are sh lines, such as GHAnswers and GHLogsCall,
// run with $log set to the log's path.
//
// pde-gh-write takes the first gh on PATH to be the guard ~/.local/bin/gh
// and runs the next one, so a decoy guard goes first and the fake is found
// as the real gh. The decoy exits 99, so running it by mistake shows.
func FakeGH(t *testing.T, steps ...string) (gh, log string) {
	t.Helper()
	gh, log = fake(t, "gh", strings.Join(steps, "\n"))
	decoy := t.TempDir()
	WriteScript(t, decoy, "gh", "exit 99")
	t.Setenv("PATH", decoy+":"+os.Getenv("PATH"))
	return gh, log
}

// RewriteFakeGH replaces the steps of the fake gh at gh, keeping its log,
// which FakeGH names <gh>.log.
func RewriteFakeGH(t *testing.T, gh string, steps ...string) {
	t.Helper()
	WriteScript(t, filepath.Dir(gh), "gh", "log='"+gh+".log'\n"+strings.Join(steps, "\n"))
}

// GHAnswers is a FakeGH step: when gh's first two arguments are command,
// such as "repo view", it prints reply and exits 0, so no later step runs.
func GHAnswers(command, reply string) string {
	return `if [ "$1 $2" = '` + command + `' ]; then echo '` + reply + `'; exit 0; fi`
}

// GHFails is a FakeGH step: when gh's first two arguments are command, it
// exits 1, so no later step runs.
func GHFails(command string) string {
	return `if [ "$1 $2" = '` + command + `' ]; then exit 1; fi`
}

// FakeGH steps that leave a record for the test to check.
const (
	// GHLogsCall adds a line to the call log for each call that reaches it.
	GHLogsCall = `echo call >> "$log"`
	// GHSavesBody saves the body gh reads from standard input when given
	// --body-file -, as pde-gh-write sends a body file, to the call log's
	// path with ".body" added. Given any other arguments, it saves nothing.
	GHSavesBody = `case " $* " in *" --body-file - "*) cat > "$log.body" ;; esac`
)

// FakeHerdr puts a fake herdr first on PATH and returns its call log, as
// FakeGH does.
func FakeHerdr(t *testing.T, script string) (log string) {
	t.Helper()
	_, log = fake(t, "herdr", script)
	return log
}

// FakeMoshiHook puts a fake moshi-hook first on PATH and returns its call
// log, as FakeGH does.
func FakeMoshiHook(t *testing.T, script string) (log string) {
	t.Helper()
	_, log = fake(t, "moshi-hook", script)
	return log
}

func fake(t *testing.T, name, script string) (path, log string) {
	t.Helper()
	dir := t.TempDir()
	log = filepath.Join(dir, name+".log")
	path = WriteScript(t, dir, name, "log='"+log+"'\n"+script)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	return path, log
}
