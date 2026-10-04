package chezmoi

import (
	"io/fs"
	"path"
	"path/filepath"
	"strings"
	"testing"
)

// Chezmoi refuses the whole install when a path is both managed and listed for
// removal. This happened when the Codex skill copies were removed.
func TestRemoveTargetsAvoidManagedSources(t *testing.T) {
	removeText := renderProfileTemplate(t, removeFileTemplate, "full")
	var removeTargets []string
	for _, line := range strings.Split(removeText, "\n") {
		if target := strings.TrimSpace(line); target != "" {
			removeTargets = append(removeTargets, path.Clean(target))
		}
	}

	produced := managedSourceTargets(t)
	produced = append(produced, renderedExternalTargets(renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full"))...)
	for _, removed := range removeTargets {
		for _, target := range produced {
			if pathsOverlap(removed, target) {
				t.Errorf("removal target %q overlaps produced target %q", removed, target)
			}
		}
	}
}

func renderedExternalTargets(text string) []string {
	var targets []string
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "[\"") && strings.HasSuffix(line, "\"]") {
			targets = append(targets, path.Clean(strings.TrimSuffix(strings.TrimPrefix(line, `["`), `"]`)))
		}
	}
	return targets
}

func managedSourceTargets(t *testing.T) []string {
	t.Helper()
	root := filepath.Join(repoRoot(t), "chezmoi")
	var targets []string
	err := filepath.WalkDir(root, func(name string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			return nil
		}
		if strings.HasPrefix(entry.Name(), "run_") || strings.HasPrefix(entry.Name(), ".chezmoi") {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() {
			return nil
		}
		relative, err := filepath.Rel(root, name)
		if err != nil {
			return err
		}
		parts := strings.Split(filepath.ToSlash(relative), "/")
		for index, part := range parts {
			parts[index] = translateSourceName(part)
		}
		targets = append(targets, path.Join(parts...))
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return targets
}

func translateSourceName(name string) string {
	if strings.HasPrefix(name, "dot_") {
		name = "." + strings.TrimPrefix(name, "dot_")
	} else {
		for _, prefix := range []string{"symlink_", "private_", "executable_"} {
			if strings.HasPrefix(name, prefix) {
				name = strings.TrimPrefix(name, prefix)
				break
			}
		}
	}
	return strings.TrimSuffix(name, ".tmpl")
}

func pathsOverlap(first, second string) bool {
	return first == second || strings.HasPrefix(first, second+"/") || strings.HasPrefix(second, first+"/")
}
