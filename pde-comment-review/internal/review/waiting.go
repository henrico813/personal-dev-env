package review

import (
	"errors"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"time"
)

type appliedFile struct {
	Batch int `json:"round"`
}

// Records that the latest round's answers were written into the code, so
// the review stops waiting until an AI adds another round.
func (r *Review) SetApplied() error {
	applied := appliedFile{Batch: len(r.Batches)}
	return replaceJSONFile(filepath.Join(r.Dir, "applied.json"), applied)
}

// Returns every review whose latest round is not applied yet and whose
// checkout still exists, the one with the newest round first.
func ListWaitingReviews() ([]*Review, error) {
	folders, err := os.ReadDir(FindStateRoot())
	if errors.Is(err, fs.ErrNotExist) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	var waiting []*Review
	added := map[string]time.Time{}
	for _, folder := range folders {
		if !folder.IsDir() || !namePattern.MatchString(folder.Name()) {
			continue
		}
		r, err := loadReview(folder.Name())
		if err != nil {
			return nil, err
		}
		if len(r.Batches) == 0 || r.AppliedBatch == len(r.Batches) {
			continue
		}
		// A removed worktree's review can never be applied.
		if _, err := os.Stat(r.Repo()); errors.Is(err, fs.ErrNotExist) {
			continue
		}
		info, err := os.Stat(buildBatchFilePath(r.Dir, len(r.Batches)))
		if err != nil {
			return nil, err
		}
		added[r.Name] = info.ModTime()
		waiting = append(waiting, r)
	}
	sort.Slice(waiting, func(i, j int) bool {
		return added[waiting[i].Name].After(added[waiting[j].Name])
	})
	return waiting, nil
}
