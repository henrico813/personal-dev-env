package internal

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

func writeNote(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func issue(status, date string) string {
	return "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: " + status +
		"\nproject: X\ndate_created: " + date + "\ntopics: []\n---\n\n# Title\n"
}

func projectsFixture(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-001 Planning.md"), issue("planning", "2026-01-02"))
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-002 Active.md"), issue("in-progress", "2026-01-03"))
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-003 Completed.md"), issue("completed", "2026-01-04"))
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-004 Refused.md"), issue("wontdo", "2026-01-05"))
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-005 Unknown.md"), issue("mystery", "2026-01-06"))
	writeNote(t, filepath.Join(root, "DevEnv", "DESIGN-Thing.md"), "---\ntags: []\n---\n# Design\n")
	writeNote(t, filepath.Join(root, "DevEnv", "Notes.md"), "# No frontmatter\n")
	writeNote(t, filepath.Join(root, "Homelab", "HOME-001 Done.md"), issue("done", "2026-02-01"))
	writeNote(t, filepath.Join(root, "Empty", "Notes.md"), "# Just notes\n")
	writeNote(t, filepath.Join(root, ".obsidian", "hidden.md"), issue("open", "2026-01-01"))
	writeNote(t, filepath.Join(root, "loose.md"), issue("open", "2026-01-01"))
	return root
}

func executeList(args ...string) (int, string, string) {
	var stdout, stderr bytes.Buffer
	exit := Execute(append([]string{"list"}, args...), &stdout, &stderr)
	return exit, stdout.String(), stderr.String()
}

func runListJSON(t *testing.T, args ...string) string {
	t.Helper()
	args = append(args, "--json")
	exit, stdout, stderr := executeList(args...)
	if exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr)
	}
	return stdout
}

// The vault list should resemble ls over projects, not count design docs,
// plain notes, Obsidian data, or loose root files. Fixed status columns replaced
// the unreadable per-spelling counts; unknown appears only when used.
func TestListJSONCountsIncludedIssues(t *testing.T) {
	root := projectsFixture(t)
	var view projectsView
	if err := json.Unmarshal([]byte(runListJSON(t, "--dir", root)), &view); err != nil {
		t.Fatalf("overview JSON: %v", err)
	}
	if view.View != "projects" || len(view.Projects) != 2 {
		t.Fatalf("projects view = %+v", view)
	}
	var devenv, homelab projectSummary
	for _, project := range view.Projects {
		switch project.ProjectDir {
		case "DevEnv":
			devenv = project
		case "Homelab":
			homelab = project
		}
	}
	wantStatuses := []planStatus{statusOpen, statusInProgress, statusDone, statusWontDo, statusUnknown}
	statuses := make([]planStatus, 0, len(devenv.StatusCounts))
	counts := make([]int, 0, len(devenv.StatusCounts))
	for _, count := range devenv.StatusCounts {
		statuses = append(statuses, count.Status)
		counts = append(counts, count.Count)
	}
	if devenv.Total != 5 || !slices.Equal(statuses, wantStatuses) || !slices.Equal(counts, []int{1, 1, 1, 1, 1}) {
		t.Fatalf("DevEnv counts = %+v", devenv)
	}
	if homelab.Total != 1 || len(homelab.StatusCounts) != 4 ||
		homelab.StatusCounts[2] != (statusCount{Status: statusDone, Count: 1}) {
		t.Fatalf("Homelab counts = %+v", homelab)
	}
}

