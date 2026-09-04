//go:build linux

package fsutil

import (
	"os"
	"syscall"
)

func identifyPath(path string) (pathIdentity, error) {
	info, err := os.Lstat(path)
	if err != nil {
		return pathIdentity{}, err
	}
	stat := info.Sys().(*syscall.Stat_t)
	return pathIdentity{Device: uint64(stat.Dev), Inode: stat.Ino}, nil
}
