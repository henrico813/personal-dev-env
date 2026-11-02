package app

import (
	"os"
	"path/filepath"
	"testing"
)

// Writes the first line of each file less shows to the log file in its own
// folder, then quits with the next key from the keys file there, as the
// lesskey file's quit actions do.
const recordingLess = `#!/bin/sh
dir=${0%/*}
head -n 1 >> "$dir/log"
key=$(head -c 1 "$dir/keys")
tail -c +2 "$dir/keys" > "$dir/keys.rest" && mv "$dir/keys.rest" "$dir/keys"
exit $(printf '%d' "'$key")
`

// Writes the file the file picker opens on to the log file in its own
// folder, then ends as the next line of the picks file there says. A line
// such as "l 1" prints the key and file 1 as fzf does, and "l -" prints the
// key alone and exits 1, as fzf does when nothing matches the search.
const choosingFzf = `#!/bin/sh
dir=${0%/*}
pos=$(printf '%s\n' "$@" | sed -n 's/^--bind=load:pos(\([0-9]*\))$/\1/p')
echo "picker on $pos" >> "$dir/log"
pick=$(head -n 1 "$dir/picks")
tail -n +2 "$dir/picks" > "$dir/picks.rest" && mv "$dir/picks.rest" "$dir/picks"
key=${pick% *}
row=${pick#* }
if [ "$row" = - ]; then
  printf '%s\n' "$key"
  exit 1
fi
printf '%s\n%s\t+1 -0\ta.go\n' "$key" "$row"
`

// The diff screen must keep the current file across less, the file picker
// and every key, with L and H stopping at the ends. Otherwise a key shows
// the wrong diff or closes the diff screen.
func TestOpenDiffTracksFileAcrossKeys(t *testing.T) {
	bin := writeScripts(
		t,
		map[string]string{
			"less": recordingLess,
			"fzf":  choosingFzf,
		},
	)
	// The keys file holds the diff view's keys in turn, and the picks file
	// the file picker's picks.
	writeFile(
		t,
		filepath.Join(
			bin,
			"keys",
		),
		"LLLHoooHh",
		0o600,
	)
	writeFile(
		t,
		filepath.Join(
			bin,
			"picks",
		),
		"o 1\nl -\nl 1\n",
		0o600,
	)
	t.Setenv(
		"PATH",
		bin+":"+os.Getenv("PATH"),
	)

	runPRs(
		t,
		"works",
		"_diff",
		"example/repo",
		"12",
	)

	shown, _ := os.ReadFile(filepath.Join(
		bin,
		"log",
	))
	want := "diff --git a/a.go b/a.go\n" + // The first file.
		"diff --git a/b.go b/b.go\n" + // L
		"diff --git a/c.go b/c.go\n" + // L
		"diff --git a/c.go b/c.go\n" + // L stays on the last file.
		"diff --git a/b.go b/b.go\n" + // H
		"picker on 2\n" + // o opens the picker on b.go.
		"diff --git a/b.go b/b.go\n" + // The picker's o, with a.go selected.
		"picker on 2\n" + // o
		"diff --git a/b.go b/b.go\n" + // The picker's l with nothing matching.
		"picker on 2\n" + // o
		"diff --git a/a.go b/a.go\n" + // The picker's l on a.go.
		"diff --git a/a.go b/a.go\n" // H stays on the first file, then h.
	if string(shown) != want {
		t.Errorf(
			"screens shown were\n%s\nwant\n%s",
			shown,
			want,
		)
	}
}
