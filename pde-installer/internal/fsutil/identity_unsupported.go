//go:build !linux

package fsutil

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

func identifyPath(path string) (pathIdentity, error) {
	hash := sha256.New()
	err := filepath.Walk(path, func(current string, info os.FileInfo, walkErr error) (err error) {
		if walkErr != nil {
			return walkErr
		}
		relative, err := filepath.Rel(path, current)
		if err != nil {
			return err
		}
		_, _ = fmt.Fprintf(hash, "%s\x00%s\x00", relative, info.Mode())
		if info.Mode()&os.ModeSymlink != 0 {
			target, err := os.Readlink(current)
			if err != nil {
				return err
			}
			_, _ = io.WriteString(hash, target)
			return nil
		}
		if !info.Mode().IsRegular() {
			return nil
		}
		file, err := os.Open(current)
		if err != nil {
			return err
		}
		defer func() { err = errors.Join(err, file.Close()) }()
		_, err = io.Copy(hash, file)
		return err
	})
	if err != nil {
		return pathIdentity{}, err
	}
	return pathIdentity{Digest: hex.EncodeToString(hash.Sum(nil))}, nil
}
