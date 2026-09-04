package main

import (
	"fmt"
	"os"
	"time"
)

func backupConfigInstallPath(path string) (string, error) {
	if _, err := os.Lstat(path); err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}

	for {
		stamp := time.Now().UTC().Format("20060102_150405.000000000")
		backup := fmt.Sprintf("%s.bak.%s.%d", path, stamp, os.Getpid())
		if _, err := os.Lstat(backup); err == nil {
			continue
		} else if !os.IsNotExist(err) {
			return "", err
		}
		if err := os.Rename(path, backup); err != nil {
			return "", err
		}
		return backup, nil
	}
}

func restoreConfigInstallPath(path, backup string) error {
	if err := os.RemoveAll(path); err != nil {
		return err
	}
	if backup == "" {
		return nil
	}
	return os.Rename(backup, path)
}
