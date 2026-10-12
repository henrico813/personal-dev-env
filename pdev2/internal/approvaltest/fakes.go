package approvaltest

import (
	"os"
	"path/filepath"
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

// FakeGH puts a fake gh first on PATH and returns its path, for code that
// takes the gh to run, and its call log. script runs with $log set to the
// log's path, so it records only what it writes there.
func FakeGH(t *testing.T, script string) (gh, log string) {
	t.Helper()
	return fake(t, "gh", script)
}

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
