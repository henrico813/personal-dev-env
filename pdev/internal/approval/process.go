package approval

import (
	"os"
	"os/exec"
	"path/filepath"
	"syscall"
)

// Starts pde-gh as the runner of one request, as in pde-gh --pde-runner ID.
const RunnerFlag = "--pde-runner"

// Reports whether a process with this PID exists.
func ProcessAlive(pid int) bool { return syscall.Kill(pid, 0) == nil }

// Ends a program that StartDetached started, with its children. It runs in
// its own session, so its PID is also its process group ID.
func StopGroup(pid int) {
	if pid > 0 && ProcessAlive(pid) {
		_ = syscall.Kill(-pid, syscall.SIGTERM)
	}
}

// Starts program in its own session without waiting for it. The tool that
// ran an agent's command stops its process group when it ends, and the
// runner and phone question must outlive that, since answers come hours later.
func StartDetached(program string, args ...string) (int, error) {
	cmd := exec.Command(program, args...)
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return 0, err
	}
	return cmd.Process.Pid, nil
}

// Returns the path of name in this program's folder, where the installer
// puts pde-gh beside pde-pr-approve, whatever PATH either was given.
func FindSibling(name string) (string, error) {
	self, err := os.Executable()
	if err != nil {
		return "", err
	}
	return filepath.Join(filepath.Dir(self), name), nil
}

// Starts program as the runner of rec and saves its PID. Callers hold the
// request lock, and the returned record carries the PID for later saves.
func StartRunner(rec Record, program string) Record {
	pid, err := StartDetached(program, RunnerFlag, rec.ID)
	if err != nil {
		return rec
	}
	rec.RunnerPID = pid
	// Like the phone question's PID, this is not a change of the request, so
	// it must not restart expiry.
	if err := SaveKeepingAge(rec); err != nil {
		_ = syscall.Kill(-pid, syscall.SIGKILL)
		rec.RunnerPID = 0
	}
	return rec
}

// Calls start when rec has no live runner, such as one killed or lost on
// reboot, so a late approval still runs. Callers hold the request lock.
func EnsureRunner(rec Record, start func(Record) Record) Record {
	if rec.RunnerPID == 0 || !ProcessAlive(rec.RunnerPID) {
		return start(rec)
	}
	return rec
}

// Reports whether path is a file, not a folder, with any execute bit set.
func IsExecutable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir() && info.Mode()&0o111 != 0
}
