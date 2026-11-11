//go:build darwin

package fsutil

import (
	"errors"
	"fmt"

	"golang.org/x/sys/unix"
)

// Swaps the two paths in one step with renamex_np RENAME_SWAP, which APFS
// supports.
func exchangePaths(left, right string) error {
	err := unix.RenamexNp(left, right, unix.RENAME_SWAP)
	if errors.Is(err, unix.ENOTSUP) || errors.Is(err, unix.EINVAL) {
		return fmt.Errorf("renamex_np RENAME_SWAP is unsupported by this filesystem: %w", err)
	}
	if errors.Is(err, unix.EXDEV) {
		return fmt.Errorf("staging and destination must use the same filesystem: %w", err)
	}
	return err
}
