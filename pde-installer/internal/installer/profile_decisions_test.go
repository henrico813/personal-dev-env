package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

// Each profile's dry-run stages, apt packages and the chezmoi data its
// templates read. A change here changes what every machine of that profile
// installs.
func TestDryRunKeepsEachProfilesDecisions(t *testing.T) {
	tests := []struct {
		profile     string
		stages      []string
		packages    []string
		chezmoiData string
	}{
		{
			profile: "terminal",
			stages: []string{
				"update Ubuntu package index",
				"install Ubuntu dependencies",
				"download and verify",
				"atomically activate tmux 3.7b",
				"download and verify",
				"install Aqua packages in staging root",
				"atomically activate ~/.local/share/aquaproj-aqua",
				"migrate legacy PDE configuration",
				"preview chezmoi changes",
				"refresh OpenCode server environment",
			},
			packages: []string{
				"ca-certificates",
				"curl",
				"file",
				"git",
				"gzip",
				"tar",
				"unzip",
				"xclip",
				"xz-utils",
				"zsh",
			},
			chezmoiData: `{"pde":{"tiers":["terminal"]}}`,
		},
		{
			profile: "client",
			stages: []string{
				"update Ubuntu package index",
				"install Ubuntu dependencies",
				"download and verify",
				"atomically activate tmux 3.7b",
				"download and verify",
				"atomically activate direct release tools",
				"download and verify",
				"install Aqua packages in staging root",
				"atomically activate ~/.local/share/aquaproj-aqua",
				"materialize complete npm lock with pinned npm ci",
				"run required npm package install scripts in staging",
				"atomically activate npm prefix and executables",
				"download and verify",
				"atomically activate managed fonts",
				"refresh font cache",
				"build and atomically activate skill-lessons",
				"migrate legacy PDE configuration",
				"install and atomically activate opencode-inline.nvim",
				"preview chezmoi changes",
				"render skill lessons when skill-lessons is installed",
				"refresh OpenCode server environment",
			},
			packages: []string{
				"ca-certificates",
				"curl",
				"file",
				"fontconfig",
				"git",
				"gzip",
				"python3",
				"tar",
				"unzip",
				"xclip",
				"xz-utils",
				"zsh",
			},
			chezmoiData: `{"pde":{"tiers":["terminal","workstation"]}}`,
		},
		{
			profile: "full",
			stages: []string{
				"update Ubuntu package index",
				"install Ubuntu dependencies",
				"download and verify",
				"atomically activate tmux 3.7b",
				"download and verify",
				"atomically activate direct release tools",
				"download and verify",
				"install Aqua packages in staging root",
				"atomically activate ~/.local/share/aquaproj-aqua",
				"materialize complete npm lock with pinned npm ci",
				"run required npm package install scripts in staging",
				"atomically activate npm prefix and executables",
				"download and verify",
				"atomically activate managed fonts",
				"refresh font cache",
				"build and atomically activate planner",
				"build and atomically activate opencode-inline-shim",
				"build and atomically activate vibe",
				"build and atomically activate pde-gh-write",
				"build and atomically activate pde-comment-review",
				"build and atomically activate pde-comment-review-next",
				"build and atomically activate skill-lessons",
				"build and atomically activate pde-comment-checker",
				"migrate legacy PDE configuration",
				"install and atomically activate opencode-inline.nvim",
				"preview chezmoi changes",
				"download and verify",
				"build, verify, and atomically activate complete blink.cmp plugin tree",
				"render skill lessons when skill-lessons is installed",
				"refresh OpenCode server environment",
			},
			packages: []string{
				"build-essential",
				"bzip2",
				"ca-certificates",
				"curl",
				"file",
				"fontconfig",
				"gawk",
				"git",
				"gzip",
				"make",
				"patch",
				"python3",
				"tar",
				"unzip",
				"xclip",
				"xz-utils",
				"zsh",
			},
			chezmoiData: `{"pde":{"tiers":["terminal","workstation","development"]}}`,
		},
	}
	for _, test := range tests {
		t.Run(test.profile, func(t *testing.T) {
			home := t.TempDir()
			t.Setenv("HOME", home)
			t.Setenv("PATH", writeMissingPackagesBin(t)+string(os.PathListSeparator)+os.Getenv("PATH"))
			var output bytes.Buffer
			command := NewCommand()
			command.SetOut(&output)
			command.SetArgs([]string{"install", test.profile, "--dry-run", "--repo-root", testRepositoryRoot(t)})

			if err := command.Execute(); err != nil {
				t.Fatalf("install --dry-run: %v\n%s", err, output.String())
			}

			stages, packages := readDryRun(output.String(), home)
			if !slices.Equal(stages, test.stages) {
				t.Errorf("stages =\n%s\nwant\n%s", strings.Join(stages, "\n"), strings.Join(test.stages, "\n"))
			}
			if !slices.Equal(packages, test.packages) {
				t.Errorf("apt packages = %q, want %q", packages, test.packages)
			}
			if data := readChezmoiData(output.String()); data != test.chezmoiData {
				t.Errorf("chezmoi data = %s, want %s", data, test.chezmoiData)
			}
		})
	}
}

// Returns a PATH folder where dpkg-query reports every package missing, so the
// dry run lists the whole apt package set.
func writeMissingPackagesBin(t *testing.T) string {
	t.Helper()
	bin := t.TempDir()
	for name, script := range map[string]string{
		"apt-get":    "#!/bin/sh\nexit 0\n",
		"sudo":       "#!/bin/sh\nexit 0\n",
		"dpkg-query": "#!/bin/sh\nexit 1\n",
	} {
		if err := os.WriteFile(filepath.Join(bin, name), []byte(script), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	return bin
}

// Returns each dry-run stage without URLs or checksums, and the sorted apt
// packages. Repeated stages, such as one download per tool, count once.
func readDryRun(output, home string) ([]string, []string) {
	var stages, packages []string
	for _, line := range strings.Split(output, "\n") {
		if args, found := strings.CutPrefix(line, "  sudo apt-get install -y --no-install-recommends "); found {
			packages = strings.Fields(args)
			continue
		}
		stage, found := strings.CutPrefix(line, "DRY-RUN: ")
		if !found {
			continue
		}
		stage, _, _ = strings.Cut(stage, " https://")
		stage = strings.ReplaceAll(stage, home, "~")
		if len(stages) == 0 || stages[len(stages)-1] != stage {
			stages = append(stages, stage)
		}
	}
	slices.Sort(packages)
	return stages, packages
}

// Returns the --override-data value from the dry run's chezmoi command.
func readChezmoiData(output string) string {
	fields := strings.Fields(output)
	for i := 0; i+1 < len(fields); i++ {
		if fields[i] == "--override-data" {
			return fields[i+1]
		}
	}
	return ""
}
