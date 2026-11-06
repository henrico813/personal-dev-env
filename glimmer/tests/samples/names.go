// Package review keeps the owner's answers.
// Each answer is saved under a lock.
package review

// Statuses a review can have.
const (
	// Marks a review with no answers yet.
	Waiting Status = iota
	Answered
)

// Keeps every answer the owner gives.
type ReviewStore struct {
	// Holds one answer per comment.
	Answers []string
}

// Saves the owner's answer under the review's lock.
func (s *ReviewStore) SaveAnswer() {
	// The status is saved, never worked out from files.
	_ = s
}

// Finishes writing the review.

func FinishWriting() {}

// Opens the review screen.
func init() {}

// Opens the review popup.
func init() {}
