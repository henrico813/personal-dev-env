package workflow

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"path/filepath"
	"reflect"
)

type sessionReceipt struct {
	SchemaVersion string            `json:"schema_version"`
	Status        string            `json:"status"`
	RepoRoot      string            `json:"repo_root"`
	TaskNames     []string          `json:"task_names"`
	Artifacts     []ReceiptArtifact `json:"artifacts"`
}

func validateReceipt(state State, path string) (ResearchState, error) {
	if state.Research == nil || state.Research.ManagedRoot == "" {
		return ResearchState{}, fmt.Errorf("bind a Surveil managed root first")
	}
	expected := filepath.Join(state.Research.ManagedRoot, ".surveil-session", "receipt.json")
	path, e := filepath.Abs(path)
	if e != nil || filepath.Clean(path) != expected {
		return ResearchState{}, fmt.Errorf("receipt path must be %s", expected)
	}
	raw, id, e := readRegular(path)
	if e != nil {
		return ResearchState{}, e
	}
	var r sessionReceipt
	if e = decodeStrict(raw, &r); e != nil {
		return ResearchState{}, fmt.Errorf("decode Surveil receipt: %w", e)
	}
	if r.SchemaVersion != "surveil.session.v1" || r.Status != "complete" {
		return ResearchState{}, fmt.Errorf("unsupported Surveil receipt")
	}
	if r.RepoRoot != state.Repository {
		return ResearchState{}, fmt.Errorf("receipt repository does not match workflow")
	}
	tasks := requiredTasks()
	if !reflect.DeepEqual(r.TaskNames, tasks) {
		return ResearchState{}, fmt.Errorf("receipt tasks must be %v", tasks)
	}
	want := make([]ReceiptArtifact, 0, len(tasks)*3+1)
	for _, t := range tasks {
		for _, k := range []string{"context", "trace", "report"} {
			x := t
			want = append(want, ReceiptArtifact{Kind: k, TaskName: &x, Path: filepath.ToSlash(filepath.Join("tasks", t, k+".json"))})
		}
	}
	want = append(want, ReceiptArtifact{Kind: "evidence", Path: "evidence.json"})
	if len(r.Artifacts) != len(want) {
		return ResearchState{}, fmt.Errorf("receipt has %d artifacts, want %d", len(r.Artifacts), len(want))
	}
	root := filepath.Dir(path)
	for i, a := range r.Artifacts {
		w := want[i]
		if a.Kind != w.Kind || !reflect.DeepEqual(a.TaskName, w.TaskName) || a.Path != w.Path {
			return ResearchState{}, fmt.Errorf("unexpected receipt artifact %d", i)
		}
		if filepath.IsAbs(a.Path) || filepath.Clean(a.Path) != a.Path {
			return ResearchState{}, fmt.Errorf("invalid receipt artifact path %q", a.Path)
		}
		b, _, e := readRegular(filepath.Join(root, filepath.FromSlash(a.Path)))
		if e != nil {
			return ResearchState{}, e
		}
		sum := sha256.Sum256(b)
		d, de := hex.DecodeString(a.SHA256)
		if de != nil || len(d) != sha256.Size || a.SHA256 != hex.EncodeToString(d) || a.ByteLen != uint64(len(b)) || a.SHA256 != hex.EncodeToString(sum[:]) {
			return ResearchState{}, fmt.Errorf("receipt artifact %q failed integrity validation", a.Path)
		}
	}
	return ResearchState{ManagedRoot: state.Research.ManagedRoot, Receipt: &id, Artifacts: r.Artifacts}, nil
}
