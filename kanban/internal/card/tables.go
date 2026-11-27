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

// One allowed move and who may make it, "staff" or "user".
type Move struct{ From, To, Who string }

// Every allowed move, in the Python card's order. card move takes the
// shortest chain of these to a target stage.
var Moves = []Move{
	{From: Draft, To: Todo, Who: "staff"},
	{From: Todo, To: Draft, Who: "staff"},
	{From: Todo, To: Active, Who: "staff"},
	{From: Active, To: Todo, Who: "staff"},
	{From: Active, To: Draft, Who: "user"},
	{From: Active, To: InReview, Who: "staff"},
	{From: InReview, To: Active, Who: "user"},
	{From: InReview, To: Done, Who: "user"},
	// A blocked card waits on something outside card's control, so it never
	// counts against a card limit.
	{From: Active, To: Blocked, Who: "staff"},
	{From: InReview, To: Blocked, Who: "staff"},
	{From: Blocked, To: InReview, Who: "staff"},
	{From: Blocked, To: Active, Who: "staff"},
	{From: Blocked, To: Done, Who: "user"},
	{From: Dropped, To: Draft, Who: "user"},
	{From: Draft, To: Dropped, Who: "user"},
	{From: Todo, To: Dropped, Who: "user"},
	{From: Active, To: Dropped, Who: "user"},
	{From: InReview, To: Dropped, Who: "user"},
	{From: Blocked, To: Dropped, Who: "user"},
}

// The stages a chain of moves may pass through. Blocked needs a reason
// and dropped is a decision, so a chain only ends in those.
var ChainStages = []string{Draft, Todo, Active, InReview}

// The stages only the user may start a card in, with card new --adopt.
var AdoptStages = []string{Active, InReview, Blocked}

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

// The sections card add and card edit may change, and card new --from may
// fill. Notes is the user's text, Appendix holds agent notes, and only
// card writes Log.
var WritableSections = []string{"Context", Criteria, OpenQuestions, Decisions}

// The longest title, in characters.
const TitleLimit = 50