// Vault notes use aliases such as completed, closed, wontdo, and superseded.
// Agents filter by normalized state, while the source spelling stays in
// frontmatter so normalization does not lose note data.
func TestListJSONNormalizesIssueAliases(t *testing.T) {
	root := projectsFixture(t)
	var view plansView
	if err := json.Unmarshal([]byte(runListJSON(t, "devenv", "--dir", root)), &view); err != nil {
		t.Fatalf("project JSON: %v", err)
	}
	want := map[string]struct {
		status planStatus
		raw    string
	}{
		"TASK-001 Planning.md":  {statusOpen, "planning"},
		"TASK-002 Active.md":    {statusInProgress, "in-progress"},
		"TASK-003 Completed.md": {statusDone, "completed"},
		"TASK-004 Refused.md":   {statusWontDo, "wontdo"},
		"TASK-005 Unknown.md":   {statusUnknown, "mystery"},
	}
	if view.View != "project-plans" || view.ProjectDir != "DevEnv" || len(view.Plans) != len(want) {
		t.Fatalf("project view = %+v", view)
	}
	for _, plan := range view.Plans {
		name := filepath.Base(plan.Path)
		expected, ok := want[name]
		if !ok || plan.Status != expected.status || plan.Frontmatter["status"] != expected.raw {
			t.Fatalf("plan %s = status %q frontmatter status %#v", name, plan.Status, plan.Frontmatter["status"])
		}
	}
}

// Review found that --status nonsense returned unknown plans with exit 0,
// which looked like a real answer. Known aliases must still filter across
// projects.
func TestListStatusAliasAndInvalidInput(t *testing.T) {
	root := projectsFixture(t)
	var view plansView
	if err := json.Unmarshal([]byte(runListJSON(t, "--status", "completed", "--dir", root)), &view); err != nil {
		t.Fatalf("status JSON: %v", err)
	}
	if view.View != "plans" || view.ProjectDir != "" || len(view.Plans) != 2 {
		t.Fatalf("cross-project status view = %+v", view)
	}
	projects := []string{view.Plans[0].Project, view.Plans[1].Project}
	if !slices.Contains(projects, "DevEnv") || !slices.Contains(projects, "Homelab") {
		t.Fatalf("status results are not cross-project: %v", projects)
	}
	for _, plan := range view.Plans {
		if plan.Status != statusDone {
			t.Fatalf("status = %q, want done", plan.Status)
		}
	}

	exit, _, stderr := executeList("--status", "nonsense", "--dir", root, "--json")
	if exit != 2 || !strings.Contains(stderr, "accepted values:") || !strings.Contains(stderr, "wont-do") {
		t.Fatalf("invalid status exit=%d stderr=%q", exit, stderr)
	}
}

// JSON is the core API and human output renders it; this is the single parity
// check. JSON loses object-key order, and review found no-match JSON exited 0
// while the human view exited 2.
func TestListDetailPreservesOrderAndParity(t *testing.T) {
	root := projectsFixture(t)
	var view plansView
	if err := json.Unmarshal([]byte(runListJSON(t, "DevEnv", "task-003", "--dir", root)), &view); err != nil {
		t.Fatalf("detail JSON: %v", err)
	}
	if view.View != "plan-details" || len(view.Plans) != 1 {
		t.Fatalf("detail view = %+v", view)
	}
	fields := make([]string, 0, len(view.Plans[0].FrontmatterOrder))
	for _, field := range view.Plans[0].FrontmatterOrder {
		fields = append(fields, field.Key)
	}
	wantFields := []string{"tags", "type", "status", "project", "date_created", "topics"}
	if !slices.Equal(fields, wantFields) {
		t.Fatalf("frontmatter order = %v, want %v", fields, wantFields)
	}
	var rendered bytes.Buffer
	renderPlanDetails(&rendered, view)
	if exit, got, stderr := executeList("DevEnv", "task-003", "--dir", root); exit != 0 || got != rendered.String() {
		t.Fatalf("human detail exit=%d stderr=%q\n got: %q\nwant: %q", exit, stderr, got, rendered.String())
	}
	for _, args := range [][]string{
		{"DevEnv", "zzz", "--dir", root},
		{"DevEnv", "zzz", "--dir", root, "--json"},
	} {
		exit, _, stderr := executeList(args...)
		if exit != 2 || !strings.Contains(stderr, "no plan in DevEnv matches") {
			t.Fatalf("no-match args %v exit=%d stderr=%q", args, exit, stderr)
		}
	}
}
