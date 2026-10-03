package planpatch

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

// A diff body can quote backticks from the source. The fence around the body
// must grow past them, and every byte outside the two fence lines must stay the
// same, or the edit would close the block early and wipe out nearby notes.
func TestReplaceDiffPreservesAndGrowsFences(t *testing.T) {
	prefix := "---\nuser: keep raw wrapper\n---\n# Plan\nHuman note.\n"
	suffix := "\nAnother human note.\n- [X] Keep capitalization\n"
	raw := []byte(prefix + "```diff\nold body\n```" + suffix)
	start := strings.Index(string(raw), "old body")
	end := start + len("old body")
	replacement := []byte("--- a/readme.md\n+++ b/readme.md\n@@ -1 +1 @@\n ```\n")
	out, err := ReplaceDiff(raw, start, end, replacement)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasPrefix(out, []byte(prefix)) || !bytes.HasSuffix(out, []byte(suffix)) {
		t.Fatal("collateral change")
	}
	if !bytes.Contains(out, []byte("````diff\n")) || !bytes.Contains(out, []byte("\n````"+suffix)) {
		t.Fatalf("fence not enlarged:\n%s", out)
	}
	if !bytes.Contains(out, bytes.TrimSuffix(replacement, []byte("\n"))) {
		t.Fatal("replacement altered")
	}
}

// A file change added by hand can have an empty code block. Replacing that
// empty body must insert the new diff instead of rejecting a span with no
// content; otherwise the change can only be fixed by editing the diff by hand.
func TestReplaceDiffAcceptsEmptyBody(t *testing.T) {
	prefix := "# Plan\nHuman note.\n"
	suffix := "\nTail note.\n"
	raw := []byte(prefix + "```diff\n```" + suffix)
	start := len(prefix) + len("```diff\n")
	end := start - 1
	replacement := []byte("--- a/x.md\n+++ b/x.md\n@@ -0,0 +1 @@\n+new\n")
	out, err := ReplaceDiff(raw, start, end, replacement)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.HasPrefix(out, []byte(prefix)) || !bytes.HasSuffix(out, []byte(suffix)) {
		t.Fatalf("collateral change:\n%s", out)
	}
	if !bytes.Contains(out, bytes.TrimSuffix(replacement, []byte("\n"))) {
		t.Fatalf("replacement missing:\n%s", out)
	}
	if !bytes.Contains(out, []byte("```diff\n--- a/x.md")) {
		t.Fatalf("fence body not filled:\n%s", out)
	}
}

// The edit_expect token is a hash of the plan text, the selected change, and
// the source base commit (the full commit ID the change is measured against).
// Changing any of them invalidates the token, so an edit prepared against an
// old plan fails instead of overwriting a human edit.
func TestExpectBindsPlanTargetAndBaseCommit(t *testing.T) {
	original := Expect([]byte("first\nselected"), "target", strings.Repeat("a", 40))
	variants := map[string]string{
		"changed plan":        Expect([]byte("changed\nselected"), "target", strings.Repeat("a", 40)),
		"changed target":      Expect([]byte("first\nselected"), "other", strings.Repeat("a", 40)),
		"changed base commit": Expect([]byte("first\nselected"), "target", strings.Repeat("b", 40)),
		"no base commit":      Expect([]byte("first\nselected"), "target", ""),
	}
	for name, token := range variants {
		if original == token {
			t.Fatalf("%s did not invalidate token", name)
		}
	}
}

// The writer refuses to overwrite a plan that changed since it was read, and a
// leftover lock or a symlink is rejected rather than followed. A successful
// write replaces the content while keeping the permission bits and leaving no
// lock behind, so a stale editor cannot silently undo a human edit.
func TestWriteRejectsStaleBusySymlink(t *testing.T) {
	filename := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(filename, []byte("human edit"), 0640); err != nil {
		t.Fatal(err)
	}
	if err := WriteIfUnchanged(filename, []byte("old"), []byte("new")); err == nil {
		t.Fatal("accepted stale file")
	}
	if err := os.WriteFile(filename+".planner.lock", nil, 0600); err != nil {
		t.Fatal(err)
	}
	if err := WriteIfUnchanged(filename, []byte("human edit"), []byte("new")); err == nil {
		t.Fatal("ignored lock")
	}
	if err := os.Remove(filename + ".planner.lock"); err != nil {
		t.Fatal(err)
	}
	link := filename + ".link"
	if err := os.Symlink(filename, link); err != nil {
		t.Fatal(err)
	}
	if err := WriteIfUnchanged(link, []byte("human edit"), []byte("new")); err == nil {
		t.Fatal("accepted symlink")
	}
	raw, err := os.ReadFile(filename)
	if err != nil {
		t.Fatal(err)
	}
	if string(raw) != "human edit" {
		t.Fatal("failure lost human content")
	}
	if err := WriteIfUnchanged(filename, raw, []byte("accepted")); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(filename)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode().Perm() != 0640 {
		t.Fatal("permissions changed")
	}
	if _, err := os.Stat(filename + ".planner.lock"); !os.IsNotExist(err) {
		t.Fatal("lock leaked")
	}
}

// Plan diff bodies are LF-only text with no NUL bytes. A bad span or a CRLF body
// is rejected without touching the caller's bytes, so malformed model output
// cannot corrupt the plan it is inserted into.
func TestBadSpanAndCRLFRejected(t *testing.T) {
	raw := []byte("```diff\nold\n```\n")
	copyOfRaw := append([]byte(nil), raw...)
	if _, err := ReplaceDiff(raw, -1, 3, []byte("new")); err == nil {
		t.Fatal("accepted bad span")
	}
	if _, err := ReplaceDiff(raw, 8, 11, []byte("new\r\n")); err == nil {
		t.Fatal("accepted unsupported CRLF")
	}
	if !bytes.Equal(raw, copyOfRaw) {
		t.Fatal("changed original input")
	}
}

// Two concurrent planner new runs must not both create the same plan. The loser
// has to fail instead of replacing the winner's file, or one creation would
// silently destroy the other's plan.
func TestWriteNewAllowsOneWriter(t *testing.T) {
	path := filepath.Join(t.TempDir(), "plan.md")
	contents := [][]byte{[]byte("first\n"), []byte("second\n")}
	errs := make(chan error, len(contents))
	var wg sync.WaitGroup
	for _, content := range contents {
		content := content
		wg.Add(1)
		go func() {
			defer wg.Done()
			errs <- WriteNew(path, content)
		}()
	}
	wg.Wait()
	close(errs)

	succeeded := 0
	for err := range errs {
		if err == nil {
			succeeded++
			continue
		}
		if !os.IsExist(err) {
			t.Fatalf("losing writer error=%v, want destination-exists error", err)
		}
	}
	if succeeded != 1 {
		t.Fatalf("successful writers=%d, want 1", succeeded)
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if string(got) != "first\n" && string(got) != "second\n" {
		t.Fatalf("unexpected content: %q", got)
	}
}

// Rerunning planner new on a finished plan must fail and leave the plan bytes
// alone, or a stray invocation would wipe an approved plan.
func TestWriteNewKeepsExistingFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(path, []byte("human edit"), 0o644); err != nil {
		t.Fatal(err)
	}

	if err := WriteNew(path, []byte("replacement")); err == nil {
		t.Fatal("overwrote existing file")
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "human edit" {
		t.Fatalf("existing bytes changed: %q", got)
	}
}
