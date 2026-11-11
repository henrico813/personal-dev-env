//go:build !linux && !darwin

package fsutil

import "fmt"

func exchangePaths(_, _ string) error {
	return fmt.Errorf("atomic replacement of an existing path requires Linux renameat2 or macOS renamex_np")
}
