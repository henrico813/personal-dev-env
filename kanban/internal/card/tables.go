package card

// The Projects folder's path inside the vault.
const ProjectsFolder = "500 Zettelkasten/Projects"

// The stages a card can be in.
const (
	Draft    = "draft"
	Todo     = "todo"
	Active   = "active"
	InReview = "in-review"
	Blocked  = "blocked"
	Done     = "done"
	Dropped  = "dropped"
)

// The stages in board order.
var Stages = []string{Draft, Todo, Active, InReview, Blocked, Done, Dropped}

// The card kinds. A card made without --kind is a build card.
var Kinds = []string{"build", "research"}

// What each kind of card ends in, shown in the Obsidian template's kind picker.
var KindMeanings = map[string]string{
	"build":    "ends in a change to code, config or the running setup",
	"research": "ends in a document or a decision",
}

// The most cards active and in-review may hold.
const (
	ActiveLimit = 8
	ReviewLimit = 6
)

// The frontmatter fields in the order card writes them.
var Fields = []string{
	"tags",
	"type",
	"stage",
	"stage_date",
	"template_version",
	"project",
	"kind",
	"date_created",
	"plans",
	"prs",
	"sessions",
	"topics",
}

// The sections that rules name.
const (
	Criteria      = "Acceptance Criteria"
	OpenQuestions = "Open Questions"
	Decisions     = "Decisions"
)

// The version 2 sections in order.
var Sections = []string{
	"Context",
	Criteria,
	OpenQuestions,
	Decisions,
	"Notes",
	"Log",
	"Appendix",
}

// The longest title, in characters.
const TitleLimit = 50
