//go:build !linux

package fsutil

import "fmt"

func exchangePaths(_, _ string) error {
	return fmt.Errorf("atomic replacement of an existing path requires Linux renameat2 RENAME_EXCHANGE")
}
