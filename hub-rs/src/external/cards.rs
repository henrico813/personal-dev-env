//! Calls the `card` command, which keeps kanban cards in the vault,
//! and parses its text. Read the `Cards` trait first. `card` has no
//! JSON output, so this is the only file that reads card text.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::LazyLock;
use std::thread;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// A whole card key. The card command allows digits in a project code,
/// such as `DEMO2-KB-001`.
static CARD_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z][A-Z0-9]*-KB-[0-9]{3,}$").unwrap());
/// A card key at the start of a session name, such as `demo-kb-007-impl`.
static KEY_IN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9]*-kb-[0-9]{3,}").unwrap());
/// A card key anywhere in free text, in any case, such as `demo-kb-007,`.
static KEY_IN_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b[A-Z][A-Z0-9]*-KB-[0-9]{3,}\b").unwrap());

/// The card sections whose bullets tell the AI model that sorts ready
/// cards what the work needs.
const CRITERIA_SECTIONS: [&str; 2] = ["Context", "Acceptance Criteria"];

/// The most `card show` calls `CardCli::read_cards` runs at once. Each
/// call starts Python and runs `nobs vault path`, and nobody has checked
/// how many of those the vault's helper takes at once.
const MAX_PARALLEL_SHOWS: usize = 8;

/// Calls the `card` command. `CardCli` runs the real one; tests use
/// `InMemoryCards` from `tests/common`.
pub trait Cards {
    /// Returns every card from one `card list` call, in its order.
    fn list_cards(&self) -> Result<Vec<ListedCard>, CardError>;
    /// Returns the keys of ready cards in `card list` order.
    fn list_ready(&self) -> Result<Vec<CardKey>, CardError>;
    /// Returns a card's title, project, session IDs, open needs-you note
    /// and criteria.
    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError>;
    /// Reads each card in `keys` and returns the results in the same
    /// order. This default reads one card at a time.
    fn read_cards(&self, keys: &[CardKey]) -> Vec<Result<CardNote, CardError>> {
        keys.iter().map(|key| self.read_card(key)).collect()
    }
    /// Returns the board's counts and their limits.
    fn read_board(&self) -> Result<BoardCounts, CardError>;
    /// Adds one line to the card's Log.
    fn add_note(&self, key: &CardKey, text: &str) -> Result<(), CardError>;
    /// Adds a live agent's session ID to the card's sessions list.
    /// `card` reads the ID from Herdr, so the agent must still run.
    fn link_session(&self, key: &CardKey, agent_name: &str) -> Result<(), CardError>;
    /// Moves a ready card to active. `card` refuses a start past the
    /// active limit, with its reason.
    fn move_to_active(&self, key: &CardKey) -> Result<(), CardError>;
}

/// Runs the `card` command.
#[derive(Clone)]
pub struct CardCli {
    program: PathBuf,
}

impl CardCli {
    /// Runs `program`, which `main` gives as `card` and adapter tests
    /// give as a script path, so no test changes `PATH`.
    pub fn new(program: PathBuf) -> CardCli {
        CardCli { program }
    }

