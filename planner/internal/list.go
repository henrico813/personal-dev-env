package internal

import (
	"fmt"
	"io"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"text/tabwriter"
)

// vaultProjectsSubdir is where project folders live inside the PDE vault,
// per the vault's Projects/AGENTS.md.
const vaultProjectsSubdir = "500 Zettelkasten/Projects"

// PlanSummary is one plan found by planner list.
type PlanSummary struct {
	Project     string         `json:"project_dir"`
	Path        string         `json:"path"`
	Title       string         `json:"title"`
	Status      PlanStatus     `json:"status"`
	RawStatus   any            `json:"raw_status"`
	Frontmatter map[string]any `json:"frontmatter"`
	keys        []string
}

// StatusCount counts plans with one normalized status.
type StatusCount struct {
	Status PlanStatus `json:"status"`
	Count  int        `json:"count"`
}

// ProjectSummary reports counts for one containing project folder.
type ProjectSummary struct {
	ProjectDir   string        `json:"project_dir"`
	Total        int           `json:"total"`
	StatusCounts []StatusCount `json:"status_counts"`
}

// ProjectsView is the JSON response for the projects overview.
type ProjectsView struct {
	View     string           `json:"view"`
	Projects []ProjectSummary `json:"projects"`
}

// PlansView contains plans returned by a list query.
type PlansView struct {
	View       string        `json:"view"`
	ProjectDir string        `json:"project_dir,omitempty"`
	Plans      []PlanSummary `json:"plans"`
}

// FrontmatterField pairs a frontmatter key with its parsed value.
type FrontmatterField struct {
	Key   string `json:"key"`
	Value any    `json:"value"`
}

// PlanDetail includes parsed values and their source order.
type PlanDetail struct {
	ProjectDir       string             `json:"project_dir"`
	Path             string             `json:"path"`
	Title            string             `json:"title"`
	Status           PlanStatus         `json:"status"`
	RawStatus        any                `json:"raw_status"`
	Frontmatter      map[string]any     `json:"frontmatter"`
	FrontmatterOrder []FrontmatterField `json:"frontmatter_order"`
}

// PlanDetailsView is the JSON response for plan detail queries.
type PlanDetailsView struct {
	View  string       `json:"view"`
	Plans []PlanDetail `json:"plans"`
}

