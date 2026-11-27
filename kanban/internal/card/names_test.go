package card

import (
	"maps"
	"path/filepath"
	"testing"

	"github.com/BurntSushi/toml"
)

// The repository's names.toml and the cases every reader of it runs.
var (
	namesFile = filepath.Join("..", "..", "..", "chezmoi", "dot_config", "pde", "names.toml")
	casesFile = filepath.Join("..", "..", "..", "testdata", "names", "cases.toml")
)

// Card must read each shared case as every other reader of names.toml does,
// or it reads a branch or agent name differently from the rule that made it.
// A case with no fields is a name the rule must not read.
func TestParseNameReadsSharedCases(t *testing.T) {
	rules, err := ReadNameRules(namesFile)
	if err != nil {
		t.Fatal(err)
	}
	var cases struct {
		Parse []struct {
			Rule   string
			Name   string
			Fields map[string]string
		}
	}
	if _, err := toml.DecodeFile(casesFile, &cases); err != nil {
		t.Fatal(err)
	}

	for _, tt := range cases.Parse {
		t.Run(tt.Rule+" "+tt.Name, func(t *testing.T) {
			rule, err := rules.FindNameRule(tt.Rule)
			if err != nil {
				t.Fatal(err)
			}

			fields, ok := ParseName(rule, tt.Name)

			if wantOK := tt.Fields != nil; ok != wantOK {
				t.Fatalf("ParseName read it = %v, want %v", ok, wantOK)
			}
			if ok && !maps.Equal(fields, tt.Fields) {
				t.Errorf("fields = %v, want %v", fields, tt.Fields)
			}
		})
	}
}
