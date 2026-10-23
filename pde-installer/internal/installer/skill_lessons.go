package installer

import (
	"fmt"
	"os"
	"path/filepath"

	"pde-installer/internal/run"
)

// Installing skills overwrites each installed SKILL.md, which empties any
// `## Lessons` section. When skill-lessons is on PATH or in ~/.local/bin,
// `skill-lessons render` writes saved lessons back. A failed render only
// warns, because the lessons are still in skill-lessons' store.
//
// The render runs after chezmoi's drift check, which would treat the rendered
// SKILL.md files as drift and roll the install back.
func renderSkillLessons(home string, runner run.Runner) error {
	command := run.Command{Name: "skill-lessons", Args: []string{"render"}}
	if runner.DryRun {
		return runner.Plan("render skill lessons when skill-lessons is installed", &command)
	}
	// skill-lessons installs to ~/.local/bin, which may not be on PATH yet.
	path := filepath.Join(home, ".local", "bin") + string(os.PathListSeparator) + os.Getenv("PATH")
	binary, err := run.LookPath(command.Name, path)
	if err != nil {
		return nil
	}
	command.Name = binary
	if err := runner.Run("render skill lessons", command); err != nil {
		fmt.Fprintf(runner.Err(), "warning: %v; run skill-lessons render to restore lessons\n", err)
	}
	return nil
}
