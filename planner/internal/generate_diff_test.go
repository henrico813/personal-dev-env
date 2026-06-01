package internal

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestGenerateUnifiedDiffOutput(t *testing.T) {
	diff, err := generateUnifiedDiff(
		"planner/internal/example.go",
		"func old() string {\n\treturn \"old\"\n}\n",
		"func old() string {\n\treturn \"new\"\n}\n",
	)
	require.NoError(t, err)

	assert.Contains(t, diff, "diff --git a/planner/internal/example.go b/planner/internal/example.go")
	assert.Contains(t, diff, "--- a/planner/internal/example.go")
	assert.Contains(t, diff, "+++ b/planner/internal/example.go")
	assert.Contains(t, diff, "@@")
	assert.Contains(t, diff, "-\treturn \"old\"")
	assert.Contains(t, diff, "+\treturn \"new\"")
}

func TestGenerateUnifiedDiffErrors(t *testing.T) {
	tests := []struct {
		name     string
		filename string
		oldText  string
		newText  string
		wantErr  string
	}{
		{
			name:     "invalid filename",
			filename: "bad file name.go",
			oldText:  "old\n",
			newText:  "new\n",
			wantErr:  "invalid file change filename",
		},
		{
			name:     "no op change",
			filename: "planner/internal/example.go",
			oldText:  "same\n",
			newText:  "same\n",
			wantErr:  "unchanged",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			_, err := generateUnifiedDiff(tc.filename, tc.oldText, tc.newText)
			require.Error(t, err)
			assert.Contains(t, err.Error(), tc.wantErr)
		})
	}
}

func TestGenerateUnifiedDiffLineEndings(t *testing.T) {
	tests := []struct {
		name      string
		oldText   string
		newText   string
		wantParts []string
	}{
		{
			name:    "crlf input",
			oldText: "line1\r\nline2\r\n",
			newText: "line1\r\nline3\r\n",
			wantParts: []string{
				"-line2",
				"+line3",
			},
		},
		{
			name:    "lf input",
			oldText: "line1\nline2\n",
			newText: "line1\nline3\n",
			wantParts: []string{
				"-line2",
				"+line3",
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			diff, err := generateUnifiedDiff(
				"planner/internal/example.go",
				tc.oldText,
				tc.newText,
			)
			require.NoError(t, err)

			for _, want := range tc.wantParts {
				assert.Contains(t, diff, want)
			}
		})
	}
}
