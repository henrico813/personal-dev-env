package cmd

import (
	"bytes"
	"errors"
	"os"
	"os/exec"
	"strings"
	"syscall"
	"time"

	"pdev2/internal/approval"
)

// The phone question is sent by this same program, started again with
// --moshi-ask. Tests replace moshiProgram so they never message a phone.
var moshiProgram = os.Executable

// moshi-hook cannot take back a question it has sent, so one sent for a
// request later answered in the Herdr popup stays on the phone until it shows
// "timed out" four hours later. Most popup answers come within seconds, so a
// request from Herdr asks the phone only once it is phoneDelay old. Tests
// replace waitBeforePhone so they do not wait.
const phoneDelay = 30 * time.Second

var waitBeforePhone = time.Sleep

// startMoshi sends the phone question unless the one saved in the record is
// still running, so reruns of a waiting request never send a second question.
// The returned record carries the saved PID, so later saves under the same
// lock keep it.
func startMoshi(rec approval.Record) approval.Record {
	if rec.MoshiPID != 0 && processAlive(rec.MoshiPID) {
		return rec
	}
	path, err := moshiProgram()
	if err != nil {
		return rec
	}
	cmd := exec.Command(path, "--moshi-ask", rec.ID)
	// When the agent's command ends, the tool that ran it stops every program
	// the command started (its process group). You may answer hours later, so
	// the question is started apart from that group, where the stop does not
	// reach it, and is not waited for.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return rec
	}
	rec.MoshiPID = cmd.Process.Pid
	// The new question replaces a failed one, so its reason no longer applies.
	rec.MoshiError = ""
	// Reruns check this PID before sending a question. If it cannot be saved,
	// stop the question, or a rerun would send a second one.
	if err := approval.SaveKeepingAge(rec); err != nil {
		_ = cmd.Process.Kill()
		rec.MoshiPID = 0
	}
	return rec
}

// Ends a phone question that can no longer decide anything. One still in its
// wait never reaches the phone; one moshi-hook already sent stays there. The
// question runs in its own session, so its PID is also its process group ID.
// moshiAsk must not call this for its own answer.
func stopMoshi(pid int) {
	if pid != 0 && processAlive(pid) {
		_ = syscall.Kill(-pid, syscall.SIGTERM)
	}
}

// moshiAsk runs as pde-gh-write --moshi-ask ID. For a request from Herdr it
// first waits until the request is phoneDelay old. It then waits up to four
// hours for the phone answer and saves it only if the request is still
// pending; a later answer changes nothing. It never runs gh: the request's
// runner, the waiting pde-gh-write, or a rerun does.
func moshiAsk(args []string) int {
	if len(args) != 1 {
		return 1
	}
	answer, failure := approval.ChoiceNone, ""
	rec, err := approval.Load(args[0])
	// Outside Herdr no popup shows, so the phone is asked at once. An answer
	// during the wait ends this program through stopMoshi; the reload checks
	// again before anything is sent. A rerun of an older request asks at once.
	if err == nil && rec.State == approval.StatePending && os.Getenv("HERDR_ENV") == "1" {
		// The cap keeps a clock set back after the request from delaying the phone.
		waitBeforePhone(min(time.Until(rec.Created.Add(phoneDelay)), phoneDelay))
		rec, err = approval.Load(args[0])
	}
	// The question comes from the saved record, so it shows what the popup shows.
	if err == nil && rec.State == approval.StatePending {
		ask := exec.Command("moshi-hook", "ask", "--require-remote", "--source", "pde-gh-write", "--timeout", "4h", moshiQuestion(rec))
		// This program runs detached and nobody reads its output, so moshi-hook's
		// reason for failing is kept for the record.
		var hookErr bytes.Buffer
		ask.Stderr = &hookErr
		commandErr := ask.Run()
		// moshi-hook ask exits 0 for yes and 1 for no; any other result, such as
		// exit 4 when Moshi cannot be reached or a killed helper, is no answer.
		var exitErr *exec.ExitError
		if commandErr == nil {
			answer = approval.ChoiceYes
		} else if errors.As(commandErr, &exitErr) && exitErr.ExitCode() == 1 {
			answer = approval.ChoiceNo
		} else {
			failure = moshiFailure(commandErr, hookErr.String())
		}
	}
	lock, err := approval.Lock(args[0])
	if err != nil {
		return 1
	}
	defer lock.Close()
	rec, err = approval.Load(args[0])
	if err != nil || approval.Expired(rec) {
		return 0
	}
	if answer != approval.ChoiceNone && rec.State == approval.StatePending {
		if _, err := recordAnswer(rec, answer); err != nil {
			return 1
		}
		return 0
	}
	// Remove this program's PID whenever it ends while the request is live. The
	// system reuses PIDs, so a stale one could name an unrelated program that
	// looks like a running question, stop reruns from sending a new one, and
	// be signaled by stopMoshi. The failure is saved with it, so the waiting
	// pde-gh-write can report it.
	if rec.MoshiPID != os.Getpid() {
		return 0
	}
	rec.MoshiPID = 0
	rec.MoshiError = failure
	if err := approval.SaveKeepingAge(rec); err != nil {
		return 1
	}
	return 0
}

// moshiFailure joins the exit status with the last line moshi-hook printed,
// which names the cause, as in "moshi-hook: can't ask on your phone: ...".
func moshiFailure(err error, output string) string {
	lines := strings.Split(strings.TrimSpace(output), "\n")
	if last := lines[len(lines)-1]; last != "" {
		return err.Error() + ": " + last
	}
	return err.Error()
}

// Uses the popup's context lines, so the phone and the popup name a request the same way.
func moshiQuestion(rec approval.Record) string {
	lines := approval.ContextLines(rec)
	lines[0] = "Approve " + lines[0] + "?"
	return strings.Join(lines, "\n")
}
