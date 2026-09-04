package installer

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"golang.org/x/sys/unix"
	"pde-installer/internal/fsutil"
)

type installerLock struct {
	file *os.File
}

func acquireInstallerLock(home string) (*installerLock, error) {
	path := filepath.Join(home, ".local", "state", "pde", "installer.lock")
	if err := fsutil.GuardHome(home, path); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return nil, fmt.Errorf("create installer state: %w", err)
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, fmt.Errorf("open installer lock: %w", err)
	}
	if err := unix.Flock(int(file.Fd()), unix.LOCK_EX|unix.LOCK_NB); err != nil {
		_ = file.Close()
		if errors.Is(err, unix.EWOULDBLOCK) {
			return nil, fmt.Errorf("another installer is already running")
		}
		return nil, fmt.Errorf("lock installer state: %w", err)
	}
	return &installerLock{file: file}, nil
}

func (l *installerLock) Close() error {
	return errors.Join(
		unix.Flock(int(l.file.Fd()), unix.LOCK_UN),
		l.file.Close(),
	)
}