    /// Runs `card` with `args` and returns what it printed.
    fn run_card(&self, args: &[&str]) -> Result<String, CardError> {
        let output =
            Command::new(&self.program)
                .args(args)
                .output()
                .map_err(|error| match error.kind() {
                    io::ErrorKind::NotFound => CardError::NotInstalled,
                    _ => CardError::Failed(format!("cannot run card: {error}")),
                })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(CardError::Failed(if stderr.is_empty() {
                format!("card ended with {}", output.status)
            } else {
                stderr
            }));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl Cards for CardCli {
    fn list_cards(&self) -> Result<Vec<ListedCard>, CardError> {
        let printed = self.run_card(&["list", "--tabs"])?;
        Ok(parse_card_list(&printed))
    }

    fn list_ready(&self) -> Result<Vec<CardKey>, CardError> {
        let printed = self.run_card(&["list", "--tabs", "--stage", "ready"])?;
        Ok(parse_card_list(&printed)
            .into_iter()
            .map(|card| card.key)
            .collect())
    }

    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError> {
        let printed = self.run_card(&["show", key.as_str()])?;
        Ok(parse_card_note(key, &printed))
    }

    /// Runs up to `MAX_PARALLEL_SHOWS` `card show` calls side by side,
    /// so 8 calls take about as long as one.
    fn read_cards(&self, keys: &[CardKey]) -> Vec<Result<CardNote, CardError>> {
        let mut notes = Vec::with_capacity(keys.len());
        for batch in keys.chunks(MAX_PARALLEL_SHOWS) {
            // Scoped threads may borrow `self` and the keys, since the
            // scope waits for every thread before it returns.
            thread::scope(|scope| {
                let readers: Vec<_> = batch
                    .iter()
                    .map(|key| scope.spawn(move || self.read_card(key)))
                    .collect();
                for reader in readers {
                    // A reader only panics on a bug, so pass the panic on.
                    let note = reader
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
                    notes.push(note);
                }
            });
        }
        notes
    }

    fn read_board(&self) -> Result<BoardCounts, CardError> {
        let printed = self.run_card(&["board"])?;
        parse_board_counts(&printed)
            .ok_or_else(|| CardError::Failed("card board printed no counts".to_owned()))
    }

    fn add_note(&self, key: &CardKey, text: &str) -> Result<(), CardError> {
        self.run_card(&["note", "--", key.as_str(), text]).map(drop)
    }

    fn link_session(&self, key: &CardKey, agent_name: &str) -> Result<(), CardError> {
        // Neither value is user text, and neither can start with a dash.
        self.run_card(&["link", key.as_str(), "--session", agent_name])
            .map(drop)
    }

    fn move_to_active(&self, key: &CardKey) -> Result<(), CardError> {
        self.run_card(&["move", key.as_str(), "active"]).map(drop)
    }
}

/// A card key in upper case, such as `DEMO-KB-007`. Reading one from
/// JSON checks its form, so a parked record cannot hold a bad key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CardKey(String);

impl CardKey {
    /// Returns the key a session name starts with, such as
    /// `DEMO-KB-007` for `demo-kb-007-impl`.
    pub(crate) fn find_in_name(name: &str) -> Option<CardKey> {
        KEY_IN_NAME
            .find(name)
            .map(|found| CardKey(found.as_str().to_uppercase()))
    }

    /// Returns every key named in free text, in any case, such as the
    /// user's own words approving an auto run.
    pub(crate) fn find_all_in_text(text: &str) -> Vec<CardKey> {
        KEY_IN_TEXT
            .find_iter(text)
            .map(|found| CardKey(found.as_str().to_uppercase()))
            .collect()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the key in lower case, as branch and folder names hold
    /// it.
    pub(crate) fn to_lowercase(&self) -> String {
        self.0.to_lowercase()
    }
}

/// Reads a key in any case, as `hub read --card demo-kb-007` gives it.
impl FromStr for CardKey {
    type Err = String;

    fn from_str(text: &str) -> Result<CardKey, String> {
        let upper = text.to_uppercase();
        if CARD_KEY.is_match(&upper) {
            Ok(CardKey(upper))
        } else {
            Err(format!("{text} is not a card key"))
        }
    }
}

/// Lets a parked record's `card` field be read as a checked key.
impl TryFrom<String> for CardKey {
    type Error = String;

    fn try_from(text: String) -> Result<CardKey, String> {
        text.parse()
    }
}

/// A card's stage, in board order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Draft,
    Ready,
    Active,
    Review,
    Accepted,
    Dropped,
}

impl Stage {
    /// Returns `card`'s word for the stage, such as `review`.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Stage::Draft => "draft",
            Stage::Ready => "ready",
            Stage::Active => "active",
            Stage::Review => "review",
            Stage::Accepted => "accepted",
            Stage::Dropped => "dropped",
        }
    }

    /// Returns the stage `card` names with `word`, or `None` for a word
    /// this hub does not know.
    fn from_word(word: &str) -> Option<Stage> {
        [
            Stage::Draft,
            Stage::Ready,
            Stage::Active,
            Stage::Review,
            Stage::Accepted,
            Stage::Dropped,
        ]
        .into_iter()
        .find(|stage| stage.as_str() == word)
    }
}

