//! Runs `hub ask` and `hub answer`, which keep a question for the user
//! on its card. `find_problems` names the parts a question file needs.

use super::Outcome;
use crate::external::cards::{CardKey, Mark, OPEN_QUESTIONS};
use crate::shared::{HubError, Tools};

/// The card section an answered question moves to.
const DECISIONS: &str = "Decisions";

/// Adds the question in `text` to the card's Open Questions, then marks
/// the card needs-you with its title. Prints a line such as
/// `DEMO-KB-007 asks 'Deploy tonight?'`.
///
/// # Errors
///
/// `HubError::NotDone`, changing nothing, when `text` lacks a part the
/// user needs to decide, or the card already waits on the user.
/// `HubError::PartlyDone` when the question was added and the mark
/// failed.
pub fn run_ask(tools: &Tools<'_>, key: &CardKey, text: &str) -> Result<Outcome, HubError> {
    let question = read_question(text)?;
    if let Some(open) = tools.cards.read_card(key)?.needs_you {
        return Err(HubError::NotDone(format!(
            "{} already waits on the user: {open}",
            key.as_str()
        )));
    }
    let title = &question[0];
    tools
        .cards
        .add_item(key, OPEN_QUESTIONS, &format_item(&question))?;
    tools
        .cards
        .add_mark(key, Mark::NeedsYou, title)
        .map_err(|error| HubError::PartlyDone {
            done: format!("added the question to {} {OPEN_QUESTIONS}", key.as_str()),
            error: Box::new(error.into()),
        })?;
    Ok(Outcome::line(format!("{} asks '{title}'", key.as_str())))
}

/// Moves the card's open question to Decisions, with `answer` as its
/// last line, then marks the card answered. Prints a line such as
/// `DEMO-KB-007 answered 'Deploy tonight?': A`.
///
/// # Errors
///
/// `HubError::NotDone`, changing nothing, when the card waits on no
/// question. `HubError::PartlyDone` when a step failed after the answer
/// was added. The answered mark comes last, so after any failure the
/// card still shows that it waits.
pub fn run_answer(tools: &Tools<'_>, key: &CardKey, answer: &str) -> Result<Outcome, HubError> {
    let note = tools.cards.read_card(key)?;
    let Some(title) = note.needs_you else {
        return Err(HubError::NotDone(format!(
            "{} waits on no question",
            key.as_str()
        )));
    };
    // A worker may have marked the card with no Open Questions item.
    let found = note
        .open_questions
        .iter()
        .position(|item| item.first() == Some(&title));
    let mut decision = match found {
        Some(index) => note.open_questions[index].clone(),
        None => vec![title.clone()],
    };
    decision.push(format!("Answer: {answer}"));

    tools
        .cards
        .add_item(key, DECISIONS, &format_item(&decision))?;
    let mut done = format!("added the answer to {} {DECISIONS}", key.as_str());
    let partly_done = |done: &str, error| HubError::PartlyDone {
        done: done.to_owned(),
        error: Box::new(HubError::Card(error)),
    };
    if let Some(index) = found {
        tools
            .cards
            .remove_item(key, OPEN_QUESTIONS, index + 1)
            .map_err(|error| partly_done(&done, error))?;
        done.push_str(" and removed the question");
    }
    tools
        .cards
        .add_mark(key, Mark::Answered, answer)
        .map_err(|error| partly_done(&done, error))?;
    Ok(Outcome::line(format!(
        "{} answered '{title}': {answer}",
        key.as_str()
    )))
}

/// Returns the question's title, then its other lines, blank lines
/// left out.
fn read_question(text: &str) -> Result<Vec<String>, HubError> {
    let mut lines = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty());
    let Some(title) = lines
        .next()
        .and_then(|line| line.strip_prefix("## "))
        .map(str::trim)
        .filter(|title| !title.is_empty())
    else {
        return Err(HubError::NotDone(
            "the question must start with a ## title line".to_owned(),
        ));
    };
    let mut question = vec![title.to_owned()];
    question.extend(lines.map(str::to_owned));
    let problems = find_problems(&question[1..]);
    if !problems.is_empty() {
        return Err(HubError::NotDone(format!(
            "question '{title}': {}",
            problems.join(", ")
        )));
    }
    Ok(question)
}

/// Lists each part the user needs to decide that `lines` lack, such as
/// `no Recommended: line`. Lines that start with a space carry on the
/// line above, so they are not parts.
fn find_problems(lines: &[String]) -> Vec<String> {
    const FIELDS: [&str; 3] = ["Recommended:", "If no reply:", "Depends on:"];
    let starts: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|line| !line.starts_with(' '))
        .collect();
    let choice_count = starts.iter().filter(|line| line.starts_with("- ")).count();
    let has_question = starts
        .iter()
        .any(|line| !line.starts_with("- ") && !FIELDS.iter().any(|field| line.starts_with(field)));
    let mut problems = Vec::new();
    if !has_question {
        problems.push("no question line".to_owned());
    }
    if choice_count < 2 {
        problems.push("fewer than 2 choices".to_owned());
    }
    for field in ["Recommended:", "If no reply:"] {
        if !starts.iter().any(|line| line.starts_with(field)) {
            problems.push(format!("no {field} line"));
        }
    }
    problems
}

/// Returns `lines` as one card item. Each line after the first starts
/// with two spaces, so `card` keeps the whole question as one item.
fn format_item(lines: &[String]) -> String {
    lines.join("\n  ")
}
