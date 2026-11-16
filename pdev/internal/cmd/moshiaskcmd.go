package cmd

import (
	"bytes"
	"errors"
	"os"
	"os/exec"
	"strings"
	"time"

	"pdev/internal/approval"
)

// moshi-hook cannot take back a question it has sent, so one sent for a
// request later answered in the Herdr popup stays on the phone until it shows
// "timed out" four hours later. Most popup answers come within seconds, so a
// request from Herdr asks the phone only once it is phoneDelay old. Tests
// replace waitBeforePhone so they do not wait.
const phoneDelay = 30 * time.Second

var waitBeforePhone = time.Sleep

// Runs as pde-pr-approve --moshi-ask ID, which pde-gh starts, and waits up
// to four hours for the phone answer, after phoneDelay for a request from
// Herdr. It saves the answer only while the request is pending and never
// runs gh, which the runner, the waiting pde-gh-write or a rerun does.
func moshiAsk(args []string) int {
	if len(args) != 1 {
		return 1
	}
	answer, failure := approval.ChoiceNone, ""
	rec, err := approval.Load(args[0])
	// Outside Herdr no popup shows, and a rerun of an older request needs no
	// wait, so both ask at once. An answer during the wait stops this program,
	// and the reload checks again before anything is sent.
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
	// The system reuses PIDs, so a stale one could look like a running question
	// and be signaled by approval.StopGroup. The PID is removed whenever this
	// ends while the request is live, with the failure for pde-gh-write to report.
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
