package card

import (
	"fmt"
	"strings"
	"unicode/utf16"
)

// Runs inside Obsidian's Templater plugin. Keep its name rule the same as
// badNameChars, fill() the same as slotPattern and filters.
const templaterScript = `<%*
const root = "500 Zettelkasten/Projects";
const projects = [];
for (const folder of app.vault.getAbstractFileByPath(root).children) {
  const agents = app.vault.getAbstractFileByPath(` + "`${folder.path}/AGENTS.md`" + `);
  if (!agents) continue;
  const text = await app.vault.read(agents);
  const found = text.match(/Project code:\s*` + "`?([A-Z][A-Z0-9]*)`?" + `/);
  if (found) projects.push({ name: folder.name, code: found[1], folder });
}
const project = await tp.system.suggester(
  (p) => p.name, projects, true, "Project");
const title = (await tp.system.prompt("Card title", "", true)).trim();
const kinds = KINDS_JSON;
const kind = await tp.system.suggester(
  Object.entries(kinds).map(([name, meaning]) => ` + "`${name}: ${meaning}`" + `),
  Object.keys(kinds), true, "Kind");
const fill = (template, values) => template.replace(
  /\{([a-z_]+)(?:\|([a-z0-9]+))?\}/g, (slot, field, filter) => {
    const value = String(values[field]);
    if (filter === "lower") return value.toLowerCase();
    if (filter === "slug") {
      return (value.toLowerCase().match(/[a-z0-9]+/g) || []).join("-");
    }
    if (filter === "pad3") return value.padStart(3, "0");
    return value;
  });
const cardFileRe = new RegExp(CARD_FILE_RE_JSON);
const planFileRe = new RegExp(PLAN_FILE_RE_JSON);
let high = 0;
for (const file of app.vault.getMarkdownFiles()) {
  if (!file.path.startsWith(` + "`${root}/`" + `)) continue;
  const found = file.name.match(cardFileRe) || file.name.match(planFileRe);
  const key = found?.groups.key;
  if (key?.startsWith(` + "`${project.code}-`" + `)) {
    const digits = key.slice(project.code.length).replace(/[^0-9]/g, "");
    high = Math.max(high, Number(digits));
  }
}
const key = fill(CARD_KEY_MAKE_JSON, { code: project.code, n: high + 1 });
const today = tp.date.now("YYYY-MM-DD");
const name = title.replace(/[\\/:*?"<>|#^\[\]]/g, "").trim();
const fileName = fill(CARD_FILE_MAKE_JSON, { key, title: name });
await tp.file.move(` + "`${project.folder.path}/${fileName.replace(/\\.md$/, \"\")}`" + `);
-%>
`

// Renders the Obsidian template 001.09 Zettel-Kanban.md. Its Templater
// script's patterns come from the naming rules.
func RenderTemplate(rules NameRules) (string, error) {
	keyRule, err := rules.FindNameRule("card_key")
	if err != nil {
		return "", err
	}
	fileRule, err := rules.FindNameRule("card_file")
	if err != nil {
		return "", err
	}
	planRule, err := rules.FindNameRule("plan_file")
	if err != nil {
		return "", err
	}
	kinds := make([]string, len(Kinds))
	for i, kind := range Kinds {
		kinds[i] = QuoteASCII(kind) + ": " + QuoteASCII(KindMeanings[kind])
	}
	script := strings.NewReplacer(
		"KINDS_JSON", "{"+strings.Join(kinds, ", ")+"}",
		"CARD_KEY_MAKE_JSON", QuoteASCII(keyRule.Make),
		"CARD_FILE_MAKE_JSON", QuoteASCII(fileRule.Make),
		"CARD_FILE_RE_JSON", QuoteASCII(formatJSPattern(fileRule)),
		"PLAN_FILE_RE_JSON", QuoteASCII(formatJSPattern(planRule)),
	).Replace(templaterScript)
	text := renderCard("<% key %>", "<% title %>", "<% project.name %>", "<% kind %>", "<% today %>")
	return strings.Replace(text, "---\n", "---\n"+script, 1), nil
}

// Quotes text as Python's json.dumps does by default, with every non-ASCII
// character escaped, such as "Größe".
func QuoteASCII(text string) string {
	var quoted strings.Builder
	quoted.WriteByte('"')
	for _, r := range text {
		switch {
		case r == '"' || r == '\\':
			quoted.WriteString(`\` + string(r))
		case r == '\n':
			quoted.WriteString(`\n`)
		case r == '\r':
			quoted.WriteString(`\r`)
		case r == '\t':
			quoted.WriteString(`\t`)
		case r == '\b':
			quoted.WriteString(`\b`)
		case r == '\f':
			quoted.WriteString(`\f`)
		case r < 0x20 || (r > 0x7f && r <= 0xffff):
			fmt.Fprintf(&quoted, `\u%04x`, r)
		case r > 0xffff:
			high, low := utf16.EncodeRune(r)
			fmt.Fprintf(&quoted, `\u%04x\u%04x`, high, low)
		default:
			quoted.WriteRune(r)
		}
	}
	quoted.WriteByte('"')
	return quoted.String()
}

// Turns each (?P<name>...) group into JavaScript's (?<name>...).
func formatJSPattern(rule NameRule) string {
	return strings.ReplaceAll(rule.Parse.String(), "(?P<", "(?<")
}

// Renders a new draft card's text, with empty sections and one Log line.
func renderCard(cardKey, title, project, kind, today string) string {
	values := map[string][]string{
		"tags":     {QuoteASCII("#Kanban")},
		"plans":    nil,
		"prs":      nil,
		"sessions": nil,
		"topics":   {QuoteASCII("[[" + project + "]]")},
	}
	plain := map[string]string{
		"type":             "card",
		"stage":            Draft,
		"stage_date":       today,
		"template_version": "2",
		"project":          project,
		"kind":             kind,
		"date_created":     today,
	}
	lines := []string{"---"}
	for _, name := range Fields {
		items, isList := values[name]
		switch {
		case !isList:
			lines = append(lines, name+": "+plain[name])
		case len(items) == 0:
			lines = append(lines, name+": []")
		default:
			lines = append(lines, name+":")
			for _, item := range items {
				lines = append(lines, "  - "+item)
			}
		}
	}
	lines = append(lines, "---", "", "# "+cardKey+" "+title, "---", "")
	for _, name := range Sections {
		lines = append(lines, "## "+name, "---", "")
		if name == "Log" {
			lines = append(lines, "- "+today+" "+Draft+": by the user in Obsidian", "")
		}
	}
	return strings.TrimRight(strings.Join(lines, "\n"), "\n") + "\n"
}