func runList(args []string, stdout io.Writer, stderr io.Writer) int {
	const usage = "usage: planner list [PROJECT [PLAN]] [--status STATUS] [--dir PROJECTS_DIR] [--json]"
	project, plan, status, root := "", "", "", ""
	var statusFilter PlanStatus
	asJSON := false
	for i := 0; i < len(args); i++ {
		switch arg := args[i]; {
		case arg == "--json":
			asJSON = true
		case arg == "--status" || arg == "--dir":
			i++
			if i >= len(args) || args[i] == "" {
				reportError(stderr, "list", newPlannerCLIError(PlannerUsageError, nil, usage))
				return 2
			}
			if arg == "--status" {
				status = args[i]
				statusFilter = normalizePlanStatus(status)
			} else {
				root = args[i]
			}
		case strings.HasPrefix(arg, "--") || plan != "":
			reportError(stderr, "list", newPlannerCLIError(PlannerUsageError, nil, usage))
			return 2
		case project == "":
			project = arg
		default:
			plan = strings.ToLower(arg)
		}
	}

	if root == "" {
		var err error
		if root, err = defaultProjectsDir(); err != nil {
			reportError(stderr, "list", newPlannerCLIError(PlannerReadInputError, err,
				"cannot find the vault Projects folder; pass --dir PROJECTS_DIR"))
			return 1
		}
	}
	plans, err := listPlans(root)
	if err != nil {
		reportError(stderr, "list", newPlannerCLIError(PlannerReadInputError, err, root))
		return 1
	}
	if project != "" {
		project, err = matchProject(root, project)
		if err != nil {
			reportError(stderr, "list", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
	}
	kept := plans[:0]
	for _, p := range plans {
		if (project == "" || p.Project == project) && (status == "" || p.Status == statusFilter) &&
			(plan == "" || strings.Contains(strings.ToLower(filepath.Base(p.Path)), plan)) {
			kept = append(kept, p)
		}
	}
	plans = kept

	var view any
	switch {
	case plan != "":
		view = newPlanDetailsView(plans)
	case project != "":
		view = PlansView{View: "project-plans", ProjectDir: project, Plans: plans}
	case status != "":
		view = PlansView{View: "plans", Plans: plans}
	default:
		view = newProjectsView(plans)
	}
	if asJSON {
		out, err := MarshalJSONNoEscape(view)
		if err != nil {
			reportError(stderr, "list", newPlannerCLIError(PlannerWriteOutputError, err, "list JSON"))
			return 1
		}
		_, _ = stdout.Write(append(out, '\n'))
		return 0
	}
	switch v := view.(type) {
	case PlanDetailsView:
		if plan != "" && len(v.Plans) == 0 {
			reportError(stderr, "list", newPlannerCLIError(PlannerUsageError, nil,
				fmt.Sprintf("no plan in %s matches %q", project, plan)))
			return 2
		}
		renderPlanDetails(stdout, v)
	case ProjectsView:
		renderProjects(stdout, v)
	case PlansView:
		if project == "" {
			renderPlans(stdout, root, v)
		} else {
			renderProjectPlans(stdout, root, v)
		}
	}
	return 0
}

// defaultProjectsDir resolves the vault with `pde vault path default`.
func defaultProjectsDir() (string, error) {
	out, err := exec.Command("pde", "vault", "path", "default").Output()
	if err != nil {
		return "", fmt.Errorf("pde vault path default: %w", err)
	}
	return filepath.Join(strings.TrimSpace(string(out)), vaultProjectsSubdir), nil
}

// matchProject returns the project folder name under root matching name,
// ignoring case, so "devenv" finds "DevEnv".
func matchProject(root, name string) (string, error) {
	entries, err := os.ReadDir(root)
	if err != nil {
		return "", err
	}
	var names []string
	for _, e := range entries {
		if !e.IsDir() || strings.HasPrefix(e.Name(), ".") {
			continue
		}
		if strings.EqualFold(e.Name(), name) {
			return e.Name(), nil
		}
		names = append(names, e.Name())
	}
	return "", fmt.Errorf("no project %q in %s; projects: %s", name, root, strings.Join(names, ", "))
}

// newProjectsView summarizes plans by their containing project directory.
func newProjectsView(plans []PlanSummary) ProjectsView {
	byProject := map[string]*ProjectSummary{}
	for _, plan := range plans {
		project, ok := byProject[plan.Project]
		if !ok {
			project = &ProjectSummary{ProjectDir: plan.Project}
			for _, status := range planStatusOrder(false) {
				project.StatusCounts = append(project.StatusCounts, StatusCount{Status: status})
			}
			byProject[plan.Project] = project
		}
		project.Total++
		found := false
		for i := range project.StatusCounts {
			if project.StatusCounts[i].Status == plan.Status {
				project.StatusCounts[i].Count++
				found = true
				break
			}
		}
		if !found {
			project.StatusCounts = append(project.StatusCounts, StatusCount{Status: plan.Status, Count: 1})
		}
	}
	projects := make([]ProjectSummary, 0, len(byProject))
	for _, project := range byProject {
		projects = append(projects, *project)
	}
	sort.Slice(projects, func(i, j int) bool {
		return strings.ToLower(projects[i].ProjectDir) < strings.ToLower(projects[j].ProjectDir)
	})
	return ProjectsView{View: "projects", Projects: projects}
}

func newPlanDetailsView(plans []PlanSummary) PlanDetailsView {
	view := PlanDetailsView{View: "plan-details", Plans: make([]PlanDetail, 0, len(plans))}
	for _, plan := range plans {
		detail := PlanDetail{
			ProjectDir: plan.Project, Path: plan.Path, Title: plan.Title,
			Status: plan.Status, RawStatus: plan.RawStatus,
			Frontmatter: plan.Frontmatter, FrontmatterOrder: make([]FrontmatterField, 0, len(plan.keys)),
		}
		for _, key := range plan.keys {
			detail.FrontmatterOrder = append(detail.FrontmatterOrder, FrontmatterField{Key: key, Value: plan.Frontmatter[key]})
		}
		view.Plans = append(view.Plans, detail)
	}
	return view
}

// renderProjects prints the projects overview from its JSON view model.
func renderProjects(w io.Writer, view ProjectsView) {
	if len(view.Projects) == 0 {
		_, _ = fmt.Fprintln(w, "No plans found.")
		return
	}
	includeUnknown := false
	for _, project := range view.Projects {
		for _, count := range project.StatusCounts {
			if count.Status == PlanStatusUnknown && count.Count > 0 {
				includeUnknown = true
			}
		}
	}
	statuses := planStatusOrder(includeUnknown)

	tw := tabwriter.NewWriter(w, 0, 0, 2, ' ', 0)
	_, _ = fmt.Fprint(tw, "PROJECT\tPLANS")
	for _, status := range statuses {
		_, _ = fmt.Fprintf(tw, "\t%s", frontmatterDisplay(status))
	}
	_, _ = fmt.Fprintln(tw)
	for _, project := range view.Projects {
		counts := make(map[PlanStatus]int, len(project.StatusCounts))
		for _, count := range project.StatusCounts {
			counts[count.Status] = count.Count
		}
		_, _ = fmt.Fprintf(tw, "%s\t%d", project.ProjectDir, project.Total)
		for _, status := range statuses {
			_, _ = fmt.Fprintf(tw, "\t%d", counts[status])
		}
		_, _ = fmt.Fprintln(tw)
	}
	_ = tw.Flush()
}

// renderProjectPlans prints project plan rows from their JSON view model.
func renderProjectPlans(w io.Writer, root string, view PlansView) {
	renderPlans(w, root, view)
}

func renderPlans(w io.Writer, root string, view PlansView) {
	if len(view.Plans) == 0 {
		_, _ = fmt.Fprintln(w, "No plans found.")
		return
	}
	tw := tabwriter.NewWriter(w, 0, 0, 2, ' ', 0)
	_, _ = fmt.Fprintln(tw, "STATUS\tCREATED\tFILE")
	for _, plan := range view.Plans {
		file := plan.Path
		if rel, err := filepath.Rel(filepath.Join(root, plan.Project), plan.Path); err == nil {
			file = rel
		}
		_, _ = fmt.Fprintf(tw, "%s\t%s\t%s\n",
			plan.Status,
			orDash(frontmatterString(plan.Frontmatter, "date_created")), file)
	}
	_ = tw.Flush()
}

// renderPlanDetails prints ordered frontmatter fields from the detail view.
func renderPlanDetails(w io.Writer, view PlanDetailsView) {
	tw := tabwriter.NewWriter(w, 0, 0, 2, ' ', 0)
	for i, plan := range view.Plans {
		if i > 0 {
			_, _ = fmt.Fprintln(tw)
		}
		_, _ = fmt.Fprintln(tw, plan.Title)
		_, _ = fmt.Fprintf(tw, "  file\t%s\n", plan.Path)
		for _, field := range plan.FrontmatterOrder {
			value := field.Value
			if field.Key == "status" {
				value = plan.Status
			}
			_, _ = fmt.Fprintf(tw, "  %s\t%s\n", field.Key, orDash(frontmatterDisplay(value)))
		}
	}
	_ = tw.Flush()
}

func frontmatterDisplay(value any) string {
	switch v := value.(type) {
	case nil:
		return ""
	case string:
		return v
	case []string:
		return strings.Join(v, ", ")
	case []any:
		items := make([]string, len(v))
		for i, item := range v {
			items[i] = fmt.Sprint(item)
		}
		return strings.Join(items, ", ")
	default:
		return fmt.Sprint(v)
	}
}

func orDash(s string) string {
	if s == "" {
		return "-"
	}
	return s
}

// listPlans returns markdown files inside root's project folders whose
// frontmatter has type: issue, sorted by path. Files directly in root,
// hidden directories such as .obsidian, and notes without frontmatter are
// skipped.
func listPlans(root string) ([]PlanSummary, error) {
	plans := []PlanSummary{}
	err := filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			if path != root && strings.HasPrefix(d.Name(), ".") {
				return filepath.SkipDir
			}
			return nil
		}
		if !strings.EqualFold(filepath.Ext(path), ".md") {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		project, _, nested := strings.Cut(filepath.ToSlash(rel), "/")
		if !nested {
			return nil
		}
		raw, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		fm, keys, body, ok := parseFrontmatter(string(raw))
		if !ok || fm["type"] != "issue" {
			return nil
		}
		rawStatus := fm["status"]
		plans = append(plans, PlanSummary{
			Project: project, Path: path, Title: markdownTitle(body, path),
			Status: normalizePlanStatus(rawStatus), RawStatus: rawStatus,
			Frontmatter: fm, keys: keys,
		})
		return nil
	})
	if err != nil {
		return nil, err
	}
	return plans, nil
}