/// One line of `card list --tabs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedCard {
    pub key: CardKey,
    pub stage: Stage,
    /// The folder the card is in, such as `DevEnv`.
    pub project: String,
    pub title: String,
}

/// What `card show` gives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CardNote {
    /// The heading's words after the key, such as `Retry failed
    /// uploads`, or empty.
    pub title: String,
    /// The front matter's `project:`, or empty.
    pub project: String,
    /// Session IDs, oldest first.
    pub sessions: Vec<String>,
    /// A `needs-you` Log line comes after the last `answered` line, so
    /// the card waits on the user.
    pub needs_you_open: bool,
    /// The bullets of Context and Acceptance Criteria, in note order.
    pub criteria: Vec<String>,
    /// The front matter's plan notes, such as `DEMO-12 Retry uploads`.
    pub plans: Vec<String>,
    /// The front matter's pull request links.
    pub prs: Vec<String>,
    /// The newest Log line that starts `next: `, such as `review PR
    /// #498`, which the hub writes when it moves the card to review.
    pub next_step: Option<String>,
}

/// What `card board` gives. `board_over` and `board_under` events
/// keep these fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCounts {
    pub active: u32,
    pub review: u32,
    pub active_limit: u32,
    pub review_limit: u32,
}

impl BoardCounts {
    /// Returns true when active or review holds more cards than its
    /// limit.
    pub fn is_over(&self) -> bool {
        self.active > self.active_limit || self.review > self.review_limit
    }
}

/// Writes the counts as `card board` prints them, such as
/// `active 10 of 8, review 18 of 6`.
impl fmt::Display for BoardCounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "active {} of {}, review {} of {}",
            self.active, self.active_limit, self.review, self.review_limit
        )
    }
}

/// A card call that failed.
#[derive(Debug)]
pub enum CardError {
    NotInstalled,
    /// `card` exited nonzero, with its standard error, or printed text
    /// the hub cannot read.
    Failed(String),
}

/// Writes the message `main` prints after `hub: `.
impl fmt::Display for CardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardError::NotInstalled => formatter.write_str("the card command is not installed"),
            CardError::Failed(message) => formatter.write_str(message),
        }
    }
}

/// Lets `HubError::source` return it.
impl std::error::Error for CardError {}

/// Reads `card list --tabs` lines, which hold the key, stage, kind,
/// project and title split by tabs, skipping the kind. The last line,
/// `N cards`, holds no key.
fn parse_card_list(text: &str) -> Vec<ListedCard> {
    text.lines()
        .filter_map(|line| {
            let [key, stage, _kind, project, title] = line.splitn(5, '\t').collect::<Vec<_>>()[..]
            else {
                return None;
            };
            Some(ListedCard {
                key: key.parse().ok()?,
                stage: Stage::from_word(stage)?,
                project: project.to_owned(),
                title: title.to_owned(),
            })
        })
        .collect()
}

