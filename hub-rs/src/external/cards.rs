//! Calls the `card` command, which keeps kanban cards in the vault,
//! and reads what it prints. Read the `Cards` trait first. This is the
//! only file that reads card output.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::str::FromStr;
use std::sync::LazyLock;
use std::thread;

use jiff::civil::Date;
use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// A card key in either shape, such as `DEMO-KB-007` or `DEMO-516`.
/// A new key counts only when its code has exactly four characters, so
/// words such as `ISO-8601` or `merge-553` are not keys. An old key's
/// code may have any length and hold digits, such as `DEMO2-KB-001`.
pub(crate) const KEY_PATTERN: &str = "[A-Z][A-Z0-9]*-KB-[0-9]{3,}|[A-Z][A-Z0-9]{3}-[0-9]{3,}";
/// A whole card key.
static CARD_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^(?:{KEY_PATTERN})$")).unwrap());
/// A card key at the start of a session name, such as `demo-516-impl`.
static KEY_IN_NAME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^(?i:{KEY_PATTERN})")).unwrap());
/// A card key anywhere in free text, in any case, such as `demo-516,`.
static KEY_IN_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"(?i)\b(?:{KEY_PATTERN})\b")).unwrap());

/// The card sections whose bullets tell the AI model that sorts
/// cards ready to start what the work needs.
const CRITERIA_SECTIONS: [&str; 2] = ["Context", "Acceptance Criteria"];

/// The card section that holds the questions waiting on the user.
pub(crate) const OPEN_QUESTIONS: &str = "Open Questions";

/// The most `card show` calls `CardCli::read_cards` runs at once. Each
/// call starts Python and runs `nobs vault path`, and nobody has checked
/// how many of those the vault's helper takes at once.
const MAX_PARALLEL_SHOWS: usize = 8;

/// Calls the `card` command. `CardCli` runs the real one; tests use
/// `InMemoryCards` from `tests/common`.
pub trait Cards {
    /// Returns every card from one `card list --json` call, in its
    /// order.
    fn list_cards(&self) -> Result<Vec<ListedCard>, CardError>;
    /// Returns the keys of cards ready to start, in `card list` order.
    fn list_todo(&self) -> Result<Vec<CardKey>, CardError>;
    /// Returns a card's title, project, stage, session IDs, open
    /// questions and criteria.
    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError>;
    /// Reads each card in `keys` and returns the results in the same
    /// order. This default reads one card at a time.
    fn read_cards(&self, keys: &[CardKey]) -> Vec<Result<CardNote, CardError>> {
        keys.iter().map(|key| self.read_card(key)).collect()
    }
    /// Returns the board's counts, their limits, and the cards waiting
    /// on people outside the team.
    fn read_board(&self) -> Result<Board, CardError>;
    /// Adds one line to the card's Log.
    fn add_note(&self, key: &CardKey, text: &str) -> Result<(), CardError>;
    /// Adds a live agent's session ID to the card's sessions list.
    /// `card` reads the ID from Herdr, so the agent must still run.
    fn link_session(&self, key: &CardKey, agent_name: &str) -> Result<(), CardError>;
    /// Moves a card ready to start into progress. `card` refuses a
    /// start past the work limit, with its reason.
    fn move_to_active(&self, key: &CardKey) -> Result<(), CardError>;
    /// Adds `text` as a new item at the end of a section; in Open Questions
    /// `card` puts a Q number and today's date in front. Each later line of
    /// `text` must start with two spaces, or `card` reads it as an item of its own.
    fn add_item(&self, key: &CardKey, section: &str, text: &str) -> Result<(), CardError>;
    /// Removes the Open Questions item numbered `Q<number>`. `card`
    /// logs the removal by `hub`, so no Herdr pane is needed.
    fn remove_question(&self, key: &CardKey, number: u32) -> Result<(), CardError>;
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
        read_card_output(self.run_card_unchecked(args)?)
    }

    /// Runs `card` with `args` and returns its output, whatever its
    /// exit code.
    fn run_card_unchecked(&self, args: &[&str]) -> Result<Output, CardError> {
        Command::new(&self.program)
            .args(args)
            .output()
            .map_err(|error| match error.kind() {
                io::ErrorKind::NotFound => CardError::NotInstalled,
                _ => CardError::Failed(format!("cannot run card: {error}")),
            })
    }
}

