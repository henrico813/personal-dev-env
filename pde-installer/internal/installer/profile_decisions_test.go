package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"pde-installer/internal/profile"
	"pde-installer/internal/run"
	"pde-installer/internal/ubuntu"
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
			chezmoiData: `{"pde":{"desktop":"","tiers":["terminal"]}}`,
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
			chezmoiData: `{"pde":{"desktop":"","tiers":["terminal","workstation"]}}`,
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
			chezmoiData: `{"pde":{"desktop":"","tiers":["terminal","workstation","development"]}}`,
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

// A desktop on a saved profile keeps that profile's stages and packages, adds
// its own packages, player build and chezmoi data, and calls sudo only for apt.
// Any other sudo call would break the installer's rule of changing nothing
// outside HOME.
func TestDryRunAddsDesktopToProfile(t *testing.T) {
	tests := []struct {
		profile, desktop, player string
		addedPackages            []string
		chezmoiData              string
	}{
		{
			profile: "client",
			desktop: "sway",
			player:  "mpvpaper",
			addedPackages: []string{
				"blueman",
				"bluez",
				"brightnessctl",
				"build-essential",
				"dunst",
				"fonts-noto-color-emoji",
				"greybird-gtk-theme",
				"grim",
				"grimshot",
				"jq",
				"libegl-dev",
				"libmpv-dev",
				"libnotify-bin",
				"libwayland-dev",
				"lxpolkit",
				"meson",
				"mpv",
				"network-manager-gnome",
				"ninja-build",
				"papirus-icon-theme",
				"pavucontrol",
				"pkg-config",
				"playerctl",
				"psmisc",
				"pulseaudio-utils",
				"rofi",
				"slurp",
				"sway",
				"swaybg",
				"swayidle",
				"swaylock",
				"thunar",
				"ubuntu-wallpapers",
				"waybar",
				"wayland-protocols",
				"wl-clipboard",
				"xdg-desktop-portal-wlr",
				"xwayland",
			},
			chezmoiData: `{"pde":{"desktop":"sway","tiers":["terminal","workstation"]}}`,
		},
		{
			profile: "full",
			desktop: "i3",
			player:  "xwinwrap",
			addedPackages: []string{
				"arandr",
				"autorandr",
				"blueman",
				"bluez",
				"brightnessctl",
				"cava",
				"dex",
				"dunst",
				"feh",
				"fonts-noto-color-emoji",
				"greybird-gtk-theme",
				"i3-wm",
				"i3lock",
				"libnotify-bin",
				"libx11-dev",
				"libxext-dev",
				"libxrender-dev",
				"lxappearance",
				"lxpolkit",
				"mpv",
				"network-manager-gnome",
				"papirus-icon-theme",
				"pavucontrol",
				"picom",
				"playerctl",
				"polybar",
				"psmisc",
				"pulseaudio-utils",
				"rofi",
				"thunar",
				"ubuntu-wallpapers",
				"x11-xserver-utils",
				"xfce4-power-manager",
				"xfce4-screenshooter",
				"xss-lock",
			},
			chezmoiData: `{"pde":{"desktop":"i3","tiers":["terminal","workstation","development"]}}`,
		},
	}
	for _, test := range tests {
		t.Run(test.profile+" "+test.desktop, func(t *testing.T) {
			home := t.TempDir()
			t.Setenv("HOME", home)
			t.Setenv("PATH", writeMissingPackagesBin(t)+string(os.PathListSeparator)+os.Getenv("PATH"))
			// Desktops need a newer release than the tool profiles, and the
			// command reads this host's.
			if err := ubuntu.New("full", profile.Desktop(test.desktop), run.Runner{}).Validate(); err != nil {
				t.Skipf("host cannot install %s: %v", test.desktop, err)
			}
			writeCommandTestFile(t, filepath.Join(home, ".config", "pde", "config.json"), `{"profile":"`+test.profile+`"}`)
			profileStages, profilePackages := readDryRun(runInstallDryRun(t, test.profile), home)

			output := runInstallDryRun(t, test.desktop)

			stages, packages := readDryRun(output, home)
			wantStages := slices.Clone(profileStages)
			migrate := slices.Index(wantStages, "migrate legacy PDE configuration")
			wantStages = slices.Insert(wantStages, migrate, "download and verify", "build and atomically activate "+test.player)
			if !slices.Equal(stages, wantStages) {
				t.Errorf("stages =\n%s\nwant\n%s", strings.Join(stages, "\n"), strings.Join(wantStages, "\n"))
			}
			added := slices.DeleteFunc(slices.Clone(packages), func(name string) bool {
				return slices.Contains(profilePackages, name)
			})
			if !slices.Equal(added, test.addedPackages) || len(packages)-len(added) != len(profilePackages) {
				t.Errorf("apt packages = %q, want the profile's %q plus %q", packages, profilePackages, test.addedPackages)
			}
			if data := readChezmoiData(output); data != test.chezmoiData {
				t.Errorf("chezmoi data = %s, want %s", data, test.chezmoiData)
			}
			var sudo []string
			for _, line := range strings.Split(output, "\n") {
				if command, found := strings.CutPrefix(line, "  sudo "); found {
					sudo = append(sudo, strings.Join(strings.Fields(command)[:2], " "))
				}
			}
			if want := []string{"apt-get update", "apt-get install"}; !slices.Equal(sudo, want) {
				t.Errorf("sudo commands = %q, want %q", sudo, want)
			}
		})
	}
}

func runInstallDryRun(t *testing.T, argument string) string {
	t.Helper()
	var output bytes.Buffer
	command := NewCommand()
	command.SetOut(&output)
	command.SetArgs([]string{"install", argument, "--dry-run", "--repo-root", testRepositoryRoot(t)})
	if err := command.Execute(); err != nil {
		t.Fatalf("install %s --dry-run: %v\n%s", argument, err, output.String())
	}
	return output.String()
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
