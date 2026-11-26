//! Runs `hub waiting`, which prints each question that waits on the
//! user and puts their count on the front hub's sidebar row. The hub
//! ends every message to the user with this output, so a question
//! never hides higher up the chat.

use super::Outcome;
use super::sidebar::find_front_hub;
use crate::shared::questions::{OpenQuestion, read_open_questions};
use crate::shared::{HubError, Tools};

/// Prints `N questions wait on you.`, then each open question as a
/// `**Q3. TITLE (KEY, asked 2026-11-25)**` line and its later lines, then
/// a `Reply like` line, or `No question waits on you.` when none waits.
/// The Q number is the one `card` keeps on the item, so a reply key such
/// as `KEY Q3A` names the same question in every message.
///
/// A failed card read prints one `Questions could not be read` line
/// and changes nothing. A failed sidebar write adds a last line, since
/// the questions matter more than the count.
pub fn run_waiting(tools: &Tools<'_>) -> Result<Outcome, HubError> {
    let questions = match read_open_questions(tools.cards) {
        Ok(questions) => questions,
        Err(error) => {
            return Ok(Outcome::from_line(format!(
                "Questions could not be read: {error}"
            )));
        }
    };
    let mut lines = format_questions(&questions);
    if let Err(error) = show_count_on_front_hub(tools, questions.len()) {
        lines.push(format!("Sidebar count not set: {error}"));
    }
    Ok(Outcome { lines })
}

/// Returns the lines `run_waiting` prints for these questions.
fn format_questions(questions: &[OpenQuestion]) -> Vec<String> {
    let Some(first) = questions.first() else {
        return vec!["No question waits on you.".to_owned()];
    };
    let count = match questions.len() {
        1 => "1 question waits".to_owned(),
        count => format!("{count} questions wait"),
    };
    let mut lines = vec![format!("{count} on you.")];
    for question in questions {
        let asked = match question.asked_on {
            Some(date) => format!("asked {date}"),
            None => "undated".to_owned(),
        };
        lines.push(String::new());
        lines.push(format!(
            "**Q{}. {} ({}, {asked})**",
            question.number,
            question.title,
            question.key.as_str()
        ));
        lines.extend(question.lines.iter().cloned());
    }
    lines.push(String::new());
    lines.push(format!("Reply like `{}A`.", first.format_id()));
    lines
}

/// Puts `count` on the front hub's sidebar row, the hub the "go to"
/// list shows first. Does nothing when this machine runs no hub.
fn show_count_on_front_hub(tools: &Tools<'_>, count: usize) -> Result<(), HubError> {
    let agents = tools.herdr.list_agent_panes()?;
    if let Some(hub) = find_front_hub(tools.runs_folder, &agents)? {
        tools.herdr.show_waiting_count(&hub.pane_id, count)?;
    }
    Ok(())
}
