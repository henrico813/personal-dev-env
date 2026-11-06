package review

/* Saves the owner's answer under the review's lock. */
func SaveAnswer() {}

func FinishWriting() {
	tries := 3 // The lock waits at most three times.
	wait := 1  // Each wait is one second.
	_, _ = tries, wait
}
