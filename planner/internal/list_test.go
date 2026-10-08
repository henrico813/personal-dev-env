package internal

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
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

// projectsFixture mimics the vault Projects folder: project folders holding
// plans mixed with design docs, plus Obsidian config and a loose root note.
func projectsFixture(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-001 First.md"), issue("done", "2026-01-02"))
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-002 Second.md"), issue("in-progress", "2026-03-04"))
	writeNote(t, filepath.Join(root, "DevEnv", "DESIGN-Thing.md"), "---\ntags: []\n---\n# Design\n")
	writeNote(t, filepath.Join(root, "Homelab", "HOME-001 Box.md"), issue("open", "2026-02-01"))
	writeNote(t, filepath.Join(root, "Empty", "Notes.md"), "# Just notes\n")
	writeNote(t, filepath.Join(root, ".obsidian", "x", "hidden.md"), issue("open", "2026-01-01"))
	writeNote(t, filepath.Join(root, "loose.md"), issue("open", "2026-01-01"))
	return root
}

func runListCmd(t *testing.T, args ...string) string {
	t.Helper()
	var stdout, stderr bytes.Buffer
	if exit := Execute(append([]string{"list"}, args...), &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	return stdout.String()
}

func TestListShowsProjectsWithPlanCounts(t *testing.T) {
	root := projectsFixture(t)

	out := runListCmd(t, "--dir", root)

	lines := strings.Split(strings.TrimSpace(out), "\n")
	if len(lines) != 3 {
		t.Fatalf("want header + DevEnv + Homelab only:\n%s", out)
	}
	if fields := strings.Fields(lines[1]); strings.Join(fields, " ") != "DevEnv 2 0 1 1 0" {
		t.Fatalf("DevEnv row = %q", lines[1])
	}
	if fields := strings.Fields(lines[2]); strings.Join(fields, " ") != "Homelab 1 1 0 0 0" {
		t.Fatalf("Homelab row = %q", lines[2])
	}
}

func TestListProjectShowsPlanFiles(t *testing.T) {
	root := projectsFixture(t)

	out := runListCmd(t, "devenv", "--dir", root)

	if !strings.Contains(out, "TASK-001 First.md") || !strings.Contains(out, "TASK-002 Second.md") {
		t.Fatalf("missing plans:\n%s", out)
	}
	if strings.Contains(out, "DESIGN-Thing.md") || strings.Contains(out, root) {
		t.Fatalf("want only plan files with relative names:\n%s", out)
	}
}

func runListJSON(t *testing.T, args ...string) string {
	t.Helper()
	args = append(args, "--json")
	return runListCmd(t, args...)
}

func TestListJSONNormalizesAliases(t *testing.T) {
	root := projectsFixture(t)
	cases := []struct {
		raw    string
		status PlanStatus
	}{
		{raw: " PLANNING ", status: PlanStatusOpen},
		{raw: "IN-PROGRESS", status: PlanStatusInProgress},
		{raw: "Completed", status: PlanStatusDone},
		{raw: "CLOSED", status: PlanStatusDone},
		{raw: "Won't Do", status: PlanStatusWontDo},
		{raw: "wontdo", status: PlanStatusWontDo},
		{raw: "obsolete", status: PlanStatusWontDo},
		{raw: "superseded", status: PlanStatusWontDo},
		{raw: "failed", status: PlanStatusWontDo},
		{raw: "mystery", status: PlanStatusUnknown},
	}
	for i, tc := range cases {
		content := issue(tc.raw, "2026-04-05")
		if i == 2 {
			content = strings.Replace(content, "project: X", "project: []", 1)
		}
		name := "TASK-Alias-" + string(rune('A'+i)) + ".md"
		writeNote(t, filepath.Join(root, "DevEnv", name), content)
	}

	var view PlansView
	if err := json.Unmarshal([]byte(runListJSON(t, "DevEnv", "--dir", root)), &view); err != nil {
		t.Fatalf("project JSON: %v", err)
	}
	for i, tc := range cases {
		name := "TASK-Alias-" + string(rune('A'+i)) + ".md"
		var found bool
		for _, plan := range view.Plans {
			if filepath.Base(plan.Path) != name {
				continue
			}
			found = true
			wantRaw := strings.TrimSpace(tc.raw)
			if plan.Status != tc.status || plan.RawStatus != wantRaw || plan.Frontmatter["status"] != wantRaw {
				t.Fatalf("%s = status %q, raw %#v, frontmatter %#v", name, plan.Status, plan.RawStatus, plan.Frontmatter["status"])
			}
			if i == 2 {
				if values, ok := plan.Frontmatter["project"].([]any); !ok || len(values) != 0 {
					t.Fatalf("frontmatter project = %#v, want empty array", plan.Frontmatter["project"])
				}
			}
		}
		if !found {
			t.Fatalf("missing %s in JSON", name)
		}
	}

	if got := runListCmd(t, "DevEnv", "--dir", root); !strings.Contains(got, "done") || strings.Contains(got, "COMPLETED") {
		t.Fatalf("project human status was not normalized:\n%s", got)
	}
	if got := runListCmd(t, "DevEnv", "task-alias-c", "--dir", root); !strings.Contains(got, "status") || !strings.Contains(got, "done") || strings.Contains(got, "Completed") {
		t.Fatalf("detail human status was not normalized:\n%s", got)
	}
}

func TestListJSONOrdersOverviewStatuses(t *testing.T) {
	root := projectsFixture(t)
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-003 Legacy.md"), issue("failed", "2026-04-05"))
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-004 Unknown.md"), issue("waiting", "2026-04-06"))

	var view ProjectsView
	if err := json.Unmarshal([]byte(runListJSON(t, "--dir", root)), &view); err != nil {
		t.Fatalf("overview JSON: %v", err)
	}
	var devenv, homelab ProjectSummary
	for _, project := range view.Projects {
		switch project.ProjectDir {
		case "DevEnv":
			devenv = project
		case "Homelab":
			homelab = project
		}
	}
	want := []PlanStatus{PlanStatusOpen, PlanStatusInProgress, PlanStatusDone, PlanStatusWontDo, PlanStatusUnknown}
	got := make([]PlanStatus, 0, len(devenv.StatusCounts))
	for _, count := range devenv.StatusCounts {
		got = append(got, count.Status)
	}
	if !reflect.DeepEqual(got, want) || devenv.StatusCounts[3].Count != 1 || devenv.StatusCounts[4].Count != 1 {
		t.Fatalf("DevEnv counts = %+v", devenv.StatusCounts)
	}
	if len(homelab.StatusCounts) != 4 {
		t.Fatalf("Homelab counts = %+v, want no zero unknown", homelab.StatusCounts)
	}
}

func TestListJSONKeepsStatusResults(t *testing.T) {
	root := projectsFixture(t)

	var view PlansView
	if err := json.Unmarshal([]byte(runListJSON(t, "--status", "in-progress", "--dir", root)), &view); err != nil {
		t.Fatalf("plans JSON: %v", err)
	}
	if view.View != "plans" || view.ProjectDir != "" || len(view.Plans) != 1 ||
		filepath.Base(view.Plans[0].Path) != "TASK-002 Second.md" {
		t.Fatalf("cross-project status view = %+v", view)
	}
}

func TestListStatusAliasFiltersPlans(t *testing.T) {
	root := projectsFixture(t)
	writeNote(t, filepath.Join(root, "DevEnv", "TASK-003 Legacy.md"), issue("Completed", "2026-04-05"))

	var view PlansView
	if err := json.Unmarshal([]byte(runListJSON(t, "--status", " completed ", "--dir", root)), &view); err != nil {
		t.Fatalf("status JSON: %v", err)
	}
	if len(view.Plans) != 2 {
		t.Fatalf("matching plans = %d, want done and completed", len(view.Plans))
	}
	for _, plan := range view.Plans {
		if plan.Status != PlanStatusDone {
			t.Fatalf("status = %q, want done", plan.Status)
		}
	}
}

func TestListJSONDetailPreservesFieldOrder(t *testing.T) {
	root := projectsFixture(t)

	var view PlanDetailsView
	if err := json.Unmarshal([]byte(runListJSON(t, "DevEnv", "task-002", "--dir", root)), &view); err != nil {
		t.Fatalf("detail JSON: %v", err)
	}
	if view.View != "plan-details" || len(view.Plans) != 1 {
		t.Fatalf("detail view = %+v", view)
	}
	want := []string{"tags", "type", "status", "project", "date_created", "topics"}
	got := make([]string, 0, len(view.Plans[0].FrontmatterOrder))
	for _, field := range view.Plans[0].FrontmatterOrder {
		got = append(got, field.Key)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("frontmatter order = %v, want %v", got, want)
	}
	var rendered bytes.Buffer
	renderPlanDetails(&rendered, view)
	if actual := runListCmd(t, "DevEnv", "task-002", "--dir", root); actual != rendered.String() {
		t.Fatalf("human detail differs from decoded JSON view:\n got: %q\nwant: %q", actual, rendered.String())
	}
}

func TestListHumanRendersDecodedOverview(t *testing.T) {
	root := projectsFixture(t)

	var view ProjectsView
	if err := json.Unmarshal([]byte(runListJSON(t, "--dir", root)), &view); err != nil {
		t.Fatalf("overview JSON: %v", err)
	}
	var want bytes.Buffer
	renderProjects(&want, view)
	got := runListCmd(t, "--dir", root)
	if got != want.String() {
		t.Fatalf("human view differs from decoded JSON view:\n got: %q\nwant: %q", got, want.String())
	}
}

func TestListUnknownProjectNamesChoices(t *testing.T) {
	root := projectsFixture(t)
	var stdout, stderr bytes.Buffer

	exit := Execute([]string{"list", "Nope", "--dir", root}, &stdout, &stderr)

	if exit != 2 || !strings.Contains(stderr.String(), "DevEnv") {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
}

func TestListPlanShowsAllFieldsInFileOrder(t *testing.T) {
	root := projectsFixture(t)

	out := runListCmd(t, "DevEnv", "task-002", "--dir", root)

	want := []string{"Title", "file", "tags", "#Ticket", "type", "status", "in-progress",
		"project", "date_created", "2026-03-04", "topics"}
	pos := 0
	for _, w := range want {
		i := strings.Index(out[pos:], w)
		if i < 0 {
			t.Fatalf("want %q after offset %d in:\n%s", w, pos, out)
		}
		pos += i + len(w)
	}
	if strings.Contains(out, "TASK-001") {
		t.Fatalf("matched the wrong plan:\n%s", out)
	}
}
