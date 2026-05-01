package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"time"
)

// backupIfExists moves any existing managed config aside before the installer replaces it.
func backupIfExists(path string, runner Runner) error {
	if _, err := os.Lstat(path); err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	stamp := time.Now().Format("20060102_150405")
	backup := fmt.Sprintf("%s.backup.%s", path, stamp)
	if err := runner.MkdirAll("create backup parent", filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return runner.Rename("backup existing config", path, backup)
}

// syncTree copies a repo-managed tree into its runtime destination.
func syncTree(src, dst string, runner Runner) error {
	return runner.Do("sync "+src+" to "+dst, func() error {
		return copyTree(src, dst)
	})
}

func copyTree(src, dst string) error {
	srcInfo, err := os.Stat(src)
	if err != nil {
		return err
	}
	if !srcInfo.IsDir() {
		return fmt.Errorf("%s is not a directory", src)
	}

	if err := os.RemoveAll(dst); err != nil {
		return err
	}
	if err := os.MkdirAll(dst, srcInfo.Mode().Perm()); err != nil {
		return err
	}

	return filepath.WalkDir(src, func(path string, d fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path == src {
			return nil
		}

		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		dstPath := filepath.Join(dst, rel)

		info, err := d.Info()
		if err != nil {
			return err
		}

		if d.IsDir() {
			return os.MkdirAll(dstPath, info.Mode().Perm())
		}
		if info.Mode()&os.ModeSymlink != 0 {
			target, err := os.Readlink(path)
			if err != nil {
				return err
			}
			return os.Symlink(target, dstPath)
		}

		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(dstPath), 0o755); err != nil {
			return err
		}
		return os.WriteFile(dstPath, data, info.Mode().Perm())
	})
}

func copyFile(src, dst string, runner Runner) error {
	return runner.Do("copy "+src+" to "+dst, func() error {
		info, err := os.Stat(src)
		if err != nil {
			return err
		}
		if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
			return err
		}
		data, err := os.ReadFile(src)
		if err != nil {
			return err
		}
		return os.WriteFile(dst, data, info.Mode().Perm())
	})
}

// linkBinary keeps command entrypoints stable while the runtime is managed elsewhere.
func linkBinary(src, dst string, runner Runner) error {
	return runner.Do("link "+filepath.Base(dst), func() error {
		if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
			return err
		}
		if err := os.RemoveAll(dst); err != nil {
			return err
		}
		return os.Symlink(src, dst)
	})
}
