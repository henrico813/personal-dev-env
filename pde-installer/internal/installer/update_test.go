package installer

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
	"pde-installer/internal/testfile"
)

// A pde-installer that logs each call to $HOME/install-calls and fails with
// "install broke" when FAKE_INSTALL_FAILS is set.
const fakeInstaller = `#!/bin/sh
printf '%s\n' "$*" >>"$HOME/install-calls"
if [ -n "$FAKE_INSTALL_FAILS" ]; then
	echo 'install broke' >&2
	exit 1
fi
`

// The installer in ~/.local/bin before the update.
const oldInstaller = "#!/bin/sh\necho old installer\n"

// A timer updates each machine with nobody watching, so a checkout with its
// own work, or no way to reach main, must not move. The saved reason is all
// the owner sees.
func TestUpdateRecordsWhyItChangedNothing(t *testing.T) {
	tests := []struct {
		name       string
		giveGit    []string
		wantResult string
		wantReason string
	}{
		{
			name: "tracked change",
			giveGit: []string{
				"rm",
				"--cached",
				"--quiet",
				"chezmoi/.keep",
			},
			wantResult: "skipped",
			wantReason: "the checkout has changes to tracked files",
		},
		{
			name: "feature branch",
			giveGit: []string{
				"switch",
				"--quiet",
				"--create",
				"feature/demo",
			},
			wantResult: "skipped",
			wantReason: "the checkout is on feature/demo, not main",
		},
		{
			name: "local commit",
			giveGit: []string{
				"commit",
				"--quiet",
				"--allow-empty",
				"-m",
				"local",
			},
			wantResult: "skipped",
			wantReason: "the checkout has commits that main does not have",
		},
		{
			name: "unreachable remote",
			giveGit: []string{
				"remote",
				"set-url",
				"origin",
				"/nonexistent/remote.git",
			},
			wantResult: "failed",
			wantReason: "fetch main from /nonexistent/remote.git: ",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fixture := setUpUpdateFixture(t)
			runTestGit(
				t,
				fixture.checkout,
				tt.giveGit...,
			)
			before := runTestGit(
				t,
				fixture.checkout,
				"rev-parse",
				"HEAD",
			)

			err := runUpdateCommand()

			if err == nil {
				t.Fatal("update error = nil, want the reason")
			}
			record := readUpdateRecord(
				t,
				fixture.home,
			)
			if record.Result != tt.wantResult || !strings.HasPrefix(
				record.Reason,
				tt.wantReason,
			) {
				t.Errorf(
					"update.json = %+v, want result %q and reason starting %q",
					record,
					tt.wantResult,
					tt.wantReason,
				)
			}
			if calls := readFileOrEmpty(
				t,
				filepath.Join(
					fixture.home,
					"install-calls",
				),
			); calls != "" {
				t.Errorf(
					"install calls = %q, want none",
					calls,
				)
			}
			if after := runTestGit(
				t,
				fixture.checkout,
				"rev-parse",
				"HEAD",
			); after != before {
				t.Errorf(
					"HEAD = %s, want it left at %s",
					after,
					before,
				)
			}
		})
	}
}

// Main may change the installer itself, so the old installer would install
// the new main wrongly. The update must install with the new one and keep it.
func TestUpdateInstallsMainUsingNewInstaller(t *testing.T) {
	fixture := setUpUpdateFixture(t)

	err := runUpdateCommand()

	if err != nil {
		t.Fatal(err)
	}
	if head := runTestGit(
		t,
		fixture.checkout,
		"rev-parse",
		"HEAD",
	); head != fixture.main {
		t.Errorf(
			"HEAD = %s, want main %s",
			head,
			fixture.main,
		)
	}
	wantCalls := "install --repo-root " + fixture.checkout + "\n"
	if calls := readFileOrEmpty(
		t,
		filepath.Join(
			fixture.home,
			"install-calls",
		),
	); calls != wantCalls {
		t.Errorf(
			"install calls = %q, want %q",
			calls,
			wantCalls,
		)
	}
	if installer := readFileOrEmpty(
		t,
		filepath.Join(
			fixture.home,
			".local",
			"bin",
			"pde-installer",
		),
	); installer != fakeInstaller {
		t.Errorf(
			"~/.local/bin/pde-installer = %q, want the new build",
			installer,
		)
	}
	if record := readUpdateRecord(
		t,
		fixture.home,
	); record.Result != "installed" || record.Commit != fixture.main {
		t.Errorf(
			"update.json = %+v, want installed at %s",
			record,
			fixture.main,
		)
	}
}