/// Returns what `card` printed, or its standard error as the failure.
fn read_card_output(output: Output) -> Result<String, CardError> {
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

impl Cards for CardCli {
    fn list_cards(&self) -> Result<Vec<ListedCard>, CardError> {
        let printed = self.run_card(&["list", "--json"])?;
        parse_card_list(&printed)
    }

    fn list_todo(&self) -> Result<Vec<CardKey>, CardError> {
        let printed = self.run_card(&["list", "--json", "--stage", Stage::Todo.as_str()])?;
        Ok(parse_card_list(&printed)?
            .into_iter()
            .map(|card| card.key)
            .collect())
    }

    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError> {
        let printed = self.run_card(&["show", key.as_str(), "--json"])?;
        parse_card_note(&printed)
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

    fn read_board(&self) -> Result<Board, CardError> {
        let printed = self.run_card(&["board"])?;
        let counts = parse_board_counts(&printed)
            .ok_or_else(|| CardError::Failed("card board printed no counts".to_owned()))?;
        Ok(Board {
            counts,
            blocked: parse_blocked_cards(&printed),
        })
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
        self.run_card(&["move", key.as_str(), Stage::Active.as_str()])
            .map(drop)
    }

    fn add_item(&self, key: &CardKey, section: &str, text: &str) -> Result<(), CardError> {
        self.run_card(&["add", "--", key.as_str(), section, text])
            .map(drop)
    }

    fn remove_question(&self, key: &CardKey, number: u32) -> Result<(), CardError> {
        let item = format!("Q{number}");
        self.run_card(&[
            "edit",
            "--remove",
            "--by",
            "hub",
            "--",
            key.as_str(),
            OPEN_QUESTIONS,
            &item,
        ])
        .map(drop)
    }
}

/// A card key in upper case, such as `DEMO-516` or `DEMO-KB-007`.
/// Reading one from JSON checks its form, so a parked record cannot
/// hold a bad key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct CardKey(String);

impl CardKey {
    /// Returns the key a session name starts with, such as
    /// `DEMO-516` for `demo-516-impl`.
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

/// Reads a key in any case, as a user may type it, such as
/// `demo-kb-007`.
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
    Todo,
    Active,
    InReview,
    /// Waits only on people outside the team, such as another team's
    /// reviewers. No limit counts it.
    Blocked,
    Done,
    Dropped,
}

impl Stage {
    /// Returns `card`'s word for the stage, such as `in-review`. The hub
    /// passes stage words to `card` and reads them from it only through
    /// this table. Board titles, briefing headings and JSON keys keep
    /// their own spelling.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Stage::Draft => "draft",
            Stage::Todo => "todo",
            Stage::Active => "active",
            Stage::InReview => "in-review",
            Stage::Blocked => "blocked",
            Stage::Done => "done",
            Stage::Dropped => "dropped",
        }
    }

    /// Returns the stage `card` names with `word`, or `None` for a word
    /// this hub does not know.
    fn from_word(word: &str) -> Option<Stage> {
        [
            Stage::Draft,
            Stage::Todo,
            Stage::Active,
            Stage::InReview,
            Stage::Blocked,
            Stage::Done,
            Stage::Dropped,
        ]
        .into_iter()
        .find(|stage| stage.as_str() == word)
    }
}

/// One card of `card list --json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedCard {
    pub key: CardKey,
    pub stage: Stage,
    /// The folder the card is in, such as `DevEnv`.
    pub project: String,
    pub title: String,
    /// Session IDs, oldest first.
    pub sessions: Vec<String>,
    /// The Open Questions items, in note order.
    pub open_questions: Vec<OpenItem>,
}

/// One Open Questions item, as `card` numbers and dates it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OpenItem {
    /// The item's Q number, which stays until the item is answered.
    pub number: u32,
    /// The day it was asked, or `None` for an item typed by hand.
    pub asked: Option<Date>,
    /// The item's first line, after its `Q4 asked 2026-11-25: ` head.
    pub text: String,
    /// The later lines, with their two-space indent taken off.
    pub lines: Vec<String>,
}

