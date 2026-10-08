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

// This path comes from the PDE vault's Projects/AGENTS.md guidance.
const vaultProjectsSubdir = "500 Zettelkasten/Projects"

type planSummary struct {
	Project     string         `json:"project_dir"`
	Path        string         `json:"path"`
	Title       string         `json:"title"`
	Status      planStatus     `json:"status"`
	Frontmatter map[string]any `json:"frontmatter"`
	// JSON object keys do not preserve source order, so this array carries the
	// order needed by plan-detail output.
	FrontmatterOrder []frontmatterField `json:"frontmatter_order"`
}

type statusCount struct {
	Status planStatus `json:"status"`
	Count  int        `json:"count"`
}

type projectSummary struct {
	ProjectDir   string        `json:"project_dir"`
	Total        int           `json:"total"`
	StatusCounts []statusCount `json:"status_counts"`
}

type projectsView struct {
	View     string           `json:"view"`
	Projects []projectSummary `json:"projects"`
}

type plansView struct {
	View       string        `json:"view"`
	ProjectDir string        `json:"project_dir,omitempty"`
	Plans      []planSummary `json:"plans"`
}

type frontmatterField struct {
	Key   string `json:"key"`
	Value any    `json:"value"`
}

func runList(args []string, stdout io.Writer, stderr io.Writer) int {
	const usage = "usage: planner list [PROJECT [PLAN]] [--status STATUS] [--dir PROJECTS_DIR] [--json]"
	project, plan, status, root := "", "", "", ""
	var statusFilter planStatus
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
				var ok bool
				statusFilter, ok = parsePlanStatus(status)
				if !ok {
					reportError(stderr, "list", newPlannerCLIError(PlannerUsageError, nil,
						fmt.Sprintf("invalid status %q; accepted values: %s", status, acceptedPlanStatuses())))
					return 2
				}
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
	if plan != "" && len(plans) == 0 {
		reportError(stderr, "list", newPlannerCLIError(PlannerUsageError, nil,
			fmt.Sprintf("no plan in %s matches %q", project, plan)))
		return 2
	}

	var view any
	switch {
	case plan != "":
		view = plansView{View: "plan-details", Plans: plans}
	case project != "":
		view = plansView{View: "project-plans", ProjectDir: project, Plans: plans}
	case status != "":
		view = plansView{View: "plans", Plans: plans}
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
	case plansView:
		if v.View == "plan-details" {
			renderPlanDetails(stdout, v)
		} else {
			renderPlans(stdout, root, v)
		}
	case projectsView:
		renderProjects(stdout, v)
	}
	return 0
}

func defaultProjectsDir() (string, error) {
	out, err := exec.Command("pde", "vault", "path", "default").Output()
	if err != nil {
		return "", fmt.Errorf("pde vault path default: %w", err)
	}
	return filepath.Join(strings.TrimSpace(string(out)), vaultProjectsSubdir), nil
}

// Project names match without regard to case, so "devenv" finds "DevEnv".
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

func newProjectsView(plans []planSummary) projectsView {
	byProject := map[string]*projectSummary{}
	for _, plan := range plans {
		project, ok := byProject[plan.Project]
		if !ok {
			project = &projectSummary{ProjectDir: plan.Project}
			for _, status := range standardPlanStatuses {
				project.StatusCounts = append(project.StatusCounts, statusCount{Status: status})
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
			project.StatusCounts = append(project.StatusCounts, statusCount{Status: plan.Status, Count: 1})
		}
	}
	projects := make([]projectSummary, 0, len(byProject))
	for _, project := range byProject {
		projects = append(projects, *project)
	}
	sort.Slice(projects, func(i, j int) bool {
		return strings.ToLower(projects[i].ProjectDir) < strings.ToLower(projects[j].ProjectDir)
	})
	return projectsView{View: "projects", Projects: projects}
}

func renderProjects(w io.Writer, view projectsView) {
	if len(view.Projects) == 0 {
		_, _ = fmt.Fprintln(w, "No plans found.")
		return
	}
	includeUnknown := false
	for _, project := range view.Projects {
		for _, count := range project.StatusCounts {
			if count.Status == statusUnknown && count.Count > 0 {
				includeUnknown = true
			}
		}
	}
	statuses := append([]planStatus(nil), standardPlanStatuses...)
	if includeUnknown {
		statuses = append(statuses, statusUnknown)
	}

	tw := tabwriter.NewWriter(w, 0, 0, 2, ' ', 0)
	_, _ = fmt.Fprint(tw, "PROJECT\tPLANS")
	for _, status := range statuses {
		_, _ = fmt.Fprintf(tw, "\t%s", string(status))
	}
	_, _ = fmt.Fprintln(tw)
	for _, project := range view.Projects {
		counts := make(map[planStatus]int, len(project.StatusCounts))
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

func renderPlans(w io.Writer, root string, view plansView) {
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

func renderPlanDetails(w io.Writer, view plansView) {
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

func listPlans(root string) ([]planSummary, error) {
	plans := []planSummary{}
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
		frontmatterOrder := make([]frontmatterField, 0, len(keys))
		for _, key := range keys {
			frontmatterOrder = append(frontmatterOrder, frontmatterField{Key: key, Value: fm[key]})
		}
		plans = append(plans, planSummary{
			Project: project, Path: path, Title: markdownTitle(body, path),
			Status: normalizePlanStatus(fm["status"]), Frontmatter: fm,
			FrontmatterOrder: frontmatterOrder,
		})
		return nil
	})
	if err != nil {
		return nil, err
	}
	return plans, nil
}

// Frontmatter parsing accepts scalar pairs and lists, not nested YAML.
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
