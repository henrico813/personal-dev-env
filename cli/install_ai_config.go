package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"
	"time"
)

const aiConfigStateVersion = 1

type skillOwnership struct {
	Version int      `json:"version"`
	Shared  []string `json:"shared"`
	Codex   []string `json:"codex"`
}
type aiConfigChange struct {
	source                                             string
	data                                               []byte
	destination, backupPath, stagedPath, generatedLink string
}
type aiConfigPlan struct{ changes []aiConfigChange }
type activatedAIConfigChange struct {
	change    aiConfigChange
	hadBackup bool
}
type treeEntry struct {
	mode   fs.FileMode
	data   []byte
	target string
}

func installAIConfig(cfg *Config, runner Runner) error { return syncAIConfig(cfg, runner) }
func syncAIConfig(cfg *Config, runner Runner) error {
	plan, err := planAIConfig(cfg)
	if err != nil {
		return err
	}
	if runner.DryRun {
		return previewAIConfig(plan, runner)
	}
	return withAIConfigLock(cfg, func() error {
		p, e := planAIConfig(cfg)
		if e != nil {
			return e
		}
		return applyAIConfigPlan(cfg, p)
	})
}
func newAIConfigChange(s, d, b string) aiConfigChange {
	return aiConfigChange{source: s, destination: d, backupPath: b}
}
func planAIConfig(cfg *Config) (*aiConfigPlan, error) {
	shared, e := discoverSkillPackages(filepath.Join(cfg.AIRepoDir, "skills"))
	if e != nil {
		return nil, fmt.Errorf("discover shared skills: %w", e)
	}
	codex, e := discoverSkillPackages(filepath.Join(cfg.AIRepoDir, "codex", "skills"))
	if e != nil {
		return nil, fmt.Errorf("discover Codex skills: %w", e)
	}
	if x := intersectNames(shared, codex); len(x) > 0 {
		return nil, fmt.Errorf("shared and Codex skills conflict: %s", strings.Join(x, ", "))
	}
	prev, has, e := readSkillOwnership(cfg)
	if e != nil {
		return nil, e
	}
	next := skillOwnership{aiConfigStateVersion, shared, codex}
	manifest, e := json.MarshalIndent(next, "", "  ")
	if e != nil {
		return nil, fmt.Errorf("encode AI config ownership: %w", e)
	}
	manifest = append(manifest, '\n')
	changes := []aiConfigChange{
		newAIConfigChange(filepath.Join(cfg.AIRepoDir, "opencode", "agents"), filepath.Join(cfg.OpenCodeConfigDir, "agents"), "opencode/agents"),
		newAIConfigChange(filepath.Join(cfg.AIRepoDir, "opencode", "commands"), filepath.Join(cfg.OpenCodeConfigDir, "commands"), "opencode/commands"),
		newAIConfigChange(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), filepath.Join(cfg.OpenCodeConfigDir, "AGENTS.md"), "opencode/AGENTS.md"),
		newAIConfigChange(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), filepath.Join(cfg.CodexConfigDir, "AGENTS.md"), "codex/AGENTS.md"),
		newAIConfigChange(filepath.Join(cfg.AIRepoDir, "pi", "agent", "settings.json"), filepath.Join(cfg.PiAgentDir, "settings.json"), "pi/settings.json"),
		newAIConfigChange(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), filepath.Join(cfg.PiAgentDir, "AGENTS.md"), "pi/AGENTS.md")}
	for _, c := range changes {
		if e := validateAIConfigSource(c.source); e != nil {
			return nil, e
		}
	}
	if !has {
		x, e := legacyAgentSkillBackupChanges(cfg)
		if e != nil {
			return nil, e
		}
		changes = append(changes, x...)
	}
	ps := nameSet(prev.Shared)
	pc := nameSet(append(append([]string{}, prev.Shared...), prev.Codex...))
	current := nameSet(append(append([]string{}, shared...), codex...))
	for _, n := range shared {
		src := filepath.Join(cfg.AIRepoDir, "skills", n)
		c := newAIConfigChange(src, filepath.Join(cfg.HomeDir, ".agents", "skills", n), filepath.Join("agents-skills", n))
		if e := validateSkillCollision(c, ps[n], nil); e != nil {
			return nil, e
		}
		changes = append(changes, c)
		c = newAIConfigChange(src, filepath.Join(cfg.CodexConfigDir, "skills", n), filepath.Join("codex-skills", n))
		if e := validateSkillCollision(c, pc[n], nil); e != nil {
			return nil, e
		}
		changes = append(changes, c)
	}
	for _, n := range codex {
		c := newAIConfigChange(filepath.Join(cfg.AIRepoDir, "codex", "skills", n), filepath.Join(cfg.CodexConfigDir, "skills", n), filepath.Join("codex-skills", n))
		if n == "create-plan" {
			c.generatedLink = filepath.Join(cfg.AIRuntimeDir, "planner", "planner")
		}
		var ignored map[string]struct{}
		if n == "create-plan" {
			ignored = map[string]struct{}{"bin": {}, "bin/planner": {}}
		}
		if e := validateSkillCollision(c, pc[n], ignored); e != nil {
			return nil, e
		}
		changes = append(changes, c)
	}
	for _, n := range prev.Shared {
		if !nameSet(shared)[n] {
			changes = append(changes, newAIConfigChange("", filepath.Join(cfg.HomeDir, ".agents", "skills", n), filepath.Join("agents-skills", n)))
		}
	}
	for n := range pc {
		if !current[n] {
			changes = append(changes, newAIConfigChange("", filepath.Join(cfg.CodexConfigDir, "skills", n), filepath.Join("codex-skills", n)))
		}
	}
	c := newAIConfigChange("", skillOwnershipPath(cfg), "skill-ownership.json")
	c.data = manifest
	changes = append(changes, c)
	return &aiConfigPlan{changes}, nil
}
func previewAIConfig(p *aiConfigPlan, r Runner) error {
	for _, c := range p.changes {
		if c.source != "" || c.data != nil {
			if e := r.Do("stage AI config for "+c.destination, func() error { return nil }); e != nil {
				return e
			}
		}
		ok, e := pathExists(c.destination)
		if e != nil {
			return e
		}
		if ok {
			if e := r.Rename("back up AI config", c.destination, filepath.Join("<ai-config-backup>", c.backupPath)); e != nil {
				return e
			}
		}
		if c.source != "" || c.data != nil {
			if e := r.Rename("activate AI config", "<staged>", c.destination); e != nil {
				return e
			}
		}
	}
	return nil
}
func applyAIConfigPlan(cfg *Config, p *aiConfigPlan) error {
	root, e := createAIConfigRunRoot(cfg)
	if e != nil {
		return e
	}
	stage := filepath.Join(root, ".stage")
	if e = os.MkdirAll(stage, 0755); e != nil {
		return fmt.Errorf("create AI config stage: %w", e)
	}
	for i := range p.changes {
		c := &p.changes[i]
		c.backupPath = filepath.Join(root, c.backupPath)
		if c.source == "" && c.data == nil {
			continue
		}
		c.stagedPath = filepath.Join(stage, fmt.Sprintf("%03d", i))
		if e := stageAIConfigChange(*c); e != nil {
			os.RemoveAll(root)
			return e
		}
	}
	backed, e := activateAIConfigChanges(p.changes)
	if e != nil {
		return fmt.Errorf("%w; recovery data retained at %s", e, root)
	}
	_ = os.RemoveAll(stage)
	if !backed {
		_ = os.RemoveAll(root)
	}
	return nil
}
func stageAIConfigChange(c aiConfigChange) error {
	if c.data != nil {
		if e := os.MkdirAll(filepath.Dir(c.stagedPath), 0755); e != nil {
			return e
		}
		if e := os.WriteFile(c.stagedPath, c.data, 0644); e != nil {
			return fmt.Errorf("stage %s: %w", c.destination, e)
		}
	} else if e := copyAIConfigSource(c.source, c.stagedPath); e != nil {
		return fmt.Errorf("stage %s: %w", c.source, e)
	}
	if c.generatedLink == "" {
		return nil
	}
	i, e := os.Stat(c.generatedLink)
	if os.IsNotExist(e) {
		return nil
	}
	if e != nil {
		return fmt.Errorf("stat generated Planner launcher: %w", e)
	}
	if !i.Mode().IsRegular() {
		return fmt.Errorf("generated Planner launcher is not a regular file: %s", c.generatedLink)
	}
	p := filepath.Join(c.stagedPath, "bin", "planner")
	if e := os.MkdirAll(filepath.Dir(p), 0755); e != nil {
		return e
	}
	return os.Symlink(c.generatedLink, p)
}
func activateAIConfigChanges(cs []aiConfigChange) (bool, error) {
	var a []activatedAIConfigChange
	any := false
	for _, c := range cs {
		had, e := moveAIConfigBackup(c)
		if e != nil {
			return any, rollbackAIConfig(a, e)
		}
		any = any || had
		if c.stagedPath != "" {
			if e = os.MkdirAll(filepath.Dir(c.destination), 0755); e != nil {
				re := restoreAIConfigChange(c, had)
				return any, rollbackAIConfig(a, combineAIConfigErrors(e, re))
			}
			if e = os.Rename(c.stagedPath, c.destination); e != nil {
				re := restoreAIConfigChange(c, had)
				return any, rollbackAIConfig(a, combineAIConfigErrors(e, re))
			}
		}
		a = append(a, activatedAIConfigChange{c, had})
	}
	return any, nil
}
func moveAIConfigBackup(c aiConfigChange) (bool, error) {
	ok, e := pathExists(c.destination)
	if e != nil || !ok {
		return false, e
	}
	if e = os.MkdirAll(filepath.Dir(c.backupPath), 0755); e != nil {
		return false, e
	}
	if e = os.Rename(c.destination, c.backupPath); e != nil {
		return false, fmt.Errorf("back up %s: %w", c.destination, e)
	}
	return true, nil
}
func restoreAIConfigChange(c aiConfigChange, had bool) error {
	if e := os.RemoveAll(c.destination); e != nil {
		return e
	}
	if !had {
		return nil
	}
	if e := os.MkdirAll(filepath.Dir(c.destination), 0755); e != nil {
		return e
	}
	return os.Rename(c.backupPath, c.destination)
}
func rollbackAIConfig(a []activatedAIConfigChange, cause error) error {
	var re error
	for i := len(a) - 1; i >= 0; i-- {
		if e := restoreAIConfigChange(a[i].change, a[i].hadBackup); e != nil && re == nil {
			re = e
		}
	}
	return combineAIConfigErrors(cause, re)
}
func combineAIConfigErrors(a, b error) error {
	if b == nil {
		return a
	}
	return fmt.Errorf("apply AI config: %w; restore previous config: %v", a, b)
}
func createAIConfigRunRoot(c *Config) (string, error) {
	p := filepath.Join(c.AIRuntimeDir, "ai-config-backups")
	if e := os.MkdirAll(p, 0755); e != nil {
		return "", fmt.Errorf("create AI config backup root: %w", e)
	}
	r, e := os.MkdirTemp(p, time.Now().UTC().Format("20060102_150405.000000000")+fmt.Sprintf(".%d-", os.Getpid()))
	if e != nil {
		return "", fmt.Errorf("create AI config run root: %w", e)
	}
	return r, nil
}
func withAIConfigLock(c *Config, fn func() error) error {
	if e := os.MkdirAll(c.AIRuntimeDir, 0755); e != nil {
		return fmt.Errorf("create AI runtime directory: %w", e)
	}
	f, e := os.OpenFile(filepath.Join(c.AIRuntimeDir, "ai-config.lock"), os.O_CREATE|os.O_RDWR, 0600)
	if e != nil {
		return fmt.Errorf("open AI config lock: %w", e)
	}
	defer f.Close()
	if e = syscall.Flock(int(f.Fd()), syscall.LOCK_EX); e != nil {
		return fmt.Errorf("lock AI config: %w", e)
	}
	defer syscall.Flock(int(f.Fd()), syscall.LOCK_UN)
	return fn()
}
func skillOwnershipPath(c *Config) string {
	return filepath.Join(c.AIRuntimeDir, "skill-ownership.json")
}
func readSkillOwnership(c *Config) (skillOwnership, bool, error) {
	d, e := os.ReadFile(skillOwnershipPath(c))
	if os.IsNotExist(e) {
		return skillOwnership{Version: 1}, false, nil
	}
	if e != nil {
		return skillOwnership{}, false, fmt.Errorf("read AI config ownership: %w", e)
	}
	dec := json.NewDecoder(bytes.NewReader(d))
	dec.DisallowUnknownFields()
	var s skillOwnership
	if e = dec.Decode(&s); e != nil {
		return skillOwnership{}, false, fmt.Errorf("decode AI config ownership: %w", e)
	}
	if e = dec.Decode(&struct{}{}); e != io.EOF {
		return skillOwnership{}, false, fmt.Errorf("decode AI config ownership: trailing data")
	}
	if s.Version != 1 {
		return skillOwnership{}, false, fmt.Errorf("unsupported AI config ownership version: %d", s.Version)
	}
	if e = validateOwnedNames("shared", s.Shared); e != nil {
		return skillOwnership{}, false, e
	}
	if e = validateOwnedNames("Codex", s.Codex); e != nil {
		return skillOwnership{}, false, e
	}
	sort.Strings(s.Shared)
	sort.Strings(s.Codex)
	return s, true, nil
}
func legacyAgentSkillBackupChanges(c *Config) ([]aiConfigChange, error) {
	root := filepath.Join(c.HomeDir, ".agents", "skills")
	es, e := os.ReadDir(root)
	if os.IsNotExist(e) {
		return nil, nil
	}
	if e != nil {
		return nil, fmt.Errorf("read legacy skill backups: %w", e)
	}
	var out []aiConfigChange
	for _, x := range es {
		if !strings.HasPrefix(x.Name(), "git-messages.backup.") {
			continue
		}
		if !x.IsDir() {
			return nil, fmt.Errorf("legacy skill backup is not a directory: %s", x.Name())
		}
		if _, e = time.Parse("20060102_150405", strings.TrimPrefix(x.Name(), "git-messages.backup.")); e != nil {
			return nil, fmt.Errorf("invalid legacy skill backup name: %s", x.Name())
		}
		d := filepath.Join(root, x.Name())
		n, e := readSkillFrontmatterName(filepath.Join(d, "SKILL.md"))
		if e != nil {
			return nil, e
		}
		if n != "git-messages" {
			return nil, fmt.Errorf("legacy skill backup has unexpected name %q: %s", n, d)
		}
		out = append(out, newAIConfigChange("", d, filepath.Join("legacy-agents-skills", x.Name())))
	}
	return out, nil
}
func validateOwnedNames(k string, ns []string) error {
	seen := map[string]struct{}{}
	for _, n := range ns {
		if !validSkillName(n) {
			return fmt.Errorf("invalid owned %s skill name: %s", k, n)
		}
		if _, ok := seen[n]; ok {
			return fmt.Errorf("duplicate owned %s skill name: %s", k, n)
		}
		seen[n] = struct{}{}
	}
	return nil
}
func discoverSkillPackages(root string) ([]string, error) {
	es, e := os.ReadDir(root)
	if e != nil {
		return nil, e
	}
	var ns []string
	for _, x := range es {
		if !x.IsDir() {
			continue
		}
		n := x.Name()
		if !validSkillName(n) {
			return nil, fmt.Errorf("invalid skill directory name: %s", n)
		}
		p := filepath.Join(root, n)
		if e = validateAIConfigSource(p); e != nil {
			return nil, e
		}
		m, e := readSkillFrontmatterName(filepath.Join(p, "SKILL.md"))
		if e != nil {
			return nil, e
		}
		if m != n {
			return nil, fmt.Errorf("skill name %q does not match directory %q", m, n)
		}
		ns = append(ns, n)
	}
	sort.Strings(ns)
	return ns, nil
}
func readSkillFrontmatterName(p string) (string, error) {
	i, e := os.Lstat(p)
	if e != nil {
		return "", fmt.Errorf("stat skill metadata %s: %w", p, e)
	}
	if !i.Mode().IsRegular() {
		return "", fmt.Errorf("skill metadata is not a regular file: %s", p)
	}
	f, e := os.Open(p)
	if e != nil {
		return "", fmt.Errorf("open skill metadata %s: %w", p, e)
	}
	defer f.Close()
	s := bufio.NewScanner(f)
	if !s.Scan() || s.Text() != "---" {
		return "", fmt.Errorf("skill metadata must start with frontmatter: %s", p)
	}
	name := ""
	for s.Scan() {
		line := s.Text()
		if line == "---" {
			if !validSkillName(name) {
				return "", fmt.Errorf("skill metadata has invalid name %q: %s", name, p)
			}
			return name, nil
		}
		k, v, ok := strings.Cut(line, ":")
		if ok && strings.TrimSpace(k) == "name" {
			if name != "" {
				return "", fmt.Errorf("skill metadata has duplicate name: %s", p)
			}
			name = strings.TrimSpace(v)
		}
	}
	if e = s.Err(); e != nil {
		return "", fmt.Errorf("read skill metadata %s: %w", p, e)
	}
	return "", fmt.Errorf("skill metadata has no closing frontmatter: %s", p)
}
func validSkillName(n string) bool {
	if n == "" || n[0] == '-' || n[len(n)-1] == '-' {
		return false
	}
	for _, c := range n {
		if (c < 'a' || c > 'z') && (c < '0' || c > '9') && c != '-' {
			return false
		}
	}
	return true
}
func validateAIConfigSource(p string) error {
	i, e := os.Lstat(p)
	if e != nil {
		return fmt.Errorf("validate AI config source %s: %w", p, e)
	}
	if i.Mode().IsRegular() {
		return validateReadableRegularFile(p)
	}
	if !i.IsDir() {
		return fmt.Errorf("AI config source is not a file or directory: %s", p)
	}
	return filepath.WalkDir(p, func(cur string, d fs.DirEntry, w error) error {
		if w != nil {
			return w
		}
		i, e := d.Info()
		if e != nil {
			return e
		}
		if i.IsDir() || i.Mode()&os.ModeSymlink != 0 {
			return nil
		}
		if !i.Mode().IsRegular() {
			return fmt.Errorf("unsupported AI config source: %s", cur)
		}
		return validateReadableRegularFile(cur)
	})
}
func validateSkillCollision(c aiConfigChange, owned bool, ignored map[string]struct{}) error {
	ok, e := pathExists(c.destination)
	if e != nil || !ok || owned {
		return e
	}
	same, e := equalTrees(c.source, c.destination, ignored)
	if e != nil {
		return e
	}
	if !same {
		return fmt.Errorf("unmanaged skill collision at %s", c.destination)
	}
	return nil
}
func equalTrees(a, b string, ignored map[string]struct{}) (bool, error) {
	x, e := snapshotTree(a, ignored)
	if e != nil {
		return false, e
	}
	y, e := snapshotTree(b, ignored)
	if e != nil {
		return false, e
	}
	if len(x) != len(y) {
		return false, nil
	}
	for p, v := range x {
		w, ok := y[p]
		if !ok || v.mode != w.mode || v.target != w.target || !bytes.Equal(v.data, w.data) {
			return false, nil
		}
	}
	return true, nil
}
func snapshotTree(root string, ignored map[string]struct{}) (map[string]treeEntry, error) {
	out := map[string]treeEntry{}
	e := filepath.WalkDir(root, func(p string, d fs.DirEntry, w error) error {
		if w != nil {
			return w
		}
		rel, e := filepath.Rel(root, p)
		if e != nil || rel == "." {
			return e
		}
		if _, ok := ignored[filepath.ToSlash(rel)]; ok {
			return nil
		}
		i, e := d.Info()
		if e != nil {
			return e
		}
		v := treeEntry{mode: i.Mode()}
		if i.Mode()&os.ModeSymlink != 0 {
			v.target, e = os.Readlink(p)
		} else if i.Mode().IsRegular() {
			v.data, e = os.ReadFile(p)
		}
		if e != nil {
			return e
		}
		out[rel] = v
		return nil
	})
	return out, e
}
func copyAIConfigSource(s, d string) error {
	i, e := os.Stat(s)
	if e != nil {
		return e
	}
	if i.IsDir() {
		return copyTreeInto(s, d)
	}
	b, e := os.ReadFile(s)
	if e != nil {
		return e
	}
	if e = os.MkdirAll(filepath.Dir(d), 0755); e != nil {
		return e
	}
	return os.WriteFile(d, b, i.Mode().Perm())
}
func pathExists(p string) (bool, error) {
	_, e := os.Lstat(p)
	if os.IsNotExist(e) {
		return false, nil
	}
	return e == nil, e
}
func nameSet(ns []string) map[string]bool {
	s := map[string]bool{}
	for _, n := range ns {
		s[n] = true
	}
	return s
}
func intersectNames(a, b []string) []string {
	r := nameSet(b)
	var o []string
	for _, n := range a {
		if r[n] {
			o = append(o, n)
		}
	}
	return o
}