/// What `card show --json` gives.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CardNote {
    /// The card's title, such as `Retry failed uploads`.
    pub title: String,
    /// The folder the card is in, such as `DevEnv`.
    pub project: String,
    /// The card's stage, or `None` for a stage word this hub does not
    /// know.
    pub stage: Option<Stage>,
    /// Session IDs, oldest first.
    pub sessions: Vec<String>,
    /// The Log's `session: NAME (kind ID)` lines, oldest first, which
    /// `card link --session` writes for each new session.
    pub logged_sessions: Vec<LoggedSession>,
    /// The Open Questions items, in note order.
    pub open_questions: Vec<OpenItem>,
    /// The bullets of Context and Acceptance Criteria, in note order.
    pub criteria: Vec<String>,
    /// The front matter's plan notes, such as `DEMO-12 Retry uploads`.
    pub plans: Vec<String>,
    /// The front matter's pull request links.
    pub prs: Vec<String>,
    /// The newest Log line that starts `next: `, such as `review PR
    /// #498`, which the hub writes when it hands the card to the user
    /// for review.
    pub next_step: Option<String>,
}

/// One Log line `session: NAME (kind ID)`, a session Herdr showed for
/// the agent named NAME when `card link` ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoggedSession {
    pub name: String,
    pub session_id: String,
}

/// What `card board` gives. `board_over` and `board_under` events
/// keep these fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardCounts {
    pub active: u32,
    pub in_review: u32,
    pub active_limit: u32,
    pub in_review_limit: u32,
}

impl BoardCounts {
    /// Returns true when a limited stage holds more cards than its
    /// limit.
    pub fn is_over(&self) -> bool {
        self.active > self.active_limit || self.in_review > self.in_review_limit
    }
}

/// Writes the counts as `card board` prints them, such as
/// `active 10 of 8, in-review 18 of 6`.
impl fmt::Display for BoardCounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} of {}, {} {} of {}",
            Stage::Active.as_str(),
            self.active,
            self.active_limit,
            Stage::InReview.as_str(),
            self.in_review,
            self.in_review_limit
        )
    }
}

/// What `card board` gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    pub counts: BoardCounts,
    /// The cards waiting on people outside the team, oldest first.
    pub blocked: Vec<BlockedCard>,
}

impl Board {
    /// Returns a line such as `blocked 2, oldest 8 days`, or `None`
    /// when the list is empty.
    pub fn format_blocked_line(&self) -> Option<String> {
        let oldest = self.blocked.iter().map(|card| card.days).max()?;
        Some(format!(
            "{} {}, oldest {}",
            Stage::Blocked.as_str(),
            self.blocked.len(),
            format_days(oldest)
        ))
    }
}

/// A card from the `card board` list that waits on people outside
/// the team.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockedCard {
    pub key: CardKey,
    /// Whole days since the card moved to the stage.
    pub days: u32,
    pub title: String,
    /// Who the card waits on, from the move's `--why`, or empty.
    pub reason: String,
}

