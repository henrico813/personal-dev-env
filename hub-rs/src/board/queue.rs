//! Reads what the board shows, through the same steps as `hub status`
//! and `hub brief`, into plain values the screen draws. Read
//! `read_queue` first.

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::commands::status::describe_why;
use crate::shared::decisions::{self, DecisionEntry};
use crate::shared::lifecycle::{self, Lifecycle};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools, events, facts};

/// Everything the board shows, read at one moment.
#[derive(Debug)]
pub(crate) struct Queue {
    /// The time of the read, such as `10:42`.
    pub read_at: String,
    /// Sorted by state, then name.
    pub sessions: Vec<SessionLine>,
    /// Names with an unread report.
    pub unread: Vec<String>,
    /// Active and review cards with no session, such as
    /// `DEMO-KB-001 active`.
    pub cards_without_session: Vec<String>,
    /// The entries of `decisions.md`.
    pub questions: Vec<Question>,
    /// One item per table row or paragraph of `reviews.md`.
    pub reviews: Vec<String>,
    /// The `card board` counts, such as `5/8`, or `None` when the read
    /// failed.
    pub card_counts: Option<String>,
    /// One line per source that could not be read.
    pub problems: Vec<String>,
}

/// One session and why it is in its state.
#[derive(Debug)]
pub(crate) struct SessionLine {
    pub name: String,
    pub state: Lifecycle,
    /// The same words as the WHY cell of `hub status`, such as
    /// `unread report`, or `-`.
    pub why: String,
    /// The card and its stage, such as `DEMO-KB-001 active`.
    pub card: Option<String>,
}

/// One question in `decisions.md`.
#[derive(Debug)]
pub(crate) struct Question {
    pub title: String,
    /// The first line that is not a choice or a field, or empty.
    pub text: String,
}

/// Reads the sessions, reports, questions and pull requests once.
/// Times print in `time_zone`.
///
/// # Errors
///
/// `HubError::Herdr` when Herdr cannot list the agents, and
/// `HubError::File` when a run folder file cannot be read. A failed
/// `card` or OpenCode read becomes a problem line instead.
pub(crate) fn read_queue(
    tools: &Tools<'_>,
    run: &RunFolder,
    time_zone: &TimeZone,
) -> Result<Queue, HubError> {
    let memory = events::load_memory(run)?;
    let collected = facts::collect_session_facts(tools, run, &memory)?;
    let mut sessions: Vec<SessionLine> = collected
        .sessions
        .into_iter()
        .map(|facts| {
            let verdict = lifecycle::decide_lifecycle(&facts);
            SessionLine {
                why: describe_why(&facts, &verdict, time_zone),
                state: verdict.state,
                card: facts
                    .card
                    .map(|card| format!("{} {}", card.key.as_str(), card.stage.as_str())),
                name: facts.name,
            }
        })
        .collect();
    sessions.sort_by(|left, right| {
        (left.state.rank(), &left.name).cmp(&(right.state.rank(), &right.name))
    });
    let mut problems = collected.problems;
    let card_counts = match tools.cards.read_board() {
        Ok(board) => Some(format!("{}/{}", board.busy, board.limit)),
        Err(error) => {
            problems.push(format!("cards: {error}"));
            None
        }
    };
    let questions = decisions::read_decisions(&run.read_optional_file("decisions.md")?)
        .iter()
        .map(Question::from_entry)
        .collect();
    Ok(Queue {
        read_at: Timestamp::now()
            .to_zoned(time_zone.clone())
            .strftime("%H:%M")
            .to_string(),
        sessions,
        unread: collected.unread,
        cards_without_session: collected
            .no_session
            .iter()
            .map(|(key, stage)| format!("{} {}", key.as_str(), stage.as_str()))
            .collect(),
        questions,
        reviews: split_reviews(&run.read_optional_file("reviews.md")?),
        card_counts,
        problems,
    })
}

impl Question {
    /// Returns the entry's title and its question line. A choice or a
    /// field such as `Recommended:` is never the question.
    fn from_entry(entry: &DecisionEntry) -> Question {
        const FIELDS: [&str; 3] = ["Recommended:", "If no reply:", "Depends on:"];
        let text = entry
            .lines
            .iter()
            .find(|line| {
                !line.starts_with(' ')
                    && !line.starts_with("- ")
                    && !FIELDS.iter().any(|field| line.starts_with(field))
            })
            .cloned()
            .unwrap_or_default();
        Question {
            title: entry.title.clone(),
            text,
        }
    }
}

/// Splits `reviews.md`, which is free prose, into one item per table
/// row and one per paragraph. A paragraph's lines join with spaces.
/// Headings and table header rows are left out.
fn split_reviews(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let mut last_was_table_row = false;
    for line in text.lines().map(str::trim) {
        // A heading needs a space after its marks, so `#468 (tests)` is text.
        let is_heading = line.starts_with('#') && line.trim_start_matches('#').starts_with(' ');
        let is_text = !line.is_empty() && !is_heading && !line.starts_with('|');
        if !is_text && !paragraph.is_empty() {
            items.push(paragraph.join(" "));
            paragraph.clear();
        }
        if is_text {
            paragraph.push(line);
            last_was_table_row = false;
        } else if line.starts_with('|') {
            let cells: Vec<&str> = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .filter(|cell| !cell.is_empty())
                .collect();
            let is_separator = !cells.is_empty()
                && cells
                    .iter()
                    .all(|cell| cell.chars().all(|c| c == '-' || c == ':'));
            // The row above a `| --- |` line is the table's header.
            if is_separator && last_was_table_row {
                items.pop();
            }
            if !is_separator && !cells.is_empty() {
                items.push(cells.join("  "));
            }
            last_was_table_row = !is_separator && !cells.is_empty();
        } else {
            last_was_table_row = false;
        }
    }
    if !paragraph.is_empty() {
        items.push(paragraph.join(" "));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `reviews.md` is prose the hub writes by hand. A table's header
    /// row shown as a pull request, or a paragraph cut where a line
    /// starts with `#471`, puts items on the board that are not there.
    #[test]
    fn split_reviews_drops_headers_keeps_whole_paragraphs() {
        let text = "\
# Pull requests waiting for the user's review

| PR | What it does | Worker |
| --- | --- | --- |

| #301 | Retry failed uploads | demo-kb-001-impl |

Merge in order 471, 468:
#471 (docs), #468 (tests).
";

        let items = split_reviews(text);

        assert_eq!(
            items,
            [
                "#301  Retry failed uploads  demo-kb-001-impl",
                "Merge in order 471, 468: #471 (docs), #468 (tests).",
            ]
        );
    }
}
