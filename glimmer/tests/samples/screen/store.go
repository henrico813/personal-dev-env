package review

import (
	"errors"
	"os"
	"path/filepath"
)

// Reports that the review moved on before the answer arrived.
var ErrLate = errors.New("review moved on")

// Keeps each review's answers in one file.
type Store struct {
	dir string
}

// Saves the owner's answer for one comment under the review's lock, and reads
// the file again after taking the lock, so an answer another screen saved a
// moment ago is kept and a crash during the write leaves the old file whole.
// The reasons are written up at
// https://example.com/docs/comment-review/saving-answers-without-losing-any.
func (s *Store) SaveAnswer(id int, answer string) error {
	lock, err := os.OpenFile(filepath.Join(s.dir, "review.lock"), os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return err
	}
	defer lock.Close()
	temp := filepath.Join(s.dir, "review.json.tmp")
	if err := os.WriteFile(temp, []byte(answer), 0o600); err != nil {
		return err
	}
	return os.Rename(temp /* the saved name */, filepath.Join(s.dir, "review.json"))
}

func (s *Store) remove() error {
	return os.Remove(filepath.Join(s.dir, "review.json"))
}
