package card

import (
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"time"
)

// Reads a card note again, such as after an Obsidian save. It fails when
// the note's type is no longer card.
func Load(path, cardKey string) (Card, error) {
	// The time comes before the text, so a save during the read fails
	// the time check in writeSafely rather than being lost.
	info, err := os.Stat(path)
	if err != nil {
		return Card{}, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return Card{}, err
	}
	text := normalizeNewlines(string(data))
	if ReadFields(text)["type"].Text != "card" {
		return Card{}, fmt.Errorf("%s is not a card note", filepath.Base(path))
	}
	return Card{Path: path, CardKey: cardKey, Text: text, ModTime: info.ModTime()}, nil
}

// Applies change to the card's text and writes it when the text changed,
// reporting whether it wrote. After an Obsidian save since the read, it
// reads the note and applies change once more before it gives up.
func Update(c Card, change func(text string) (string, error)) (bool, error) {
	for attempt := range 2 {
		if attempt == 1 {
			var err error
			if c, err = Load(c.Path, c.CardKey); err != nil {
				return false, err
			}
		}
		text, err := change(c.Text)
		if err != nil {
			return false, err
		}
		if text == c.Text {
			return false, nil
		}
		written, err := writeSafely(c.Path, c.CardKey, text, c.ModTime)
		if err != nil || written {
			return written, err
		}
	}
	return false, fmt.Errorf("%s changed twice while writing; try again", c.CardKey)
}

// Writes ".<card key>.tmp" in the note's folder, syncs it, and renames it
// over the note. It renames only when the note's time is still readTime,
// and a zero readTime means a new note.
func writeSafely(path, cardKey, text string, readTime time.Time) (bool, error) {
	temp := filepath.Join(filepath.Dir(path), "."+cardKey+".tmp")
	if err := writeAndSync(temp, text); err != nil {
		return false, err
	}
	if !readTime.IsZero() {
		info, err := os.Stat(path)
		if err != nil {
			return false, err
		}
		// A save after this check and before the rename is still lost.
		// The Python card has the same gap.
		if !info.ModTime().Equal(readTime) {
			return false, os.Remove(temp)
		}
	}
	if err := os.Rename(temp, path); err != nil {
		return false, err
	}
	return true, nil
}

// Writes text to a new or emptied file and syncs it to disk.
func writeAndSync(path, text string) error {
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o666)
	if err != nil {
		return err
	}
	if _, err := file.WriteString(text); err != nil {
		_ = file.Close()
		return err
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		return err
	}
	return file.Close()
}

// Takes an exclusive flock on path, on the local disk, and returns the
// function that releases it. It makes the file and its folder if missing.
func HoldLock(path string) (release func(), err error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o777); err != nil {
		return nil, err
	}
	file, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o666)
	if err != nil {
		return nil, err
	}
	if err := syscall.Flock(int(file.Fd()), syscall.LOCK_EX); err != nil {
		_ = file.Close()
		return nil, err
	}
	// Closing the file releases the lock.
	return func() { _ = file.Close() }, nil
}
