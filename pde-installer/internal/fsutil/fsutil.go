package fsutil

import (
	"archive/zip"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const maxDownloadSize = 1024 * 1024 * 1024

// Download fetches a size-limited file and verifies its SHA-256 checksum.
func Download(url, destination, checksum string) (err error) {
	client := &http.Client{Timeout: 20 * time.Minute}
	response, err := client.Get(url)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, response.Body.Close()) }()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return fmt.Errorf("download %s: HTTP %s", url, response.Status)
	}
	if response.ContentLength > maxDownloadSize {
		return fmt.Errorf("download %s exceeds size limit", url)
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		return err
	}
	temporary, err := os.CreateTemp(filepath.Dir(destination), ".download-")
	if err != nil {
		return err
	}
	name := temporary.Name()
	defer func() { _ = os.Remove(name) }()
	hash := sha256.New()
	written, copyErr := io.Copy(io.MultiWriter(temporary, hash), io.LimitReader(response.Body, maxDownloadSize+1))
	syncErr := temporary.Sync()
	closeErr := temporary.Close()
	if copyErr != nil {
		return copyErr
	}
	if closeErr != nil {
		return closeErr
	}
	if syncErr != nil {
		return syncErr
	}
	if written > maxDownloadSize {
		return fmt.Errorf("download %s exceeds size limit", url)
	}
	actual := hex.EncodeToString(hash.Sum(nil))
	if !strings.EqualFold(actual, checksum) {
		return fmt.Errorf("checksum mismatch for %s: got %s, want %s", url, actual, checksum)
	}
	if err := os.Rename(name, destination); err != nil {
		return err
	}
	return syncDirectory(filepath.Dir(destination))
}

// GuardHome rejects mutation paths outside home or below symlinks.
func GuardHome(home string, paths ...string) error {
	return guardHome(home, rejectLeafSymlink, paths...)
}

// GuardHomeAllowLeafSymlink permits only the final path element to be a symlink.
func GuardHomeAllowLeafSymlink(home string, paths ...string) error {
	return guardHome(home, allowLeafSymlink, paths...)
}

type leafSymlinkPolicy uint8

const (
	rejectLeafSymlink leafSymlinkPolicy = iota
	allowLeafSymlink
)

func guardHome(home string, leafPolicy leafSymlinkPolicy, paths ...string) error {
	resolvedHome, err := filepath.EvalSymlinks(home)
	if err != nil {
		return fmt.Errorf("resolve HOME: %w", err)
	}
	for _, path := range paths {
		if path == "" {
			continue
		}
		clean := filepath.Clean(path)
		relative, err := filepath.Rel(home, clean)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			return fmt.Errorf("mutation path outside HOME: %s", clean)
		}
		current := home
		for _, component := range strings.Split(relative, string(filepath.Separator)) {
			if component == "." || component == "" {
				continue
			}
			current = filepath.Join(current, component)
			info, err := os.Lstat(current)
			if os.IsNotExist(err) {
				break
			}
			if err != nil {
				return fmt.Errorf("inspect mutation path %s: %w", current, err)
			}
			if info.Mode()&os.ModeSymlink != 0 && (leafPolicy == rejectLeafSymlink || current != clean) {
				return fmt.Errorf("mutation path has symlink ancestor that may resolve outside HOME: %s", current)
			}
		}
		ancestor := clean
		for {
			if info, err := os.Lstat(ancestor); err == nil {
				if leafPolicy == allowLeafSymlink && ancestor == clean && info.Mode()&os.ModeSymlink != 0 {
					ancestor = filepath.Dir(ancestor)
					continue
				}
				break
			} else if !os.IsNotExist(err) {
				return fmt.Errorf("inspect mutation ancestor %s: %w", ancestor, err)
			}
			ancestor = filepath.Dir(ancestor)
		}
		resolved, err := filepath.EvalSymlinks(ancestor)
		if err != nil {
			return fmt.Errorf("resolve mutation ancestor %s: %w", ancestor, err)
		}
		relative, err = filepath.Rel(resolvedHome, resolved)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			return fmt.Errorf("mutation path resolves outside HOME: %s -> %s", clean, resolved)
		}
	}
	return nil
}

// FileSHA256 returns the lowercase SHA-256 digest of a file.
func FileSHA256(path string) (digest string, err error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer func() { err = errors.Join(err, file.Close()) }()
	hash := sha256.New()
	if _, err := io.Copy(hash, file); err != nil {
		return "", err
	}
	return hex.EncodeToString(hash.Sum(nil)), nil
}