/// Returns `1 day` or `N days`.
pub fn format_days(days: u32) -> String {
    match days {
        1 => "1 day".to_owned(),
        _ => format!("{days} days"),
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

/// One card as `card list --json` or `card show --json` prints it, with
/// `sections` from `card show` only. A field `card` leaves out reads as empty.
#[derive(Default, Deserialize)]
#[serde(default)]
struct CardJson {
    key: String,
    title: String,
    stage: String,
    project: String,
    /// Plan links, such as `[[DEMO-12 Retry uploads]]`.
    plans: Vec<String>,
    prs: Vec<String>,
    /// Entries such as `opencode ses_0a1b2c3d4e5f`, with the ID last.
    sessions: Vec<String>,
    open_questions: Vec<OpenItem>,
    sections: BTreeMap<String, String>,
}

/// Reads `card list --json`. A card whose key or stage this hub does
/// not know is left out.
fn parse_card_list(text: &str) -> Result<Vec<ListedCard>, CardError> {
    let cards: Vec<CardJson> = parse_json(text)?;
    Ok(cards
        .into_iter()
        .filter_map(|card| {
            Some(ListedCard {
                key: card.key.parse().ok()?,
                stage: Stage::from_word(&card.stage)?,
                project: card.project,
                title: card.title,
                sessions: read_session_ids(&card.sessions),
                open_questions: card.open_questions,
            })
        })
        .collect())
}

/// Reads `card show --json`, taking criteria from the `- ` lines of Context
/// and Acceptance Criteria. The Log gives the logged sessions and the
/// newest `next: ` line, such as `- 2026-11-11 note (hub): next: review PR #498`.
fn parse_card_note(text: &str) -> Result<CardNote, CardError> {
    let card: CardJson = parse_json(text)?;
    let mut criteria = Vec::new();
    for name in CRITERIA_SECTIONS {
        let section = card.sections.get(name).map_or("", String::as_str);
        criteria.extend(
            section
                .lines()
                .filter(|line| line.starts_with("- "))
                .map(str::to_owned),
        );
    }
    let mut logged_sessions = Vec::new();
    let mut next_step = None;
    let log = card.sections.get("Log").map_or("", String::as_str);
    for line in log.lines() {
        let Some((_, words)) = line
            .strip_prefix("- ")
            .and_then(|entry| entry.split_once(' '))
        else {
            continue;
        };
        // A `card note` line reads `note (who): next: ...`.
        let text = words.split_once("): ").map_or(words, |(_, text)| text);
        if let Some(step) = text.strip_prefix("next: ") {
            next_step = Some(step.trim().to_owned());
        }
        logged_sessions.extend(parse_logged_session(words));
    }
    Ok(CardNote {
        stage: Stage::from_word(&card.stage),
        sessions: read_session_ids(&card.sessions),
        plans: card
            .plans
            .iter()
            .map(|plan| {
                plan.trim_start_matches("[[")
                    .trim_end_matches("]]")
                    .to_owned()
            })
            .collect(),
        title: card.title,
        project: card.project,
        prs: card.prs,
        open_questions: card.open_questions,
        logged_sessions,
        criteria,
        next_step,
    })
}

/// Reads JSON that `card` printed, or fails with the reason.
fn parse_json<T: DeserializeOwned>(text: &str) -> Result<T, CardError> {
    serde_json::from_str(text).map_err(|error| {
        CardError::Failed(format!("card printed JSON the hub cannot read: {error}"))
    })
}

/// Returns the ID in each `sessions` entry, such as `ses_1` from
/// `opencode ses_1`.
fn read_session_ids(entries: &[String]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|entry| entry.split_whitespace().last())
        .map(str::to_owned)
        .collect()
}

/// Reads a Log line's words after the date, such as
/// `session: demo-kb-007-impl (opencode ses_1)`, where the ID is the
/// last word in the parentheses.
fn parse_logged_session(words: &str) -> Option<LoggedSession> {
    let (name, kind_and_id) = words.strip_prefix("session: ")?.split_once(" (")?;
    let session_id = kind_and_id
        .trim_end()
        .strip_suffix(')')?
        .split_whitespace()
        .last()?;
    Some(LoggedSession {
        name: name.to_owned(),
        session_id: session_id.to_owned(),
    })
}

/// Reads `card board` text. Returns `None` when the text lacks its
/// `active N of M, in-review N of M` line.
fn parse_board_counts(text: &str) -> Option<BoardCounts> {
    text.lines().find_map(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        let [
            active_word,
            active,
            "of",
            active_limit,
            in_review_word,
            in_review,
            "of",
            in_review_limit,
        ] = words[..]
        else {
            return None;
        };
        if Stage::from_word(active_word) != Some(Stage::Active)
            || Stage::from_word(in_review_word) != Some(Stage::InReview)
        {
            return None;
        }
        Some(BoardCounts {
            active: active.parse().ok()?,
            in_review: in_review.parse().ok()?,
            active_limit: active_limit.trim_end_matches(',').parse().ok()?,
            in_review_limit: in_review_limit.parse().ok()?,
        })
    })
}

/// Reads the `card board` list for the stage that waits on people
/// outside the team. Each card is a `- KEY N days` line, then its
/// title and its reason, each indented two spaces. A board without
/// the list gives none.
fn parse_blocked_cards(text: &str) -> Vec<BlockedCard> {
    let mut lines = text
        .lines()
        .skip_while(|line| !is_list_heading(line, Stage::Blocked))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .peekable();
    let mut cards = Vec::new();
    while let Some(line) = lines.next() {
        let Some(entry) = line.strip_prefix("- ") else {
            continue;
        };
        let mut words = entry.split_whitespace();
        let key = words.next().and_then(|word| word.parse().ok());
        let days = words.next().and_then(|word| word.parse().ok());
        let mut details = Vec::new();
        while let Some(detail) = lines.next_if(|next| next.starts_with("  ")) {
            details.push(detail.trim().to_owned());
        }
        let (Some(key), Some(days)) = (key, days) else {
            continue;
        };
        let mut details = details.into_iter();
        cards.push(BlockedCard {
            key,
            days,
            title: details.next().unwrap_or_default(),
            reason: details.next().unwrap_or_default(),
        });
    }
    cards
}

