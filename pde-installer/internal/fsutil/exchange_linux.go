//go:build linux

package fsutil

import (
	"errors"
	"fmt"

	"golang.org/x/sys/unix"
)

func exchangePaths(left, right string) error {
	err := unix.Renameat2(unix.AT_FDCWD, left, unix.AT_FDCWD, right, unix.RENAME_EXCHANGE)
	if errors.Is(err, unix.ENOSYS) || errors.Is(err, unix.EINVAL) || errors.Is(err, unix.EOPNOTSUPP) {
		return fmt.Errorf("renameat2 RENAME_EXCHANGE is unsupported by this kernel or filesystem: %w", err)
	}
	if errors.Is(err, unix.EXDEV) {
		return fmt.Errorf("staging and destination must use the same filesystem: %w", err)
	}
	return err
}