// ExtractZip extracts regular files from an archive without path traversal.
func ExtractZip(archive, destination string) (err error) {
	reader, err := zip.OpenReader(archive)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, reader.Close()) }()
	for _, entry := range reader.File {
		clean := filepath.Clean(entry.Name)
		if filepath.IsAbs(clean) || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) {
			return fmt.Errorf("unsafe archive path %q", entry.Name)
		}
		target := filepath.Join(destination, clean)
		if entry.FileInfo().IsDir() {
			if err := os.MkdirAll(target, 0o755); err != nil {
				return err
			}
			continue
		}
		if !entry.Mode().IsRegular() {
			return fmt.Errorf("unsupported archive entry %q", entry.Name)
		}
		input, err := entry.Open()
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			_ = input.Close()
			return err
		}
		output, err := os.OpenFile(target, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, entry.Mode().Perm())
		if err != nil {
			_ = input.Close()
			return err
		}
		_, copyErr := io.Copy(output, input)
		inputErr, outputErr := input.Close(), output.Close()
		if copyErr != nil {
			return copyErr
		}
		if inputErr != nil {
			return inputErr
		}
		if outputErr != nil {
			return outputErr
		}
	}
	return nil
}

// CopyTree copies a directory tree while preserving file permissions and links.
func CopyTree(source, destination string) error {
	return filepath.WalkDir(source, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		relative, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		target := filepath.Join(destination, relative)
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if entry.IsDir() {
			return os.MkdirAll(target, info.Mode().Perm())
		}
		if entry.Type()&os.ModeSymlink != 0 {
			link, err := os.Readlink(path)
			if err != nil {
				return err
			}
			if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
				return err
			}
			return os.Symlink(link, target)
		}
		if !entry.Type().IsRegular() {
			return fmt.Errorf("unsupported file in %s: %s", source, path)
		}
		input, err := os.Open(path)
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			_ = input.Close()
			return err
		}
		output, err := os.OpenFile(target, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, info.Mode().Perm())
		if err != nil {
			_ = input.Close()
			return err
		}
		_, copyErr := io.Copy(output, input)
		inputErr, closeErr := input.Close(), output.Close()
		if copyErr != nil {
			return copyErr
		}
		if inputErr != nil {
			return inputErr
		}
		return closeErr
	})
}

// BackupName returns a collision-resistant sibling backup path.
func BackupName(path string) string {
	return fmt.Sprintf("%s.pde-backup-%s-%d", path, time.Now().UTC().Format("20060102T150405.000000000Z"), os.Getpid())
}

// Activate atomically replaces a destination with a staged path.
func Activate(stage, destination string) (string, error) {
	if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		return "", err
	}
	if _, err := os.Lstat(destination); err == nil {
		if err := exchangePaths(stage, destination); err != nil {
			return "", fmt.Errorf("atomically activate %s: %w", destination, err)
		}
		return stage, nil
	} else if !os.IsNotExist(err) {
		return "", err
	}
	if err := os.Rename(stage, destination); err != nil {
		return "", fmt.Errorf("activate new path %s with same-filesystem rename: %w", destination, err)
	}
	return "", nil
}

// Rollback restores a destination from an activation backup.
func Rollback(destination, backup string) error {
	if backup != "" {
		if _, err := os.Lstat(destination); err == nil {
			if err := exchangePaths(backup, destination); err != nil {
				return fmt.Errorf("atomically restore %s: %w", destination, err)
			}
			return os.RemoveAll(backup)
		} else if !os.IsNotExist(err) {
			return err
		}
		return os.Rename(backup, destination)
	}
	return os.RemoveAll(destination)
}

// CopyPath copies one regular file, directory, or symbolic link.
func CopyPath(source, destination string) error {
	info, err := os.Lstat(source)
	if err != nil {
		return err
	}
	if info.Mode()&os.ModeSymlink != 0 {
		target, err := os.Readlink(source)
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
			return err
		}
		return os.Symlink(target, destination)
	}
	if info.IsDir() {
		return CopyTree(source, destination)
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("unsupported path type: %s", source)
	}
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		_ = input.Close()
		return err
	}
	output, err := os.OpenFile(destination, os.O_CREATE|os.O_EXCL|os.O_WRONLY, info.Mode().Perm())
	if err != nil {
		_ = input.Close()
		return err
	}
	_, copyErr := io.Copy(output, input)
	inputErr, outputErr := input.Close(), output.Close()
	if copyErr != nil {
		return copyErr
	}
	if inputErr != nil {
		return inputErr
	}
	return outputErr
}