/// Returns true for the line that heads a `card board` list of cards
/// in `stage`, such as `IN-REVIEW, oldest first (2)`.
fn is_list_heading(line: &str, stage: Stage) -> bool {
    let heading = line.split(',').next().unwrap_or_default();
    Stage::from_word(&heading.to_lowercase()) == Some(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hub finds a worker's card from its name to launch, message
    /// and report on it. Missing `demo-516-impl` cuts the worker off from
    /// its card, and reading `merge-553` as a card makes `hub launch`
    /// refuse it.
    #[test]
    fn find_in_name_reads_only_card_keys() {
        let cases = [
            ("demo-516-impl", Some("DEMO-516")),
            ("demo-kb-007-impl", Some("DEMO-KB-007")),
            ("merge-553", None),
        ];

        for (name, key) in cases {
            let found = CardKey::find_in_name(name);

            assert_eq!(found.as_ref().map(CardKey::as_str), key, "{name}");
        }
    }

    /// The hub reads card keys from the user's own approval words. A
    /// name such as `ISO-8601` or `Windows-1252` read as a key names a
    /// card the run lacks, which blocks the approval.
    #[test]
    fn find_all_in_text_skips_non_key_words() {
        let text = "Run demo-516 per ISO-8601, RFC-2119 and Windows-1252.";

        let found = CardKey::find_all_in_text(text);

        let keys: Vec<&str> = found.iter().map(CardKey::as_str).collect();
        assert_eq!(keys, ["DEMO-516"]);
    }

    /// `run_status` and `run_brief` show the days and reason of each
    /// card waiting on people outside the team from this list, and an
    /// empty list prints none. A card read wrong tells the user the
    /// wrong team to chase.
    #[test]
    fn parse_blocked_cards_reads_list_or_none() {
        let board = "active 3 of 8, in-review 2 of 6\n\
                     blocked 2, oldest 8 days\n\
                     \n\
                     IN-REVIEW, oldest first (2)\n\
                     - DEMO-KB-001 2026-11-02\n  \
                       Fix the docs\n\
                     \n\
                     BLOCKED, oldest first (2)\n\
                     - DEMO-KB-003 8 days\n  \
                       Retry failed uploads\n  \
                       platform team's review\n\
                     - DEMO-KB-009 1 day\n  \
                       Add a cache\n";
        let no_blocked = "active 3 of 8, in-review 2 of 6\n\
                          \n\
                          IN-REVIEW, oldest first (2)\n\
                          - DEMO-KB-001 2026-11-02\n  \
                            Fix the docs\n";

        let blocked = parse_blocked_cards(board);

        assert_eq!(
            blocked,
            [
                BlockedCard {
                    key: "DEMO-KB-003".parse().unwrap(),
                    days: 8,
                    title: "Retry failed uploads".to_owned(),
                    reason: "platform team's review".to_owned(),
                },
                BlockedCard {
                    key: "DEMO-KB-009".parse().unwrap(),
                    days: 1,
                    title: "Add a cache".to_owned(),
                    reason: String::new(),
                },
            ]
        );
        assert_eq!(parse_blocked_cards(no_blocked), []);
    }

    /// `run_brief` and `run_status` print the board line from these
    /// counts, and the user decides from it whether to review before
    /// starting more. A limit or count read from the wrong word
    /// misleads the user, and the status JSON also gives these counts
    /// to scripts.
    #[test]
    fn parse_board_counts_reads_counts_or_none() {
        let board = "draft 79  todo 7  active 9\n\
                     in-review 24  blocked 0\n\
                     done 43  dropped 4\n\
                     active 9 of 8, in-review 24 of 6\n\
                     \n\
                     IN-REVIEW, oldest first (24)\n\
                     - DEMO-KB-001 2026-11-02\n  \
                       Fix the docs\n";
        let cases = [
            (
                "board",
                board,
                Some(BoardCounts {
                    active: 9,
                    in_review: 24,
                    active_limit: 8,
                    in_review_limit: 6,
                }),
            ),
            ("no-counts", "card: no default vault\n", None),
        ];

        for (case_name, text, expected) in cases {
            assert_eq!(parse_board_counts(text), expected, "{case_name}");
        }
    }

    /// The AI model sorts cards ready to start from `run_auto_todo`'s
    /// text alone. A title, project or criterion read wrong, or a Log
    /// line read as a criterion, sorts the card on wrong facts.
    #[test]
    fn parse_card_note_reads_title_project_criteria() {
        let card_show = r#"{
            "key": "DEMO-KB-001",
            "title": "Retry failed uploads",
            "stage": "todo",
            "project": "Demo",
            "sections": {
                "Context": "---\n\n- Uploads fail on a slow link.",
                "Acceptance Criteria": "---\n\n- [ ] A failed upload retries.",
                "Log": "---\n\n- 2026-11-10 todo: by the user"
            }
        }"#;

        let note = parse_card_note(card_show).unwrap();

        assert_eq!(
            note,
            CardNote {
                title: "Retry failed uploads".to_owned(),
                project: "Demo".to_owned(),
                stage: Some(Stage::Todo),
                criteria: vec![
                    "- Uploads fail on a slow link.".to_owned(),
                    "- [ ] A failed upload retries.".to_owned(),
                ],
                ..CardNote::default()
            }
        );
    }

    /// The board tells the user what to do next on each card needing
    /// the user's review, from these links and Log lines. A link read
    /// from the wrong list, or an older `next:` line winning, sends the
    /// user to the wrong plan or PR.
    #[test]
    fn parse_card_note_reads_plans_prs_next() {
        let card_show = r#"{
            "plans": ["[[DEMO-12 Retry failed uploads]]"],
            "prs": ["https://github.com/demo/app/pull/498"],
            "sections": {
                "Log": "- 2026-11-10 note (hub): next: read plan DEMO-12\n- 2026-11-11 note (hub): next: review PR #498"
            }
        }"#;

        let note = parse_card_note(card_show).unwrap();

        assert_eq!(note.plans, ["DEMO-12 Retry failed uploads"]);
        assert_eq!(note.prs, ["https://github.com/demo/app/pull/498"]);
        assert_eq!(note.next_step.as_deref(), Some("review PR #498"));
    }

    /// Park saves the session an agent was launched as, read from these
    /// lines, when Herdr shows a nested run's session instead. A wrong
    /// name or ID parks the wrong session, and a reopen resumes it.
    #[test]
    fn parse_card_note_reads_logged_sessions() {
        let card_show = r#"{
            "sections": {
                "Log": "- 2026-11-10 session: demo-kb-001-impl (opencode ses_1)\n- 2026-11-11 note (hub): parked demo-kb-001-impl\n- 2026-11-12 session: demo-kb-001-fix (opencode ses_2)"
            }
        }"#;

        let note = parse_card_note(card_show).unwrap();

        let logged: Vec<(&str, &str)> = note
            .logged_sessions
            .iter()
            .map(|logged| (logged.name.as_str(), logged.session_id.as_str()))
            .collect();
        assert_eq!(
            logged,
            [("demo-kb-001-impl", "ses_1"), ("demo-kb-001-fix", "ses_2")]
        );
    }

    /// Every session's lifecycle starts from its card's stage, and the
    /// hub lists each Open Questions item from this one call. A stage
    /// read wrong gives sessions the wrong state, and an item's number
    /// or date read wrong sends the user's answer to another question.
    /// A card in a stage this hub does not know must not fail the list.
    #[test]
    fn parse_card_list_reads_stages_and_questions() {
        let card_list = r#"[
            {
                "key": "DEMO-KB-004",
                "stage": "active",
                "project": "Demo",
                "title": "Fix the docs",
                "sessions": ["opencode ses_1"],
                "open_questions": [
                    {"number": 3, "asked": "2026-11-25", "text": "Deploy tonight?", "lines": ["- A. Yes"]},
                    {"number": 4, "asked": null, "text": "Which bucket?", "lines": []}
                ]
            },
            {"key": "DEMO-KB-007", "stage": "parked", "project": "Ops", "title": "Retry"}
        ]"#;

        let cards = parse_card_list(card_list).unwrap();

        assert_eq!(
            cards,
            [ListedCard {
                key: "DEMO-KB-004".parse().unwrap(),
                stage: Stage::Active,
                project: "Demo".to_owned(),
                title: "Fix the docs".to_owned(),
                sessions: vec!["ses_1".to_owned()],
                open_questions: vec![
                    OpenItem {
                        number: 3,
                        asked: Some("2026-11-25".parse().unwrap()),
                        text: "Deploy tonight?".to_owned(),
                        lines: vec!["- A. Yes".to_owned()],
                    },
                    OpenItem {
                        number: 4,
                        asked: None,
                        text: "Which bucket?".to_owned(),
                        lines: Vec::new(),
                    },
                ],
            }]
        );
    }
}