// A new installer that cannot install main must not replace the last one that
// could. The owner would otherwise lose the working installer to a bad merge.
func TestUpdateRecordsFailureAndKeepsInstaller(t *testing.T) {
	fixture := setUpUpdateFixture(t)
	t.Setenv(
		"FAKE_INSTALL_FAILS",
		"1",
	)

	err := runUpdateCommand()

	if err == nil {
		t.Fatal("update error = nil, want the install failure")
	}
	if installer := readFileOrEmpty(
		t,
		filepath.Join(
			fixture.home,
			".local",
			"bin",
			"pde-installer",
		),
	); installer != oldInstaller {
		t.Errorf(
			"~/.local/bin/pde-installer = %q, want the old one",
			installer,
		)
	}
	want := updateRecord{
		Result: "failed",
		Commit: fixture.main,
		Reason: "install: install broke",
	}
	if record := readUpdateRecord(
		t,
		fixture.home,
	); record.Result != want.Result || record.Commit != want.Commit || record.Reason != want.Reason {
		t.Errorf(
			"update.json = %+v, want %+v",
			record,
			want,
		)
	}
}

// A timer runs update every few minutes, and most runs find main unchanged.
// Installing each time would rebuild every tool and restart the hub watch.
func TestUpdateSkipsInstallOfInstalledMain(t *testing.T) {
	fixture := setUpUpdateFixture(t)
	if err := writeInstallRecord(
		fixture.home,
		installRecord{
			Commit:      fixture.main,
			Branch:      "main",
			InstalledAt: "2026-11-13T08:00:00Z",
		},
	); err != nil {
		t.Fatal(err)
	}

	err := runUpdateCommand()

	if err != nil {
		t.Fatal(err)
	}
	if calls := readFileOrEmpty(
		t,
		filepath.Join(
			fixture.home,
			"install-calls",
		),
	); calls != "" {
		t.Errorf(
			"install calls = %q, want none",
			calls,
		)
	}
	if record := readUpdateRecord(
		t,
		fixture.home,
	); record.Result != "current" {
		t.Errorf(
			"update.json = %+v, want current",
			record,
		)
	}
}

// The timer's update is the sync that picks up fixes made offline. A failed
// sync must not mark the update failed, because the fixes stay safe on their
// branch until the next sync.
func TestUpdateSurvivesFailedSkillFixSync(t *testing.T) {
	fixture := setUpUpdateFixture(t)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			fixture.home,
			".local",
			"bin",
			"skill-fix",
		),
		"#!/bin/sh\nprintf '%s\\n' \"$*\" >>\"$HOME/skill-fix-calls\"\nexit 1\n",
	)
	var stderr bytes.Buffer

	err := runUpdate(
		fixture.home,
		"",
		run.Runner{
			Stdout: io.Discard,
			Stderr: &stderr,
		},
	)

	if err != nil {
		t.Fatal(err)
	}
	if calls := readFileOrEmpty(
		t,
		filepath.Join(
			fixture.home,
			"skill-fix-calls",
		),
	); calls != "sync\n" {
		t.Errorf(
			"skill-fix calls = %q, want sync",
			calls,
		)
	}
	if want := "run skill-fix sync to retry"; !strings.Contains(
		stderr.String(),
		want,
	) {
		t.Errorf(
			"stderr = %q, want substring %q",
			stderr.String(),
			want,
		)
	}
}

