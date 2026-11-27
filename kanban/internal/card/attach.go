package card

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

// Copies source into the attachments folder beside the card as
// "<card key> <file name>", embeds the copy in Context, and returns the
// Context line. When the card write fails, it removes the copy, so trying
// again is not refused.
func Attach(c Card, source string) (string, error) {
	base := filepath.Base(source)
	name := strings.TrimSpace(badNameChars.ReplaceAllString(base, ""))
	if name == "" {
		return "", fmt.Errorf("nothing is left of the name %s once the characters Obsidian refuses are removed", quotePythonRepr(base))
	}
	attachment := filepath.Join(filepath.Dir(c.Path), "attachments", c.CardKey+" "+name)
	entry := "- ![[" + filepath.Base(attachment) + "]]"
	if err := copyToNewFile(source, attachment); err != nil {
		return "", err
	}
	_, err := Update(c, func(text string) (string, error) {
		return AddToSection(text, "Context", entry), nil
	})
	if err != nil {
		_ = os.Remove(attachment)
		return "", err
	}
	return entry, nil
}

// Copies source to target, and fails when target already exists. The
// exclusive create checks for the name and makes the file in one step,
// and a copy cut short is removed.
func copyToNewFile(source, target string) (err error) {
	reader, err := os.Open(source)
	if err != nil {
		return err
	}
	defer func() { _ = reader.Close() }()
	if err := os.Mkdir(filepath.Dir(target), 0o777); err != nil && !errors.Is(err, os.ErrExist) {
		return err
	}
	writer, err := os.OpenFile(target, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o666)
	if errors.Is(err, os.ErrExist) {
		return fmt.Errorf("%s is already in %s, rename the file and try again",
			filepath.Base(target), filepath.Base(filepath.Dir(target)))
	}
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			_ = os.Remove(target)
		}
	}()
	if _, err := io.Copy(writer, reader); err != nil {
		_ = writer.Close()
		return err
	}
	if err := writer.Sync(); err != nil {
		_ = writer.Close()
		return err
	}
	return writer.Close()
}
