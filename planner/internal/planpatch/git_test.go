package planpatch

import (
	"bytes"
	"crypto/sha256"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func gitOK(t *testing.T, repo string, args ...string) string {
	t.Helper()
	raw, err := runGit(repo, nil, args...)
	if err != nil {
		t.Fatal(err)
	}
	return strings.TrimSpace(string(raw))
}

func sourceRepo(t *testing.T) (string, string) {
	t.Helper()
	repo := t.TempDir()
	gitOK(t, repo, "init", "--quiet", "--template=")
	if err := os.WriteFile(filepath.Join(repo, "foo.txt"), []byte("A\n"), 0644); err != nil {
		t.Fatal(err)
	}
	gitOK(t, repo, "add", "--", "foo.txt")
	gitOK(t, repo, "-c", "user.name=Planner Test",
		"-c", "user.email=planner@example.invalid",
		"-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "baseline")
	return repo, gitOK(t, repo, "rev-parse", "HEAD")
}

func text(data string) *File { return &File{Data: []byte(data), Mode: "100644"} }

func generated(t *testing.T, before, after *File) Change {
	t.Helper()
	diff, err := Generate("foo.txt", before, after)
	if err != nil {
		t.Fatal(err)
	}
	return Change{Target: "implementation[1].file_changes[1]", Filename: "foo.txt", Diff: diff}
}

func failureCode(t *testing.T, err error) string {
	t.Helper()
	var patchErr *Error
	if !errors.As(err, &patchErr) {
		t.Fatalf("error does not carry a code: %v", err)
	}
	return patchErr.Code
}

func snapshot(t *testing.T, root string) map[string][32]byte {
	t.Helper()
	files := map[string][32]byte{}
	err := filepath.WalkDir(root, func(name string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		raw, err := os.ReadFile(name)
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(root, name)
		if err != nil {
			return err
		}
		files[rel] = sha256.Sum256(raw)
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return files
}

// Replay applies each change in order, so a B->C patch is checked against the
// A->B result rather than against the original file. It must also leave the
// user's checkout, index, and untracked files alone. Without both rules, coupled
// edits fail and validation can silently disturb the working tree.
func TestReplayUsesPriorStepsOnly(t *testing.T) {
	repo, base := sourceRepo(t)
	local := []byte("unrelated local edit\n")
	if err := os.WriteFile(filepath.Join(repo, "foo.txt"), local, 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(repo, "untracked.txt"),
		[]byte("keep me"), 0644); err != nil {
		t.Fatal(err)
	}
	before := snapshot(t, repo)
	first := generated(t, text("A\n"), text("B\n"))
	second := generated(t, text("B\n"), text("C\n"))
	second.Target = "implementation[2].file_changes[1]"
	// Ambient Git variables must not redirect the private index or worktree.
	t.Setenv("GIT_INDEX_FILE", filepath.Join(repo, ".git", "index"))
	t.Setenv("GIT_DIR", filepath.Join(repo, ".git"))
	t.Setenv("GIT_WORK_TREE", repo)
	t.Setenv("GIT_EXTERNAL_DIFF", "this-command-must-not-run")
	if err := Replay(repo, base, []Change{first, second}); err != nil {
		t.Fatal(err)
	}
	if err := Replay(repo, base, []Change{second}); err == nil {
		t.Fatal("accepted out-of-order replay")
	} else if failureCode(t, err) != CodePatchNotApplicable {
		t.Fatalf("wrong-order replay: %v", err)
	}
	after := snapshot(t, repo)
	if len(before) != len(after) {
		t.Fatal("source file set changed")
	}
	for name, hash := range before {
		if after[name] != hash {
			t.Fatalf("source changed: %s", name)
		}
	}
}

// Models that write diffs by hand often miscount hunk line counts and mangle
// quoted code. Git generates the headers and hunk counts here, so this checks
// that ordinary text plus empty, new, deleted, and executable files survive a
// generate-then-apply round trip with their content and mode intact.
func TestGenerateRoundTripsContentAndExistence(t *testing.T) {
	cases := []struct {
		name          string
		before, after *File
	}{
		{"normal", text("A\n"), text("B\n")},
		{"no final newline", text("A"), text("B")},
		{"blank final line", text("A\n"), text("A\n\n")},
		{"new empty", nil, text("")},
		{"new file", nil, text("B\n")},
		{"deleted file", text("A\n"), nil},
		{"deleted empty", text(""), nil},
		{"executable",
			&File{Data: []byte("A\n"), Mode: "100755"},
			&File{Data: []byte("B\n"), Mode: "100755"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			change := generated(t, tc.before, tc.after)
			s, err := emptySession("sha1")
			if err != nil {
				t.Fatal(err)
			}
			defer s.Close()
			if tc.before != nil {
				if err := s.Apply(generated(t, nil, tc.before)); err != nil {
					t.Fatal(err)
				}
			}
			if err := s.Apply(change); err != nil {
				t.Fatal(err)
			}
			got, err := s.Read("foo.txt")
			if err != nil {
				t.Fatal(err)
			}
			if (got == nil) != (tc.after == nil) {
				t.Fatalf("file existence mismatch: %+v", got)
			}
			if got != nil && (!bytes.Equal(got.Data, tc.after.Data) || got.Mode != tc.after.Mode) {
				t.Fatalf("content or mode mismatch: %+v", got)
			}
		})
	}
}

// Replay refuses patches that name another file, declare two files, or escape
// the tree, and it leaves the source repository untouched when it rejects one.
// Each case checks the exact error code, because callers branch on the code
// instead of matching Git's changing error text.
func TestRejectBadPatchUndeclaredPaths(t *testing.T) {
	repo, base := sourceRepo(t)
	good := generated(t, text("A\n"), text("B\n"))
	cases := []struct {
		name     string
		change   Change
		wantCode string
	}{
		{"not a patch", Change{
			Target: "target", Filename: "foo.txt",
			Diff: []byte("definitely not a patch"),
		}, CodePatchInvalid},
		{"bad counts", Change{
			Target: "target", Filename: "foo.txt",
			Diff: bytes.Replace(good.Diff, []byte("@@ -1 +1 @@"),
				[]byte("@@ -1,99 +1,99 @@"), 1),
		}, CodePatchInvalid},
		{"foreign envelope", Change{
			Target: "target", Filename: "foo.txt",
			Diff: append(append([]byte(nil), good.Diff...),
				[]byte("*** End Patch\n")...),
		}, CodePatchEnvelope},
		{"wrong file", Change{
			Target: "target", Filename: "other.txt", Diff: good.Diff,
		}, CodePatchPathMismatch},
		{"two file sections", Change{
			Target: "target", Filename: "foo.txt",
			Diff: append(append([]byte(nil), good.Diff...), good.Diff...),
		}, CodePatchPathMismatch},
		{"traversal", Change{
			Target: "target", Filename: "../foo.txt", Diff: good.Diff,
		}, CodeUnsupportedPath},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			before := snapshot(t, repo)
			err := Replay(repo, base, []Change{tc.change})
			if err == nil {
				t.Fatal("accepted invalid patch")
			}
			if got := failureCode(t, err); got != tc.wantCode {
				t.Fatalf("code = %q, want %q: %v", got, tc.wantCode, err)
			}
			after := snapshot(t, repo)
			for name, hash := range before {
				if after[name] != hash {
					t.Fatalf("failure modified %s", name)
				}
			}
		})
	}
}

// A real change can contain text that looks like patch syntax, such as a line
// "*** End Patch" or a fence line. Git must treat that text as ordinary diff
// content and split far-apart edits into separate hunks; otherwise the diff is
// unusable or a model has to count lines by hand.
func TestGeneratedPatchHandlesSeparatedHunks(t *testing.T) {
	var lines []string
	for i := 0; i < 2200; i++ {
		lines = append(lines, fmt.Sprintf("line %d", i))
	}
	before := strings.Join(lines, "\n") + "\n"
	lines[10], lines[2180] = "*** End Patch", "```"
	after := strings.Join(lines, "\n") + "\n"
	change := generated(t, text(before), text(after))
	if got := bytes.Count(change.Diff, []byte("\n@@")); got != 2 {
		t.Fatalf("expected two bounded hunks, got %d", got)
	}
	s, err := emptySession("sha1")
	if err != nil {
		t.Fatal(err)
	}
	defer s.Close()
	if err := s.Apply(generated(t, nil, text(before))); err != nil {
		t.Fatal(err)
	}
	if err := s.Apply(change); err != nil {
		t.Fatal(err)
	}
	file, err := s.Read("foo.txt")
	if err != nil {
		t.Fatal(err)
	}
	if string(file.Data) != after {
		t.Fatal("did not preserve literal source text")
	}
}

// The baseline is the full commit ID the plan's source is measured against, so
// Open rejects a moving name like HEAD instead of reading whatever the branch
// points at now. BASE_REQUIRED and BASE_UNAVAILABLE stay distinct so a caller
// can tell a wrong argument shape from a commit the repository does not have.
func TestOpenRejectsNonCommitBaseline(t *testing.T) {
	repo, _ := sourceRepo(t)
	cases := []struct {
		name, base, wantCode string
	}{
		{"branch name", "HEAD", CodeBaseRequired},
		{"short name", "main", CodeBaseRequired},
		{"missing commit", strings.Repeat("a", 40), CodeBaseUnavailable},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := Open(repo, tc.base)
			if err == nil {
				t.Fatalf("accepted baseline %q", tc.base)
			}
			if got := failureCode(t, err); got != tc.wantCode {
				t.Fatalf("code = %q, want %q: %v", got, tc.wantCode, err)
			}
		})
	}
}