/// Reads `card show` text, whose title is the first `# KEY ...` heading
/// after the front matter. Its `sessions:` items look like
/// `- "opencode ses_0a1b2c3d4e5f"` with the ID last, its `plans:`
/// items like `- "[[DEMO-12 Retry uploads]]"`, and its log lines like
/// `- 2026-11-10 needs-you: which bucket?`.
fn parse_card_note(key: &CardKey, text: &str) -> CardNote {
    let mut note = CardNote::default();
    let mut lines = text.lines().skip(1);
    // The list a `- ` line belongs to, such as `sessions`.
    let mut list = "";
    // The front matter ends at the second `---` line.
    for line in lines.by_ref().take_while(|line| line.trim() != "---") {
        if let Some(item) = line.trim().strip_prefix("- ") {
            let item = item.trim_matches('"');
            match list {
                "sessions" => note
                    .sessions
                    .extend(item.split_whitespace().last().map(str::to_owned)),
                "plans" => note.plans.push(
                    item.trim_start_matches("[[")
                        .trim_end_matches("]]")
                        .to_owned(),
                ),
                "prs" => note.prs.push(item.to_owned()),
                _ => {}
            }
        } else {
            list = line.trim().strip_suffix(':').unwrap_or("");
            if let Some(project) = line.strip_prefix("project:") {
                note.project = project.trim().to_owned();
            }
        }
    }
    let heading = format!("# {}", key.as_str());
    let mut found_heading = false;
    let mut section = "";
    for line in lines {
        if let Some(name) = line.strip_prefix("## ") {
            section = name.trim();
        } else if let Some(words) = line.strip_prefix(&heading)
            && !found_heading
        {
            note.title = words.trim().to_owned();
            found_heading = true;
        } else if CRITERIA_SECTIONS.contains(&section) && line.starts_with("- ") {
            note.criteria.push(line.to_owned());
        }
        let log_entry = line
            .strip_prefix("- ")
            .and_then(|entry| entry.split_once(' '))
            .map(|(_, words)| words);
        if let Some(words) = log_entry {
            if words.starts_with("needs-you: ") {
                note.needs_you_open = true;
            } else if words.starts_with("answered: ") {
                note.needs_you_open = false;
            }
            // A `card note` line reads `note (who): next: ...`.
            let text = words.split_once("): ").map_or(words, |(_, text)| text);
            if section == "Log"
                && let Some(step) = text.strip_prefix("next: ")
            {
                note.next_step = Some(step.trim().to_owned());
            }
        }
    }
    note
}

