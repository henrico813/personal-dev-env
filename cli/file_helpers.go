package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sync/atomic"
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

var configInstallBackupSeq uint64

func backupConfigInstallPath(path string, runner Runner) error {
	if _, err := os.Lstat(path); err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	if err := runner.MkdirAll("create config backup parent", filepath.Dir(path), 0o755); err != nil {
		return err
	}

	for {
		stamp := time.Now().UTC().Format("20060102_150405.000000000")
		backup := fmt.Sprintf("%s.bak.%s.%d.%d", path, stamp, os.Getpid(), atomic.AddUint64(&configInstallBackupSeq, 1))
		if _, err := os.Lstat(backup); err == nil {
			continue
		} else if !os.IsNotExist(err) {
			return err
		}
		return runner.Rename("backup existing config", path, backup)
	}
}

// syncTree copies a repo-managed tree into a fresh destination subtree.
func syncTree(src, dst string, runner Runner) error {
	return runner.Do("sync "+src+" to "+dst, func() error {
		return copyTree(src, dst)
	})
}

// syncTreeInto copies a repo-managed tree into an existing root without removing sibling files.
func syncTreeInto(src, dst string, runner Runner) error {
	return runner.Do("sync "+src+" into "+dst, func() error {
		return copyTreeInto(src, dst)
	})
}

func copyTree(src, dst string) error {
	if err := os.RemoveAll(dst); err != nil {
		return err
	}
	return copyTreeInto(src, dst)
}

func copyTreeInto(src, dst string) error {
	srcInfo, err := os.Stat(src)
	if err != nil {
		return err
	}
	if !srcInfo.IsDir() {
		return fmt.Errorf("%s is not a directory", src)
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
			if err := os.RemoveAll(dstPath); err != nil {
				return err
			}
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

func linkConfig(src, dst string, runner Runner) error {
	if target, ok, err := existingSymlinkTarget(dst); err != nil {
		return err
	} else if ok && target == src {
		return nil
	}

	return runner.Do("link "+dst, func() error {
		if err := validateReadableRegularFile(src); err != nil {
			return fmt.Errorf("validate config source %s: %w", src, err)
		}

		if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
			return err
		}

		if info, err := os.Lstat(dst); err == nil {
			if info.Mode()&os.ModeSymlink != 0 {
				if err := os.Remove(dst); err != nil {
					return err
				}
			} else {
				if err := backupConfigInstallPath(dst, runner); err != nil {
					return err
				}
			}
		} else if !os.IsNotExist(err) {
			return err
		}

		return os.Symlink(src, dst)
	})
}

func validateReadableRegularFile(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("%s is not a regular file", path)
	}

	f, err := os.Open(path)
	if err != nil {
		return err
	}
	return f.Close()
}

func existingSymlinkTarget(path string) (string, bool, error) {
	info, err := os.Lstat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return "", false, nil
		}
		return "", false, err
	}
	if info.Mode()&os.ModeSymlink == 0 {
		return "", false, nil
	}
	target, err := os.Readlink(path)
	if err != nil {
		return "", false, err
	}
	return target, true, nil
}
