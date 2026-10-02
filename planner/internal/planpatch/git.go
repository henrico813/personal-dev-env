// Package planpatch keeps patch mechanics out of model-authored text.
// Git operations use a disposable repository, never the user's index or worktree.
package planpatch

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// MaxPayload bounds model-authored patches and captured Git diagnostics so a
// single operation cannot exhaust memory. runGit, Read, and Generate enforce it.
const MaxPayload = 8 << 20

var objectID = regexp.MustCompile(`^(?:[0-9a-f]{40}|[0-9a-f]{64})$`)

// Change is one file patch, in the order it appears in the plan.
type Change struct {
	Target   string
	Filename string
	Diff     []byte
}

// File distinguishes an absent file (nil *File) from an existing empty file.
type File struct {
	Data []byte
	Mode string // Only regular files: 100644 or 100755.
}

// Session holds a private Git repository and index. Its alternates only read
// objects from the source repository; new objects are written under dir.
type Session struct{ dir string }

// limitedBuffer bounds memory without closing a child's output pipe early.
type limitedBuffer struct {
	bytes.Buffer
	truncated bool
}

func (b *limitedBuffer) Write(p []byte) (int, error) {
	n := len(p)
	room := MaxPayload - b.Len()
	if len(p) > room {
		p = p[:room]
		b.truncated = true
	}
	_, _ = b.Buffer.Write(p)
	return n, nil
}

// gitEnv prevents ambient Git variables and user-wide configuration from
// redirecting our index, enabling external helpers, or fetching missing objects.
func gitEnv() []string {
	env := make([]string, 0, len(os.Environ())+6)
	for _, item := range os.Environ() {
		key, _, _ := strings.Cut(item, "=")
		if !strings.HasPrefix(key, "GIT_") && key != "LC_ALL" {
			env = append(env, item)
		}
	}
	return append(env, "GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL="+os.DevNull,
		"GIT_TERMINAL_PROMPT=0", "GIT_NO_REPLACE_OBJECTS=1", "GIT_NO_LAZY_FETCH=1", "LC_ALL=C")
}

func runGit(dir string, input []byte, args ...string) ([]byte, error) {
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, "git",
		append([]string{"--literal-pathspecs", "-C", dir}, args...)...)
	cmd.Env = gitEnv()
	cmd.Stdin = bytes.NewReader(input)
	var out, diagnostic limitedBuffer
	cmd.Stdout, cmd.Stderr = &out, &diagnostic
	err := cmd.Run()
	if ctx.Err() != nil {
		return nil, fmt.Errorf("git timed out: %w", ctx.Err())
	}
	if err != nil {
		return nil, fmt.Errorf("git %s: %w: %s", args[0], err, strings.TrimSpace(diagnostic.String()))
	}
	if out.truncated || diagnostic.truncated {
		return nil, fmt.Errorf("git output exceeds %d bytes", MaxPayload)
	}
	return out.Bytes(), nil
}

func emptySession(format string) (*Session, error) {
	dir, err := os.MkdirTemp("", "planner-git-*")
	if err != nil {
		return nil, err
	}
	s := &Session{dir: dir}
	_, err = runGit(dir, nil, "init", "--bare", "--quiet", "--template=",
		"--object-format="+format, ".")
	if err != nil {
		s.Close()
		return nil, err
	}
	if _, err = s.git(nil, "read-tree", "--empty"); err != nil {
		s.Close()
		return nil, err
	}
	return s, nil
}

