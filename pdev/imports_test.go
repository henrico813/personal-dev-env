package main

import (
	"go/parser"
	"go/token"
	"io/fs"
	"path/filepath"
	"slices"
	"strconv"
	"strings"
	"testing"
)

// One binary's own package, with the only pdev packages its non-test files
// may import and the only package that may import it.
type toolFamily struct {
	allowed  []string
	importer string
}

// Each tool family is its own binary and shares code only through the
// packages in allowed. Add a row for each family split out of pdev.
var toolFamilies = map[string]toolFamily{
	"pdev/internal/ghtool": {
		allowed:  []string{"pdev/internal/approval"},
		importer: "pdev/cmd/pde-gh",
	},
}

// The gh tool decides which gh commands an agent may run, so a change to
// pde-pr-approve's code must not change it by accident, and pde-pr-approve
// must not reach into it. A pdev import across that line would join the two
// again without anyone deciding to.
func TestToolFamiliesImportOnlySharedPackages(t *testing.T) {
	imports := listModuleImports(t)

	for pkg, family := range toolFamilies {
		for _, imported := range imports[pkg] {
			if strings.HasPrefix(imported, "pdev/") && !slices.Contains(family.allowed, imported) {
				t.Errorf("%s imports %s; it may import only %v", pkg, imported, family.allowed)
			}
		}
		for importer, list := range imports {
			if importer != family.importer && importer != pkg && slices.Contains(list, pkg) {
				t.Errorf("%s imports %s; only %s may", importer, pkg, family.importer)
			}
		}
	}
}

// Maps each package in this module to the imports of its non-test files.
func listModuleImports(t *testing.T) map[string][]string {
	t.Helper()
	imports := map[string][]string{}
	err := filepath.WalkDir(".", func(path string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() || !strings.HasSuffix(path, ".go") || strings.HasSuffix(path, "_test.go") {
			return err
		}
		file, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
		if err != nil {
			return err
		}
		pkg := filepath.ToSlash(filepath.Join("pdev", filepath.Dir(path)))
		for _, spec := range file.Imports {
			imported, _ := strconv.Unquote(spec.Path.Value)
			imports[pkg] = append(imports[pkg], imported)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return imports
}