/// Reads `card board` text. Returns `None` when the text lacks its
/// `active N of M, review N of M` line.
fn parse_board_counts(text: &str) -> Option<BoardCounts> {
    text.lines().find_map(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        let [
            "active",
            active,
            "of",
            active_limit,
            "review",
            review,
            "of",
            review_limit,
        ] = words[..]
        else {
            return None;
        };
        Some(BoardCounts {
            active: active.parse().ok()?,
            review: review.parse().ok()?,
            active_limit: active_limit.trim_end_matches(',').parse().ok()?,
            review_limit: review_limit.parse().ok()?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `hub brief` and `hub status` print the board line from these
    /// counts, and the user decides from it whether to review before
    /// starting more. A limit or review count read from the wrong word
    /// misleads the user, and `hub status --json` also gives the active
    /// count to scripts.
    #[test]
    fn parse_board_counts_reads_counts_or_none() {
        let board = "draft 79  ready 7  active 9\n\
                     review 24  accepted 43  dropped 4\n\
                     active 9 of 8, review 24 of 6\n\
                     \n\
                     REVIEW, oldest first (24)\n\
                     - DEMO-KB-001 2026-11-02\n  \
                       Fix the docs\n";
        let cases = [
            (
                "board",
                board,
                Some(BoardCounts {
                    active: 9,
                    review: 24,
                    active_limit: 8,
                    review_limit: 6,
                }),
            ),
            ("no-counts", "card: no default vault\n", None),
        ];

        for (case_name, text, expected) in cases {
            assert_eq!(parse_board_counts(text), expected, "{case_name}");
        }
    }

    /// The AI model sorts ready cards from `hub auto ready`'s text
    /// alone. A title, project or criterion read wrong, or a Log line
    /// read as a criterion, sorts the card on wrong facts.
    #[test]
    fn parse_card_note_reads_title_project_criteria() {
        let card_show = "---\n\
                         stage: ready\n\
                         project: Demo\n\
                         topics: []\n\
                         ---\n\
                         \n\
                         # DEMO-KB-001 Retry failed uploads\n\
                         ---\n\
                         \n\
                         ## Context\n\
                         ---\n\
                         \n\
                         - Uploads fail on a slow link.\n\
                         \n\
                         ## Acceptance Criteria\n\
                         ---\n\
                         \n\
                         - [ ] A failed upload retries.\n\
                         \n\
                         ## Log\n\
                         ---\n\
                         \n\
                         - 2026-11-10 ready: by the user\n";

        let note = parse_card_note(&"DEMO-KB-001".parse().unwrap(), card_show);

        assert_eq!(
            note,
            CardNote {
                title: "Retry failed uploads".to_owned(),
                project: "Demo".to_owned(),
                sessions: Vec::new(),
                needs_you_open: false,
                criteria: vec![
                    "- Uploads fail on a slow link.".to_owned(),
                    "- [ ] A failed upload retries.".to_owned(),
                ],
                ..CardNote::default()
            }
        );
    }

    /// `hub board` tells the user what to do next on each card in
    /// review from these links and Log lines. A link read from the
    /// wrong list, or an older `next:` line winning, sends the user to
    /// the wrong plan or PR.
    #[test]
    fn parse_card_note_reads_plans_prs_next() {
        let card_show = "---\n\
                         plans:\n  \
                           - \"[[DEMO-12 Retry failed uploads]]\"\n\
                         prs:\n  \
                           - \"https://github.com/demo/app/pull/498\"\n\
                         sessions:\n  \
                           - \"opencode ses_1\"\n\
                         ---\n\
                         \n\
                         # DEMO-KB-001 Retry failed uploads\n\
                         ---\n\
                         \n\
                         ## Log\n\
                         ---\n\
                         \n\
                         - 2026-11-10 note (hub): next: read plan DEMO-12\n\
                         - 2026-11-11 note (hub): next: review PR #498\n";

        let note = parse_card_note(&"DEMO-KB-001".parse().unwrap(), card_show);

        assert_eq!(note.plans, ["DEMO-12 Retry failed uploads"]);
        assert_eq!(note.prs, ["https://github.com/demo/app/pull/498"]);
        assert_eq!(note.next_step.as_deref(), Some("review PR #498"));
    }

    /// Every session's lifecycle starts from its card's stage, read
    /// from one `card list` line, and `hub board` shows the title and
    /// project on the card's row. A stage read from the wrong field, or
    /// the count line read as a card, gives sessions the wrong state,
    /// such as `to-close` for a worker whose card is still active. A
    /// card with no kind must not shift its project into the title.
    #[test]
    fn parse_card_list_reads_tab_separated_fields() {
        let listing = "DEMO-KB-004\tactive\t\tDemo\tFix the docs\n\
                       DEMO-KB-007\taccepted\ttask\tOps\tRetry failed uploads, then warn\n\
                       2 cards\n";

        let cards = parse_card_list(listing);

        let card = |key: &str, stage, project: &str, title: &str| ListedCard {
            key: key.parse().unwrap(),
            stage,
            project: project.to_owned(),
            title: title.to_owned(),
        };
        assert_eq!(
            cards,
            [
                card("DEMO-KB-004", Stage::Active, "Demo", "Fix the docs"),
                card(
                    "DEMO-KB-007",
                    Stage::Accepted,
                    "Ops",
                    "Retry failed uploads, then warn"
                ),
            ]
        );
    }

    /// A card whose last needs-you line has no answer after it waits
    /// on the user, so `hub status` shows its session as `you`. Reading
    /// an answered question as open hides a finished worker behind
    /// `you`, and missing a new question after an answer leaves the
    /// user's question unseen.
    #[test]
    fn parse_card_note_finds_open_question() {
        struct Case {
            name: &'static str,
            log: &'static str,
            open: bool,
        }
        let cases = [
            Case {
                name: "asked",
                log: "- 2026-11-10 needs-you: which bucket?\n",
                open: true,
            },
            Case {
                name: "answered",
                log: "- 2026-11-10 needs-you: which bucket?\n\
                      - 2026-11-10 answered: the cold one\n",
                open: false,
            },
            Case {
                name: "asked-again",
                log: "- 2026-11-10 needs-you: which bucket?\n\
                      - 2026-11-10 answered: the cold one\n\
                      - 2026-11-11 needs-you: which region?\n",
                open: true,
            },
            Case {
                name: "no-question",
                log: "- 2026-11-10 active: by the hub\n",
                open: false,
            },
        ];

        for case in cases {
            let card_show = format!("---\nstage: active\n---\n\n## Log\n---\n\n{}", case.log);

            let note = parse_card_note(&"DEMO-KB-007".parse().unwrap(), &card_show);

            assert_eq!(note.needs_you_open, case.open, "{}", case.name);
        }
    }
}