// Open requires a full immutable commit ID. Dirty and untracked source files
// are intentionally not part of this baseline. It does not stage or stash them.
func Open(repo, base string) (*Session, error) {
	if !objectID.MatchString(base) {
		return nil, failure(CodeBaseRequired, "use a full commit ID, not HEAD or a branch name")
	}
	root, err := filepath.Abs(repo)
	if err != nil {
		return nil, err
	}
	resolved, err := runGit(root, nil, "rev-parse", "--verify", "--end-of-options", base+"^{commit}")
	if err != nil {
		return nil, failure(CodeBaseUnavailable, "%w", err)
	}
	if strings.TrimSpace(string(resolved)) != base {
		return nil, failure(CodeBaseRequired, "expected a commit, not a tag object")
	}
	format := "sha1"
	if len(base) == 64 {
		format = "sha256"
	}
	objects, err := runGit(root, nil, "rev-parse", "--path-format=absolute", "--git-path", "objects")
	if err != nil {
		return nil, err
	}
	objectDir := strings.TrimSuffix(string(objects), "\n")
	if !filepath.IsAbs(objectDir) || strings.ContainsAny(objectDir, "\r\n\x00") {
		return nil, fmt.Errorf("unsupported source object directory")
	}
	s, err := emptySession(format)
	if err != nil {
		return nil, err
	}
	// A file, rather than a colon-separated environment variable, also handles
	// ordinary paths containing spaces or colons.
	err = os.WriteFile(filepath.Join(s.dir, "objects", "info", "alternates"),
		[]byte(objectDir+"\n"), 0600)
	if err == nil {
		_, err = s.git(nil, "read-tree", base)
	}
	if err != nil {
		s.Close()
		return nil, failure(CodeBaseUnavailable, "%w", err)
	}
	return s, nil
}

func (s *Session) git(input []byte, args ...string) ([]byte, error) {
	return runGit(s.dir, input, args...)
}

// Close removes the disposable repository directory. It is safe to call after a
// failed operation because the source repository was never modified.
func (s *Session) Close() { _ = os.RemoveAll(s.dir) }

// ValidatePath rejects traversal and Git metadata paths before invoking Git.
func ValidatePath(name string) error {
	if name == "" || path.IsAbs(name) || path.Clean(name) != name ||
		strings.ContainsAny(name, "\\\x00\r\n\t:") {
		return failure(CodeUnsupportedPath, "%q", name)
	}
	for _, part := range strings.Split(name, "/") {
		if part == ".." || part == "." || strings.EqualFold(part, ".git") {
			return failure(CodeUnsupportedPath, "%q", name)
		}
	}
	return nil
}

// Syntax asks Git to parse a single-file text patch. It does not prove that the
// patch applies; Apply checks that against the current disposable index.
func (s *Session) Syntax(change Change) error {
	if err := ValidatePath(change.Filename); err != nil {
		return err
	}
	if len(change.Diff) == 0 || len(change.Diff) > MaxPayload {
		return failure(CodePatchInvalid, "empty or oversized diff")
	}
	for _, line := range bytes.Split(change.Diff, []byte("\n")) {
		// These are transport delimiters, not legal unprefixed unified-diff
		// body lines. Do not silently remove them or alter quoted code lines.
		if bytes.HasPrefix(line, []byte("*** ")) {
			return failure(CodePatchEnvelope, "pass raw unified diff, without *** delimiters")
		}
	}
	stat, err := s.git(withFinalNewline(change.Diff), "apply", "--numstat", "-z", "-")
	if err != nil {
		return failure(CodePatchInvalid, "%w", err)
	}
	records := bytes.Split(stat, []byte{0})
	if len(records) != 2 || len(records[1]) != 0 {
		return failure(CodePatchPathMismatch, "expected exactly one file patch")
	}
	fields := bytes.SplitN(records[0], []byte("\t"), 3)
	if len(fields) != 3 || string(fields[2]) != change.Filename {
		return failure(CodePatchPathMismatch, "expected %q", change.Filename)
	}
	for _, count := range fields[:2] {
		if _, err := strconv.ParseUint(string(count), 10, 64); err != nil {
			return failure(CodePatchUnsupported, "binary or malformed patch")
		}
	}
	return nil
}

func withFinalNewline(raw []byte) []byte {
	if bytes.HasSuffix(raw, []byte("\n")) {
		return raw
	}
	return append(append([]byte(nil), raw...), '\n')
}

// Read returns a regular file from the current proposed index, not a worktree.
func (s *Session) Read(name string) (*File, error) {
	if err := ValidatePath(name); err != nil {
		return nil, err
	}
	raw, err := s.git(nil, "ls-files", "--stage", "-z", "--", name)
	if err != nil {
		return nil, err
	}
	if len(raw) == 0 {
		return nil, nil
	}
	records := bytes.Split(raw, []byte{0})
	if len(records) != 2 {
		return nil, failure(CodePatchUnsupported, "ambiguous index entry %q", name)
	}
	meta, gotPath, ok := strings.Cut(string(records[0]), "\t")
	fields := strings.Fields(meta)
	if !ok || gotPath != name || len(fields) != 3 || fields[2] != "0" {
		return nil, failure(CodePatchUnsupported, "invalid index entry")
	}
	if fields[0] != "100644" && fields[0] != "100755" {
		return nil, failure(CodePatchUnsupported, "symlink or submodule %q", name)
	}
	data, err := s.git(nil, "cat-file", "blob", fields[1])
	if err != nil {
		return nil, err
	}
	if bytes.IndexByte(data, 0) >= 0 {
		return nil, failure(CodePatchUnsupported, "binary file %q", name)
	}
	return &File{Data: data, Mode: fields[0]}, nil
}

