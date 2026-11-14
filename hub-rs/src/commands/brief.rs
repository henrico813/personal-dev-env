//! Runs `hub brief`, which prints everything that waits on the user.
//! An entry in `decisions.md` holds a `## <title>`, the question, two
//! or more `- ` choices, `Recommended:` and `If no reply:`.

use super::Outcome;
use crate::external::cards::{Board, format_days};
use crate::shared::decisions::{self, DecisionEntry};
use crate::shared::run_folder::RunFolder;
use crate::shared::{HubError, Tools};

/// Prints the board line, `reviews.md`, then every decision entry in
/// full, so the user never has to ask what a numbered question was.
/// A failed `card board` becomes a `board: REASON` line. Cards waiting
/// on others come last, as news the user may chase but need not act on.
///
/// # Errors
///
/// `HubError::NotDone`, printing nothing, when an entry lacks a part
/// the user needs to decide.
pub fn run_brief(tools: &Tools<'_>, run: &RunFolder) -> Result<Outcome, HubError> {
    let entries = decisions::read_decisions(&run.read_optional_file("decisions.md")?);
    let problems: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let missing = find_missing_parts(entry);
            (!missing.is_empty()).then(|| {
                format!(
                    "decisions.md entry '{}' lacks {}",
                    entry.title,
                    missing.join(", ")
                )
            })
        })
        .collect();
    if !problems.is_empty() {
        return Err(HubError::NotDone(problems.join("\n")));
    }

    let decision_count = match entries.len() {
        1 => "1 decision".to_owned(),
        count => format!("{count} decisions"),
    };
    let board = tools.cards.read_board();
    let mut board_lines = vec![match &board {
        Ok(board) => format!("board {}; {decision_count}", board.counts),
        Err(error) => format!("board: {error}"),
    }];
    board_lines.extend(board.as_ref().ok().and_then(Board::waiting_line));
    let waiting = board.map(|board| board.waiting).unwrap_or_default();
    let mut sections = vec![board_lines.join("\n")];
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
    if !waiting.is_empty() {
        let mut section = vec!["Waiting on others (no action needed)".to_owned()];
        for card in &waiting {
            section.push(format!(
                "- {}, {}",
                card.key.as_str(),
                format_days(card.days)
            ));
            section.extend(
                [&card.title, &card.reason]
                    .into_iter()
                    .filter(|text| !text.is_empty())
                    .map(|text| format!("  {text}")),
            );
        }
        sections.push(section.join("\n"));
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
