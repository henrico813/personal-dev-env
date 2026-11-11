//! Calls the `card` command, which keeps kanban cards in the vault,
//! and parses its text. Read the `Cards` trait first. `card` has no
//! JSON output, so this is the only file that reads card text.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::LazyLock;

use regex::Regex;

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

/// Calls the `card` command. `CardCli` runs the real one; tests use
/// `InMemoryCards` from `tests/common`.
pub trait Cards {
    /// Returns the keys of ready cards in `card list` order.
    fn list_ready(&self) -> Result<Vec<CardKey>, CardError>;
    /// Returns a card's title, project, session IDs and criteria.
    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError>;
    /// Returns the board's counts and its limit.
    fn read_board(&self) -> Result<BoardCounts, CardError>;
    /// Adds one line to the card's Log.
    fn add_note(&self, key: &CardKey, text: &str) -> Result<(), CardError>;
}

/// Runs the `card` command.
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
    fn list_ready(&self) -> Result<Vec<CardKey>, CardError> {
        let printed = self.run_card(&["list", "--stage", "ready"])?;
        Ok(parse_card_list(&printed))
    }

    fn read_card(&self, key: &CardKey) -> Result<CardNote, CardError> {
        let printed = self.run_card(&["show", key.as_str()])?;
        Ok(parse_card_note(key, &printed))
    }

    fn read_board(&self) -> Result<BoardCounts, CardError> {
        let printed = self.run_card(&["board"])?;
        parse_board_counts(&printed)
            .ok_or_else(|| CardError::Failed("card board printed no counts".to_owned()))
    }

    fn add_note(&self, key: &CardKey, text: &str) -> Result<(), CardError> {
        self.run_card(&["note", "--", key.as_str(), text]).map(drop)
    }
}

/// A card key in upper case, such as `DEMO-KB-007`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
    /// The bullets of Context and Acceptance Criteria, in note order.
    pub criteria: Vec<String>,
}

/// What `card board` gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardCounts {
    pub review: u32,
    /// Active plus review, from `active+review N of M`.
    pub busy: u32,
    pub limit: u32,
}

/// A card call that failed.
#[derive(Debug)]
pub enum CardError {
    NotInstalled,
    /// `card` exited nonzero, with its standard error, or printed text
    /// the hub cannot read.
    Failed(String),
}

/// Writes card's own message.
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

/// Reads the keys of `card list` lines, which look like
/// `DEMO-KB-007 ready task Retry failed uploads`. The last line,
/// `N cards`, holds no key.
fn parse_card_list(text: &str) -> Vec<CardKey> {
    text.lines()
        .filter_map(|line| line.split_once(' '))
        .filter_map(|(first, _)| first.parse().ok())
        .collect()
}

/// Reads `card show` text. The `sessions:` items of the front matter
/// look like `- "opencode ses_0a1b2c3d4e5f"` with the ID last, and the
/// title is the first `# KEY ...` heading after it.
fn parse_card_note(key: &CardKey, text: &str) -> CardNote {
    let mut note = CardNote::default();
    let mut lines = text.lines().skip(1);
    let mut in_sessions = false;
    // The front matter ends at the second `---` line.
    for line in lines.by_ref().take_while(|line| line.trim() != "---") {
        if line.trim() == "sessions:" {
            in_sessions = true;
        } else if let Some(item) = line.trim().strip_prefix("- ").filter(|_| in_sessions) {
            let id = item.trim_matches('"').split_whitespace().last();
            note.sessions.extend(id.map(str::to_owned));
        } else {
            in_sessions = false;
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
    }
    note
}

/// Reads `card board` text. Returns `None` when the text lacks its
/// `review N` line or its `active+review N of M` line.
fn parse_board_counts(text: &str) -> Option<BoardCounts> {
    let mut review = None;
    let mut busy_and_limit = None;
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["review", count, ..] => review = count.parse().ok(),
            ["active+review", busy, "of", limit] => {
                busy_and_limit = busy.parse().ok().zip(limit.parse().ok());
            }
            _ => {}
        }
    }
    let (busy, limit) = busy_and_limit?;
    Some(BoardCounts {
        review: review?,
        busy,
        limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `hub brief` prints the board line from these counts, and the
    /// user decides from it whether to review before starting more. A
    /// limit or review count read from the wrong word misleads them.
    #[test]
    fn parse_board_counts_reads_busy_and_limit() {
        let board = "draft 79  ready 7  active 9\n\
                     review 24  accepted 43  dropped 4\n\
                     active+review 33 of 8\n\
                     \n\
                     REVIEW, oldest first (24)\n\
                     - DEMO-KB-001 2026-11-02\n  \
                       Fix the docs\n";
        let cases = [
            (
                "board",
                board,
                Some(BoardCounts {
                    review: 24,
                    busy: 33,
                    limit: 8,
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
                criteria: vec![
                    "- Uploads fail on a slow link.".to_owned(),
                    "- [ ] A failed upload retries.".to_owned(),
                ],
            }
        );
    }
}