// Apply replays one change. Failure may alter this disposable session, so the
// caller must discard it on error. Nothing is written to the source repository.
func (s *Session) Apply(change Change) error {
	fail := func(err error) error {
		return fmt.Errorf("%s (%s): %w", change.Target, change.Filename, err)
	}
	if err := s.Syntax(change); err != nil {
		return fail(err)
	}
	if _, err := s.Read(change.Filename); err != nil {
		return fail(err)
	}
	before, err := s.git(nil, "write-tree")
	if err != nil {
		return fail(err)
	}
	// --cached both checks and applies against our private index. Never use
	// --3way, --recount, --unsafe-paths, or whitespace rewriting implicitly.
	if _, err = s.git(withFinalNewline(change.Diff), "apply", "--cached",
		"--whitespace=nowarn", "-"); err != nil {
		return fail(failure(CodePatchNotApplicable, "%w", err))
	}
	changed, err := s.git(nil, "diff", "--cached", "--name-only", "-z",
		"--no-renames", strings.TrimSpace(string(before)), "--")
	if err != nil {
		return fail(err)
	}
	if string(changed) != change.Filename+"\x00" {
		return fail(failure(CodePatchPathMismatch, "rename, no-op, or undeclared file change"))
	}
	if _, err = s.Read(change.Filename); err != nil {
		return fail(err)
	}
	return nil
}

// Replay applies each change in order on top of base in a disposable
// repository. It stops at the first failure and never writes to the source
// repository. The caller decides whether changes is a full plan or a prefix.
func Replay(repo, base string, changes []Change) error {
	s, err := Open(repo, base)
	if err != nil {
		return err
	}
	defer s.Close()
	for _, change := range changes {
		if err := s.Apply(change); err != nil {
			return err
		}
	}
	return nil
}

// Generate uses Git to calculate headers, context, hunk counts, and EOF markers.
// It does not ask the model to maintain any of those fields.
func Generate(name string, before, after *File) ([]byte, error) {
	if err := ValidatePath(name); err != nil {
		return nil, err
	}
	if before == nil && after == nil {
		return nil, failure(CodePatchNoChange, "both files are absent")
	}
	s, err := emptySession("sha1")
	if err != nil {
		return nil, err
	}
	defer s.Close()
	put := func(file *File) error {
		if file == nil {
			_, err := s.git([]byte("0 "+strings.Repeat("0", 40)+"\t"+name+"\x00"),
				"update-index", "-z", "--index-info")
			return err
		}
		if len(file.Data) > MaxPayload || bytes.IndexByte(file.Data, 0) >= 0 {
			return failure(CodePatchUnsupported, "binary or oversized file")
		}
		if file.Mode != "100644" && file.Mode != "100755" {
			return failure(CodePatchUnsupported, "mode %q", file.Mode)
		}
		oid, err := s.git(file.Data, "hash-object", "-w", "--stdin")
		if err != nil {
			return err
		}
		_, err = s.git(nil, "update-index", "--add", "--cacheinfo", file.Mode,
			strings.TrimSpace(string(oid)), name)
		return err
	}
	if err := put(before); err != nil {
		return nil, err
	}
	tree, err := s.git(nil, "write-tree")
	if err != nil {
		return nil, err
	}
	if err := put(after); err != nil {
		return nil, err
	}
	diff, err := s.git(nil, "diff", "--cached", "--no-color", "--no-ext-diff",
		"--no-textconv", "--no-renames", "--src-prefix=a/", "--dst-prefix=b/",
		strings.TrimSpace(string(tree)), "--", name)
	if err != nil {
		return nil, err
	}
	if len(diff) == 0 {
		return nil, failure(CodePatchNoChange,
			"remove the obsolete plan change rather than storing an empty patch")
	}
	return diff, nil
}
