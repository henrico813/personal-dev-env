package installer

import (
	"fmt"
	"os"
	"strings"

	"pde-installer/internal/aqua"
	"pde-installer/internal/builds"
	chezmoibackend "pde-installer/internal/chezmoi"
	"pde-installer/internal/direct"
	"pde-installer/internal/manifest"
	"pde-installer/internal/npm"
	"pde-installer/internal/profile"
	"pde-installer/internal/run"
	"pde-installer/internal/tmux"
	"pde-installer/internal/ubuntu"
)

func list(config config, runner run.Runner) error {
	if err := config.validateProfile(); err != nil {
		return err
	}
	ubuntuManager := ubuntu.New(config.Profile, runner)
	tmuxManager := tmux.New(config.Home, runner)
	aquaManager := aqua.New(config.Home, config.RepoRoot, config.Profile, runner)
	aquaInstalled, aquaState, err := aquaManager.Probe()
	if err != nil {
		return fmt.Errorf("read Aqua status: %w", err)
	}
	var npmManager npm.Manager
	var directManager direct.Manager
	var directTools []direct.Tool
	var buildManager builds.Manager
	if config.Profile == profile.Full {
		npmManager = npm.New(config.Home, config.RepoRoot, runner)
		directManager = direct.New(config.Home, runner)
		var directToolsErr error
		directTools, directToolsErr = direct.Tools()
		if directToolsErr != nil {
			return fmt.Errorf("read direct tool metadata: %w", directToolsErr)
		}
		buildManager = builds.New(config.Home, config.RepoRoot, runner)
	}
	chezmoiState, err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, config.Profile, runner).Probe()
	if err != nil {
		return fmt.Errorf("read chezmoi status: %w", err)
	}
	if _, err := fmt.Fprintln(runner.Out(), "OWNER\tITEM\tREQUESTED\tINSTALLED\tSTATUS"); err != nil {
		return fmt.Errorf("write list heading: %w", err)
	}
	for _, item := range manifest.ItemsFor(config.Profile) {
		requested, installed, state := item.Version, "", "missing"
		switch item.Owner {
		case manifest.Ubuntu:
			installed, state, err = ubuntuManager.Probe(item.Name)
			if err != nil {
				return fmt.Errorf("read %s status: %w", item.Name, err)
			}
		case manifest.Aqua:
			if item.Name == "aqua" {
				installed, state = aquaInstalled, aquaState
			} else {
				installed, state, err = aquaManager.ToolProbe(item.Name, requested)
				if err != nil {
					return fmt.Errorf("read %s status: %w", item.Name, err)
				}
			}
		case manifest.NPM:
			installed, err = npmManager.Version(item.Name)
			if err != nil && !os.IsNotExist(err) {
				return fmt.Errorf("read %s status: %w", item.Name, err)
			}
			if installed == requested {
				state = "current"
			} else if installed != "" {
				state = "outdated"
			}
		case manifest.Local:
			if item.Name == "blink.cmp" {
				state, err = buildManager.BlinkStatus()
				if err != nil {
					return fmt.Errorf("read blink.cmp status: %w", err)
				}
			} else {
				state, err = buildManager.Probe(item.Name)
				if err != nil {
					return fmt.Errorf("read %s status: %w", item.Name, err)
				}
			}
		case manifest.Direct:
			if item.Name == "tmux" {
				installed, state, err = tmuxManager.Probe()
				if err != nil {
					return fmt.Errorf("read tmux status: %w", err)
				}
				break
			}
			isTool := false
			for _, tool := range directTools {
				if tool.Name == item.Name {
					isTool = true
					installed, state, err = directManager.ToolProbe(item.Name)
					if err != nil {
						return fmt.Errorf("read %s status: %w", item.Name, err)
					}
					break
				}
			}
			if isTool {
				break
			}
			for _, font := range direct.Fonts() {
				if font.Name == item.Name {
					state, err = directManager.Probe(font)
					if err != nil {
						return fmt.Errorf("read %s status: %w", item.Name, err)
					}
				}
			}
		case manifest.Chezmoi:
			state = chezmoiState
		}
		if _, err := fmt.Fprintf(runner.Out(), "%s\t%s\t%s\t%s\t%s\n", item.Owner, item.Name, requested, strings.TrimSpace(installed), state); err != nil {
			return fmt.Errorf("write list item %s: %w", item.Name, err)
		}
	}
	return nil
}
