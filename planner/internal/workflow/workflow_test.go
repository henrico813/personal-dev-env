package workflow

import (
	"os"
	"path/filepath"
	"testing"
)

func TestStartNormalizesPartialUpdatePaths(t *testing.T) {
	root := t.TempDir()
	repo := filepath.Join(root, "repo")
	input := filepath.Join(root, "plan.md")
	if err := os.Mkdir(repo, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(input, []byte("plan\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	relRepo, err := filepath.Rel(cwd, repo)
	if err != nil {
		t.Fatal(err)
	}
	relInput, err := filepath.Rel(cwd, input)
	if err != nil {
		t.Fatal(err)
	}
	id := "0123456789abcdef0123456789abcdef"
	result, err := Start(StartOptions{
		WorkflowID: id, Repository: relRepo, Input: relInput, Output: relInput,
		Operation: OperationPartialUpdate, StateRoot: filepath.Join(root, "state"),
	})
	if err != nil {
		t.Fatal(err)
	}
	store, err := Open(filepath.Join(root, "state"), result.WorkflowID, nil)
	if err != nil {
		t.Fatal(err)
	}
	state, err := store.Load()
	if err != nil {
		t.Fatal(err)
	}
	if state.Repository != repo || state.Input == nil || state.Input.Path != input || state.InitialOutput.Path != input {
		t.Fatalf("state = %#v", state)
	}
}

func TestReviewFailureIsTerminal(t *testing.T) {
	root := t.TempDir()
	repo := filepath.Join(root, "repo")
	if err := os.Mkdir(repo, 0o700); err != nil {
		t.Fatal(err)
	}
	output := filepath.Join(root, "plan.md")
	id := "11111111111111111111111111111111"
	if _, err := Start(StartOptions{WorkflowID: id, Repository: repo, Output: output, Operation: OperationNew, StateRoot: filepath.Join(root, "state")}); err != nil {
		t.Fatal(err)
	}
	store, err := Open(filepath.Join(root, "state"), id, nil)
	if err != nil {
		t.Fatal(err)
	}
	state, err := Fail(store, 1, "review", "blocking findings remain")
	if err != nil {
		t.Fatal(err)
	}
	if state.Stage != StageFailed {
		t.Fatalf("stage = %s", state.Stage)
	}
	if _, err := Fail(store, state.Revision, "review", "another failure"); err == nil {
		t.Fatal("terminal workflow accepted another failure")
	}
}
