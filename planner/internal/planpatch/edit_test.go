package planpatch

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
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
// the source baseline (the full commit ID the change is measured against).
// Changing any of them invalidates the token, so an edit prepared against an
// old plan fails instead of overwriting a human edit.
func TestExpectBindsPlanTargetAndBaseline(t *testing.T) {
	original := Expect([]byte("first\nselected"), "target", strings.Repeat("a", 40))
	variants := map[string]string{
		"changed plan":   Expect([]byte("changed\nselected"), "target", strings.Repeat("a", 40)),
		"changed target": Expect([]byte("first\nselected"), "other", strings.Repeat("a", 40)),
		"changed base":   Expect([]byte("first\nselected"), "target", strings.Repeat("b", 40)),
		"no base":        Expect([]byte("first\nselected"), "target", ""),
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
