package main

import (
	"go/ast"
	"go/parser"
	"go/token"
	"io/fs"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// Fails when internal/card imports kanban/internal/cmd or os/exec. Card
// rules then stay testable without nobs, Herdr or gh.
func TestCardPackageRunsNoPrograms(t *testing.T) {
	imports := listModuleImports(t)

	for _, imported := range imports["kanban/internal/card"] {
		if imported == "os/exec" || strings.HasPrefix(imported, "kanban/internal/cmd") {
			t.Errorf("kanban/internal/card imports %s; only internal/cmd may run programs", imported)
		}
	}
}

// Fails when a non-test file in internal/card reads the environment or the
// clock. Commands read both and pass them in, so tests can set them.
func TestCardPackageIgnoresEnvAndClock(t *testing.T) {
	banned := map[string]bool{
		"os.Getenv":    true,
		"os.LookupEnv": true,
		"os.Environ":   true,
		"time.Now":     true,
		"time.Since":   true,
	}
	paths, err := filepath.Glob("internal/card/*.go")
	if err != nil {
		t.Fatal(err)
	}

	for _, path := range paths {
		if strings.HasSuffix(path, "_test.go") {
			continue
		}
		fileSet := token.NewFileSet()
		file, err := parser.ParseFile(fileSet, path, nil, 0)
		if err != nil {
			t.Fatal(err)
		}
		ast.Inspect(file, func(node ast.Node) bool {
			selector, ok := node.(*ast.SelectorExpr)
			if !ok {
				return true
			}
			pkg, ok := selector.X.(*ast.Ident)
			if ok && banned[pkg.Name+"."+selector.Sel.Name] {
				t.Errorf(
					"%s uses %s.%s; only internal/cmd reads the environment or clock",
					fileSet.Position(selector.Pos()),
					pkg.Name,
					selector.Sel.Name,
				)
			}
			return true
		})
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
		pkg := filepath.ToSlash(filepath.Join("kanban", filepath.Dir(path)))
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
