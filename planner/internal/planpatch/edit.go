package planpatch

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// Expect binds an edit to the entire plan snapshot, selected change, and source
// baseline. Changes to earlier steps invalidate it, even if the target is intact.
func Expect(raw []byte, target, base string) string {
	h := sha256.New()
	for _, part := range [][]byte{[]byte("planner-edit-v1"), raw, []byte(target), []byte(base)} {
		_, _ = h.Write(part)
		_, _ = h.Write([]byte{0})
	}
	return "sha256:" + hex.EncodeToString(h.Sum(nil))
}

// ReplaceDiff changes only a fenced body and, when necessary, its two fence
// delimiters. start/end follow ParseResult.DiffContents: end excludes the LF
// immediately before the closing fence. An empty body yields end == start-1.
// Everything outside the two fence lines remains byte-identical.
func ReplaceDiff(raw []byte, start, end int, diff []byte) ([]byte, error) {
	if start < 1 || end < start-1 || end >= len(raw) || raw[start-1] != '\n' || raw[end] != '\n' {
		return nil, fmt.Errorf("invalid diff span")
	}
	if bytes.ContainsAny(diff, "\r\x00") {
		return nil, failure(CodePatchUnsupported,
			"Markdown diff bodies must use LF and contain no NUL bytes")
	}
	diff = bytes.TrimSuffix(diff, []byte("\n")) // Transport newline, not source-code EOF.
	openStart := bytes.LastIndexByte(raw[:start-1], '\n') + 1
	closeEnd := len(raw)
	if i := bytes.IndexByte(raw[end+1:], '\n'); i >= 0 {
		closeEnd = end + 1 + i
	}
	opening, closing := string(raw[openStart:start-1]), string(raw[end+1:closeEnd])
	trimmed := strings.TrimSpace(opening)
	fenceLen := leadingTicks(trimmed)
	if fenceLen < 3 || strings.TrimSpace(closing) != strings.Repeat("`", fenceLen) {
		return nil, fmt.Errorf("invalid diff fence span")
	}
	needed := fenceLen
	for _, line := range bytes.Split(diff, []byte("\n")) {
		if n := leadingTicks(strings.TrimSpace(string(line))); n >= needed {
			needed = n + 1
		}
	}
	oldFence, newFence := strings.Repeat("`", fenceLen), strings.Repeat("`", needed)
	opening = strings.Replace(opening, oldFence, newFence, 1)
	closing = strings.Replace(closing, oldFence, newFence, 1)
	var out bytes.Buffer
	out.Write(raw[:openStart])
	out.WriteString(opening)
	out.WriteByte('\n')
	out.Write(diff)
	out.WriteByte('\n')
	out.WriteString(closing)
	out.Write(raw[closeEnd:])
	return out.Bytes(), nil
}

func leadingTicks(s string) int {
	n := 0
	for n < len(s) && s[n] == '`' {
		n++
	}
	return n
}

// WriteIfUnchanged serializes cooperating Planner writers and detects stale
// reads. It writes after to a temporary file in the plan's directory, compares
// before against the current bytes, then renames the temporary file over the
// plan. The rename replaces the inode: owner, group, ACLs, and extended
// attributes are not preserved, only the permission bits copied from the
// original file. Non-cooperating editors can still race the final
// comparison/rename, so callers must keep a single-writer policy during
// approval and mutation.
//
// A crash between creating <plan>.planner.lock and removing it leaves the lock
// behind. Later writes fail with PLAN_BUSY until an operator removes the lock
// after confirming no writer is active.
func WriteIfUnchanged(filename string, before, after []byte) error {
	abs, err := filepath.Abs(filename)
	if err != nil {
		return err
	}
	dir, err := filepath.EvalSymlinks(filepath.Dir(abs))
	if err != nil {
		return err
	}
	abs = filepath.Join(dir, filepath.Base(abs))
	info, err := os.Lstat(abs)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return failure(CodePlanUnsupported, "expected a regular, non-symlink plan")
	}
	lock, err := os.OpenFile(abs+".planner.lock", os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
	if err != nil {
		return failure(CodePlanBusy, "%w", err)
	}
	_ = lock.Close()
	defer func() { _ = os.Remove(abs + ".planner.lock") }()
	check := func() error {
		currentInfo, err := os.Lstat(abs)
		if err != nil {
			return err
		}
		if !currentInfo.Mode().IsRegular() || !os.SameFile(info, currentInfo) {
			return failure(CodePlanStale, "file was replaced; reread and reconcile")
		}
		current, err := os.ReadFile(abs)
		if err != nil {
			return err
		}
		if !bytes.Equal(current, before) {
			return failure(CodePlanStale, "content changed; reread and reconcile")
		}
		return nil
	}
	if err := check(); err != nil {
		return err
	}
	if bytes.Equal(before, after) {
		return nil
	}
	temp, err := os.CreateTemp(dir, ".planner-edit-*")
	if err != nil {
		return err
	}
	name := temp.Name()
	removeTemp := func() { _ = os.Remove(name) }
	if err := temp.Chmod(info.Mode().Perm()); err != nil {
		_ = temp.Close()
		removeTemp()
		return err
	}
	if _, err := temp.Write(after); err != nil {
		_ = temp.Close()
		removeTemp()
		return err
	}
	if err := temp.Sync(); err != nil {
		_ = temp.Close()
		removeTemp()
		return err
	}
	if err := temp.Close(); err != nil {
		removeTemp()
		return err
	}
	if err := check(); err != nil {
		removeTemp()
		return err
	}
	if err := os.Rename(name, abs); err != nil {
		removeTemp()
		return err
	}
	return nil
}

// WriteNew creates filename from data without replacing an existing file. It
// writes a synced temporary file beside the destination and links it into
// place, so a crash cannot publish partial bytes and a losing creator fails
// instead of replacing a plan.
func WriteNew(filename string, data []byte) error {
	dir := filepath.Dir(filename)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return err
	}
	temp, err := os.CreateTemp(dir, ".planner-new-*")
	if err != nil {
		return err
	}
	name := temp.Name()
	removeTemp := func() { _ = os.Remove(name) }
	if _, err := temp.Write(data); err != nil {
		_ = temp.Close()
		removeTemp()
		return err
	}
	if err := temp.Sync(); err != nil {
		_ = temp.Close()
		removeTemp()
		return err
	}
	if err := temp.Close(); err != nil {
		removeTemp()
		return err
	}
	if err := os.Link(name, filename); err != nil {
		removeTemp()
		return err
	}
	removeTemp()
	return nil
}