// parseFrontmatter reads the flat YAML subset Obsidian properties use:
// "key: value" scalars and "key:" followed by "  - item" lists. Other YAML,
// such as nested maps, is kept as the raw scalar text or skipped.
// Example: "tags:\n  - \"#Ticket\"\nstatus: open" yields
// {"tags": ["#Ticket"], "status": "open"}.
func parseFrontmatter(input string) (fm map[string]any, keys []string, body string, ok bool) {
	input = strings.ReplaceAll(input, "\r\n", "\n")
	if !strings.HasPrefix(input, "---\n") {
		return nil, nil, input, false
	}
	rest := input[len("---\n"):]
	end := strings.Index(rest, "\n---")
	if end < 0 {
		return nil, nil, input, false
	}
	block := rest[:end]
	body = strings.TrimPrefix(rest[end+len("\n---"):], "\n")

	fm = map[string]any{}
	listKey := ""
	for _, line := range strings.Split(block, "\n") {
		trimmed := strings.TrimSpace(line)
		if listKey != "" && strings.HasPrefix(trimmed, "- ") {
			items, _ := fm[listKey].([]string)
			fm[listKey] = append(items, unquoteYAML(strings.TrimPrefix(trimmed, "- ")))
			continue
		}
		listKey = ""
		key, value, found := strings.Cut(line, ":")
		if !found || key == "" || strings.HasPrefix(key, " ") {
			continue
		}
		if _, seen := fm[key]; !seen {
			keys = append(keys, key)
		}
		value = strings.TrimSpace(value)
		if value == "" {
			listKey = key
			fm[key] = []string{}
			continue
		}
		if value == "[]" {
			fm[key] = []string{}
			continue
		}
		fm[key] = unquoteYAML(value)
	}
	return fm, keys, body, true
}

func unquoteYAML(value string) string {
	value = strings.TrimSpace(value)
	if len(value) >= 2 && (value[0] == '"' || value[0] == '\'') && value[len(value)-1] == value[0] {
		return value[1 : len(value)-1]
	}
	return value
}

// markdownTitle returns the first "# " heading, or the file name without
// its extension when the note has none.
func markdownTitle(body, path string) string {
	for _, line := range strings.Split(body, "\n") {
		if strings.HasPrefix(line, "# ") {
			return strings.TrimSpace(strings.TrimPrefix(line, "# "))
		}
	}
	return strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
}

func frontmatterString(fm map[string]any, key string) string {
	if s, ok := fm[key].(string); ok {
		return s
	}
	return ""
}