// Path validation runs before Git, so traversal and Git metadata must be
// rejected here instead of reaching the source repository. The accepted paths
// show the intended limit, so the guard does not later widen into rejecting
// ordinary repository-relative filenames.
func TestValidatePathRejectsUnsupported(t *testing.T) {
	rejected := []string{
		"../x", "a/../x", "/tmp/x", ".git/config",
		"a/.GIT/x", "C:/x", "a\\b", "./x", "", "a\tb", "a\rb",
	}
	for _, name := range rejected {
		if err := ValidatePath(name); err == nil {
			t.Fatalf("accepted %q", name)
		} else if got := failureCode(t, err); got != CodeUnsupportedPath {
			t.Fatalf("code for %q = %q, want %q", name, got, CodeUnsupportedPath)
		}
	}
	for _, name := range []string{"foo.txt", "dir/foo.txt", "a.b-c_d/e"} {
		if err := ValidatePath(name); err != nil {
			t.Fatalf("rejected %q: %v", name, err)
		}
	}
}

// Generate feeds ordinary source into Git, so binary bytes, symlinks, and
// unsupported modes must fail with a code the caller can report. Two absent
// files are a no-op rather than an empty diff, because an empty patch stores no
// change and should not sit in a plan.
func TestGenerateRejectsUnsupportedInput(t *testing.T) {
	cases := []struct {
		name          string
		before, after *File
		wantCode      string
	}{
		{"binary bytes", nil, text("a\x00b"), CodePatchUnsupported},
		{"symlink mode", nil, &File{Data: []byte("target"), Mode: "120000"}, CodePatchUnsupported},
		{"unsupported mode", nil, &File{Data: []byte("x"), Mode: "100600"}, CodePatchUnsupported},
		{"both absent", nil, nil, CodePatchNoChange},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := Generate("foo.txt", tc.before, tc.after)
			if err == nil {
				t.Fatal("accepted unsupported input")
			}
			if got := failureCode(t, err); got != tc.wantCode {
				t.Fatalf("code = %q, want %q: %v", got, tc.wantCode, err)
			}
		})
	}
}
