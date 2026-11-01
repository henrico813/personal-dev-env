package diffsplit

import (
	"fmt"
	"strings"
	"testing"
)

// A diff with a deleted, new, renamed, binary, mode change and modified
// file. The last hunk has lines that look like file headers, which count
// as changes.
const sixKindsDiff = `diff --git a/old.sh b/old.sh
deleted file mode 100755
index 1111111..0000000
--- a/old.sh
+++ /dev/null
@@ -1,2 +0,0 @@
-#!/bin/sh
-echo old
diff --git a/README.md b/README.md
new file mode 100644
index 0000000..2222222
--- /dev/null
+++ b/README.md
@@ -0,0 +1 @@
+# Title
diff --git a/old/same.py b/new/same.py
similarity index 100%
rename from old/same.py
rename to new/same.py
diff --git a/old/test.py b/new/test.py
similarity index 80%
rename from old/test.py
rename to new/test.py
index 3333333..4444444 100644
--- a/old/test.py
+++ b/new/test.py
@@ -1,2 +1,2 @@
-import a
+import b
 x = 1
diff --git a/shot.png b/shot.png
new file mode 100644
index 0000000..5555555
Binary files /dev/null and b/shot.png differ
diff --git a/run.sh b/run.sh
old mode 100644
new mode 100755
diff --git a/edit.go b/edit.go
index 6666666..7777777 100644
--- a/edit.go
+++ b/edit.go
@@ -1,2 +1,2 @@
 package edit
--- a removed line
+++ an added line
`

// Each changed file must get the right counts, tags and path, which the
// file picker shows. A wrong count, tag or path shows you the wrong change.
func TestSplitCountsTagsAndPaths(t *testing.T) {
	var rows strings.Builder
	for _, file := range split([]byte(sixKindsDiff)) {
		fmt.Fprintf(
			&rows,
			"%d +%d -%d %s %s\n",
			file.Number,
			file.Added,
			file.Removed,
			strings.Join(
				file.Tags,
				",",
			),
			file.Path,
		)
	}

	want := "1 +0 -2 deleted old.sh\n" +
		"2 +1 -0 new README.md\n" +
		"3 +0 -0 renamed new/same.py\n" +
		"4 +1 -1 renamed new/test.py\n" +
		"5 +0 -0 new,binary shot.png\n" +
		"6 +0 -0 mode run.sh\n" +
		"7 +1 -1  edit.go\n"
	if rows.String() != want {
		t.Errorf(
			"rows are\n%s\nwant\n%s",
			rows.String(),
			want,
		)
	}
}

// Each split file must hold one file's lines, starting at its own header,
// and together they must hold the whole diff in order. A line put in the
// wrong file shows a change under the wrong file name.
func TestSplitKeepsLinesUnderOwnHeader(t *testing.T) {
	files := split([]byte(sixKindsDiff))

	var joined strings.Builder
	for _, file := range files {
		text := string(file.Text)
		if !strings.HasPrefix(
			text,
			"diff --git ",
		) || strings.Count(
			text,
			"diff --git ",
		) != 1 {
			t.Errorf(
				"file %d is %q, want one header first",
				file.Number,
				text,
			)
		}
		joined.WriteString(text)
	}
	if joined.String() != sixKindsDiff {
		t.Errorf(
			"split files joined are\n%s\nwant\n%s",
			joined.String(),
			sixKindsDiff,
		)
	}
}
