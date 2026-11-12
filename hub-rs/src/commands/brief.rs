//! Runs `hub brief`, which prints everything that waits on the user.
//! An entry in `decisions.md` holds a `## <title>`, the question, two
//! or more `- ` choices, `Recommended:` and `If no reply:`.

use jiff::Zoned;

use super::Outcome;
use crate::shared::decisions::{self, DecisionEntry, Urgency};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools};

/// The most questions that may wait on the user at once. More bury the
/// ones that matter, so the hub holds a new question until the user
/// answers one.
const QUESTION_LIMIT: usize = 5;

/// Prints the board line, `reviews.md`, then every decision entry in
/// full, so the user never has to ask what a numbered question was.
/// A failed `card board` becomes a `board: REASON` line.
///
/// # Errors
///
/// `HubError::NotDone`, printing nothing, when an entry lacks a part
/// the user needs to decide, when its `Not before:` time does not read,
/// or when more than `QUESTION_LIMIT` entries wait on the user at `now`.
pub fn run_brief(tools: &Tools<'_>, run: &RunFolder, now: &Zoned) -> Result<Outcome, HubError> {
    let entries = decisions::read_decisions(&run.read_optional_file("decisions.md")?);
    let mut problems = Vec::new();
    for entry in &entries {
        let missing = find_missing_parts(entry);
        if !missing.is_empty() {
            problems.push(format!(
                "decisions.md entry '{}' lacks {}",
                entry.title,
                missing.join(", ")
            ));
        }
        if let Err(when) = entry.read_not_before() {
            problems.push(format!(
                "decisions.md entry '{}' has Not before: {when}, not a time such as 2026-11-11 20:00",
                entry.title
            ));
        }
    }
    // With no card list, no entry counts as done, so all of them count.
    let stages = tools.cards.list_stages().unwrap_or_default();
    let waiting = entries
        .iter()
        .filter(|entry| entry.urgency(now, &stages) == Urgency::Now)
        .count();
    if waiting > QUESTION_LIMIT {
        problems.push(format!(
            "{waiting} decisions.md entries wait on the user now, over the limit of \
             {QUESTION_LIMIT}; hold new questions until the user answers one"
        ));
    }
    if !problems.is_empty() {
        return Err(HubError::NotDone(problems.join("\n")));
    }

    let decision_count = match entries.len() {
        1 => "1 decision".to_owned(),
        count => format!("{count} decisions"),
    };
    let board_line = match tools.cards.read_board() {
        Ok(board) => format!(
            "board {}/{}, review {}; {decision_count}",
            board.busy, board.limit, board.review
        ),
        Err(error) => format!("board: {error}"),
    };
    let mut sections = vec![board_line];
    let pull_requests = run.read_optional_file("reviews.md")?;
    if !pull_requests.trim().is_empty() {
        sections.push(pull_requests.trim().to_owned());
    }
    for entry in &entries {
        let mut section = vec![format!("Decision: {}", entry.title)];
        section.extend(entry.lines.iter().cloned());
        sections.push(section.join("\n"));
    }
    if pull_requests.trim().is_empty() && entries.is_empty() {
        sections.push("Nothing waits on the user.".to_owned());
    }
    let text = sections.join("\n\n");
    Ok(Outcome {
        lines: text.lines().map(str::to_owned).collect(),
    })
}

/// Lists the parts an entry lacks, such as `two choices`. Lines that
/// start with a space carry on the line above, so they are not parts.
fn find_missing_parts(entry: &DecisionEntry) -> Vec<&'static str> {
    const FIELDS: [&str; 3] = ["Recommended:", "If no reply:", "Depends on:"];
    let starts: Vec<&str> = entry
        .lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.starts_with(' '))
        .collect();
    let choice_count = starts.iter().filter(|line| line.starts_with("- ")).count();
    let has_question = starts
        .iter()
        .any(|line| !line.starts_with("- ") && !FIELDS.iter().any(|field| line.starts_with(field)));
    let mut missing = Vec::new();
    if !has_question {
        missing.push("the question");
    }
    if choice_count < 2 {
        missing.push("two choices");
    }
    if !starts.iter().any(|line| line.starts_with("Recommended:")) {
        missing.push("the Recommended: line");
    }
    if !starts.iter().any(|line| line.starts_with("If no reply:")) {
        missing.push("the If no reply: line");
    }
    missing
}
