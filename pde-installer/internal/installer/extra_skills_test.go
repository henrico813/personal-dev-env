package installer

import (
	"path/filepath"
	"testing"
)

// Only a PC with extra skills names their checkout, so a missing file or key
// must mean "no extra skills" rather than an error, and a leading ~/ must stand
// for HOME.
func TestLoadExtraSkillsRepo(t *testing.T) {
	tests := []struct {
		name string
		// saved is the config.json text, or "" to leave the file out.
		saved string
		// wantUnderHome is the folder a ~/ setting expands to, instead of want.
		wantUnderHome string
		want          string
	}{
		{
			name: "no config.json",
			want: "",
		},
		{
			name:  "config without the key",
			saved: `{"profile":"full"}`,
			want:  "",
		},
		{
			name:  "absolute path",
			saved: `{"extra_skills_repo":"/srv/extra-skills"}`,
			want:  "/srv/extra-skills",
		},
		{
			name:          "folder in HOME",
			saved:         `{"extra_skills_repo":"~/extra"}`,
			wantUnderHome: "extra",
		},
		{
			// Install reads the key before the migration renames it.
			name:  "old key name not yet migrated",
			saved: `{"old_skills_repo":"/srv/extra-skills"}`,
			want:  "/srv/extra-skills",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			if test.saved != "" {
				writeProfileFile(
					t,
					filepath.Join(
						home,
						".config",
						"pde",
						"config.json",
					),
					test.saved,
				)
			}

			got, err := loadExtraSkillsRepo(home)

			want := test.want
			if test.wantUnderHome != "" {
				want = filepath.Join(
					home,
					test.wantUnderHome,
				)
			}
			if err != nil {
				t.Fatalf("loadExtraSkillsRepo() error = %v", err)
			}
			if got != want {
				t.Fatalf("loadExtraSkillsRepo() = %q, want %q", got, want)
			}
		})
	}
}