// Checkouts often clone over SSH, but a timer has no SSH agent. A missed
// rewrite makes every timed fetch fail.
func TestToHTTPSURLRewritesSSHRemotes(t *testing.T) {
	tests := []struct {
		name string
		give string
		want string
	}{
		{
			name: "scp form",
			give: "git@github.com:example/personal-dev-env.git",
			want: "https://github.com/example/personal-dev-env.git",
		},
		{
			name: "ssh form",
			give: "ssh://git@github.com/example/personal-dev-env.git",
			want: "https://github.com/example/personal-dev-env.git",
		},
		{
			name: "https unchanged",
			give: "https://github.com/example/personal-dev-env.git",
			want: "https://github.com/example/personal-dev-env.git",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := toHTTPSURL(tt.give); got != tt.want {
				t.Errorf(
					"toHTTPSURL(%q) = %q, want %q",
					tt.give,
					got,
					tt.want,
				)
			}
		})
	}
}

// A HOME and a checkout to update, one commit behind main on a bare
// repository that stands in for GitHub.
type updateFixture struct {
	home     string
	checkout string
	main     string
}

// Returns a HOME whose config.json names a checkout one commit behind main.
// Its ~/.local/bin holds the old installer and a fake go that builds the fake one.
func setUpUpdateFixture(t *testing.T) updateFixture {
	t.Helper()
	home := t.TempDir()
	t.Setenv(
		"HOME",
		home,
	)
	t.Setenv(
		"PDE_REPO_ROOT",
		"",
	)
	gitConfig := filepath.Join(
		t.TempDir(),
		"gitconfig",
	)
	writeCommandTestFile(
		t,
		gitConfig,
		"[user]\n\tname = Test\n\temail = test@example.invalid\n",
	)
	seed := initTestCheckout(
		t,
		"main",
	)
	t.Setenv(
		"GIT_CONFIG_GLOBAL",
		gitConfig,
	)
	for _, path := range []string{
		"chezmoi/.keep",
		"planner/go.mod",
		"pde-installer/go.mod",
	} {
		writeCommandTestFile(
			t,
			filepath.Join(
				seed,
				path,
			),
			"",
		)
	}
	runTestGit(
		t,
		seed,
		"add",
		".",
	)
	runTestGit(
		t,
		seed,
		"commit",
		"--quiet",
		"-m",
		"checkout files",
	)
	remote := filepath.Join(
		t.TempDir(),
		"remote.git",
	)
	runTestGit(
		t,
		seed,
		"clone",
		"--quiet",
		"--bare",
		seed,
		remote,
	)
	checkout := filepath.Join(
		t.TempDir(),
		"checkout",
	)
	runTestGit(
		t,
		seed,
		"clone",
		"--quiet",
		remote,
		checkout,
	)
	runTestGit(
		t,
		seed,
		"commit",
		"--quiet",
		"--allow-empty",
		"-m",
		"new main",
	)
	runTestGit(
		t,
		seed,
		"push",
		"--quiet",
		remote,
		"main",
	)
	writeCommandTestFile(
		t,
		filepath.Join(
			home,
			".config",
			"pde",
			"config.json",
		),
		`{"install_path": "`+checkout+`"}`,
	)
	bin := filepath.Join(
		home,
		".local",
		"bin",
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			bin,
			"pde-installer",
		),
		oldInstaller,
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			bin,
			"go",
		),
		"#!/bin/sh\ncat >\"$4\" <<'EOF'\n"+fakeInstaller+"EOF\nchmod +x \"$4\"\n",
	)
	return updateFixture{
		home:     home,
		checkout: checkout,
		main: runTestGit(
			t,
			seed,
			"rev-parse",
			"HEAD",
		),
	}
}

// Runs pde-installer update as a timer does, with no --repo-root.
func runUpdateCommand() error {
	command := NewCommand()
	command.SetArgs([]string{"update"})
	command.SetOut(io.Discard)
	command.SetErr(io.Discard)
	return command.Execute()
}

// Returns the saved update.json in home.
func readUpdateRecord(t *testing.T, home string) updateRecord {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(
		home,
		".local",
		"state",
		"pde",
		"update.json",
	))
	if err != nil {
		t.Fatal(err)
	}
	var record updateRecord
	if err := json.Unmarshal(
		data,
		&record,
	); err != nil {
		t.Fatal(err)
	}
	return record
}
